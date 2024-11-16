use std::cell::OnceCell;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use handlebars::Handlebars;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::processing::prepared_document::PreparedDocument;
use crate::processing::processors::{FileProcessorAPI, FileProcessorInternalAPI};
use crate::processing::tim_document::TIMDocument;
use crate::project::files::project_files::{ProjectFile, ProjectFileAPI};
use crate::project::global_ctx::GlobalContext;
use crate::project::project::Project;
use crate::util::path::RelativizeExtension;
use crate::util::templating::{
    ContextExtension, RendererExtension, TimRendererExt, FILE_MAP_ATTRIBUTE,
};
use crate::util::tim_client::random_par_id;

struct TaskInfo {
    par_id: String,
    file: ProjectFile,
    task_settings: TaskSettings,
}

/// Processor for TIM plugin tasks.
/// The processor generates a single TIM document with all tasks in the project.
/// The added project files are passed through the templating engine and the results
/// are added as documents paragraphs to the TIM document.
///
/// All files added to this processor must have a front matter that defines values present in
/// `TaskSettings`.
///
/// The processor registers a global context variable `_timsync_tasks_ref_map` that maps task UIDs
/// to their corresponding paragraph IDs. This may be used in other processors to find the (doc_id, par_id)
/// tuple for a task.
pub struct TaskProcessor<'a> {
    project: &'a Project,
    files: HashMap<String, TaskInfo>,
    renderer: Handlebars<'a>,
    global_context: Rc<OnceCell<GlobalContext>>,
}

/// Path to the generated tasks document.
pub const TASKS_DOCPATH: &str = "_project_tasks";
/// Title of the generated tasks document.
pub const TASKS_TITLE: &str = "Project tasks";
/// UID of the generated tasks document.
/// Used by the templating engine to implement the `task` helper.
pub const TASKS_UID: &str = "_timsync_tasks_doc";
/// Key for the tasks reference map in the global context.
/// Used by the templating engine to implement the `task` helper.
pub const TASKS_REF_MAP_KEY: &str = "_timsync_tasks_ref_map";

/// Settings for a task. Must be defined in front matter of each project file
/// that will be processed as a task.
#[derive(Deserialize)]
struct TaskSettings {
    /// Plugin type to use for the task. Mandatory to specify.
    /// The value will be set as the `plugin` attribute in the plugin paragraph.
    plugin: String,
    /// Additional attributes to be added to the plugin paragraph. Optional.
    /// Any key-value pair will be added to the paragraph as such:
    /// ````
    /// ``` {key1="value1" key2="value2" ...}
    /// ```
    /// ````
    plugin_attributes: Option<Map<String, Value>>,
}

impl<'a> TaskProcessor<'a> {
    /// Create a new task processor.
    ///
    /// # Arguments
    ///
    /// * `project` - The project to process.
    /// * `global_context` - The global context to use for the processor.
    ///
    /// returns: Result<TaskProcessor>
    pub fn new(project: &'a Project, global_context: Rc<OnceCell<GlobalContext>>) -> Result<Self> {
        let renderer = Handlebars::new()
            .with_file_helpers()
            .with_project_templates(project)?
            .with_project_helpers(project)?;

        Ok(Self {
            project,
            files: HashMap::new(),
            renderer,
            global_context,
        })
    }
}

impl<'a> FileProcessorAPI for TaskProcessor<'a> {
    fn add_file(&mut self, file: ProjectFile) -> Result<()> {
        let metadata = file.read_general_metadata()?;
        let Some(uid) = metadata.uid else {
            return Err(anyhow!(
                "File must have `uid` set in order to be processed as a task"
            ));
        };
        if let Some(other_task) = self.files.get(&uid) {
            return Err(anyhow!(
                "Task with UID `{}` already exists in the project in path {}",
                uid,
                other_task.file.path().display()
            ));
        }

        let task_settings: TaskSettings = serde_yaml::from_str(file.front_matter()?)
            .context("Could not read task information from front matter")?;

        self.files.insert(
            uid,
            TaskInfo {
                par_id: random_par_id(),
                file,
                task_settings,
            },
        );
        Ok(())
    }

    fn get_processor_context(&self) -> Option<Map<String, Value>> {
        let mut ref_map = Map::new();
        for (uid, task_info) in self.files.iter() {
            ref_map.insert(uid.clone(), Value::String(task_info.par_id.clone()));
        }
        let mut res = Map::new();
        res.insert(TASKS_REF_MAP_KEY.to_string(), Value::Object(ref_map));
        Some(res)
    }

    fn get_tim_documents(&self) -> Vec<TIMDocument> {
        vec![TIMDocument {
            renderer: self,
            title: TASKS_TITLE,
            path: TASKS_DOCPATH,
            id: None,
        }]
    }
}

impl<'a> FileProcessorInternalAPI for TaskProcessor<'a> {
    fn render_tim_document(&self, _: &TIMDocument) -> Result<PreparedDocument> {
        // This processor produces only one document.
        // Idea:
        // 1. Iterate over all project files and pass them through the Handlebars renderer
        // 2. Collect the rendered documents and insert them as plugin paragraphs with the correct plugin type and any extra attributes
        // 3. Return the prepared markdown

        let mut result_buf: Vec<u8> = Vec::new();
        let project_root_dir = self.project.get_root_path();

        let mut upload_files_map = HashMap::new();

        for (uid, task_info) in self.files.iter() {
            let proj_file_path = task_info
                .file
                .path()
                .relativize(project_root_dir)
                .to_string_lossy()
                .to_string();
            let contents = task_info.file.contents_without_front_matter()?;

            let mut ctx = self
                .global_context
                .get()
                .expect("Global context not set")
                .handlebars_context();
            ctx.extend_with_json(&task_info.file.front_matter_json()?);
            // We manually override the original "local_file_path"
            // to correctly point to the currently processed file
            // We also insert the path to point to the tasks document
            // so that the "file" helper can be used in the task files
            ctx.extend_with_json(&json!({
                "path": TASKS_DOCPATH,
                "local_file_path": proj_file_path
            }));

            write!(
                result_buf,
                "``` {{#{}  id=\"{}\" plugin=\"{}\" ",
                uid, task_info.par_id, task_info.task_settings.plugin
            )
            .context("Could not write plugin paragraph")?;
            if let Some(attr_map) = &task_info.task_settings.plugin_attributes {
                for (key, value) in attr_map.iter() {
                    write!(
                        result_buf,
                        "{}=\"{}\" ",
                        key,
                        value
                            .as_str()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| format!("{}", value))
                    )
                    .context("Could not write plugin attribute")?;
                }
            }
            write!(result_buf, "}}\n\n").context("Could not write plugin paragraph")?;

            let res = self
                .renderer
                .render_template_with_context_to_write_return_new_context(
                    &contents,
                    &ctx,
                    &mut result_buf,
                )
                .context("Could not render plugin YAML")?;

            let task_upload_files_map = res
                .modified_context
                .and_then(|c| {
                    c.data().get(FILE_MAP_ATTRIBUTE).and_then(|v| {
                        serde_json::from_value::<HashMap<String, String>>(v.clone()).ok()
                    })
                })
                .unwrap_or_default();
            upload_files_map.extend(task_upload_files_map);

            write!(result_buf, "\n\n```\n\n").context("Could not write plugin paragraph")?;
        }

        let result_str =
            String::from_utf8(result_buf).expect("Could not convert result buffer to string");

        Ok(PreparedDocument {
            markdown: result_str,
            upload_files: upload_files_map,
        })
    }

    fn get_project_file_front_matter_json(&self, _: &TIMDocument) -> Result<Value> {
        // This processor produces only one document, so we can return the same metadata
        Ok(json!({
            "uid": TASKS_UID,
        }))
    }

    fn get_project_file_local_path(&self, _: &TIMDocument) -> Option<String> {
        None
    }
}
