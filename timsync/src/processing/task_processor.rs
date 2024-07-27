use std::cell::OnceCell;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use handlebars::Handlebars;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::processing::prepared_markdown::PreparedDocumentMarkdown;
use crate::processing::processors::{FileProcessorAPI, FileProcessorInternalAPI};
use crate::processing::tim_document::TIMDocument;
use crate::project::files::project_files::{
    GeneralProjectFileMetadata, ProjectFile, ProjectFileAPI,
};
use crate::project::global_ctx::GlobalContext;
use crate::project::project::Project;
use crate::util::templating::ExtendableContext;
use crate::util::tim_client::random_par_id;

struct TaskInfo {
    par_id: String,
    file: ProjectFile,
    task_settings: TaskSettings,
}

pub struct TaskProcessor<'a> {
    files: HashMap<String, TaskInfo>,
    renderer: Handlebars<'a>,
    global_context: Rc<OnceCell<GlobalContext>>,
}

pub const TASKS_DOCPATH: &str = "_project_tasks";
pub const TASKS_UID: &str = "_timsync_tasks_doc";
pub const TASKS_REF_MAP_KEY: &str = "_timsync_tasks_ref_map";
pub const TASKS_TITLE: &str = "Project tasks";

#[derive(Deserialize)]
struct TaskSettings {
    plugin: String,
    plugin_attributes: Option<Map<String, Value>>,
}

impl<'a> TaskProcessor<'a> {
    pub fn new(project: &'a Project, global_context: Rc<OnceCell<GlobalContext>>) -> Result<Self> {
        let mut renderer = Handlebars::new();
        for (name, template) in project.get_template_files()? {
            renderer.register_template_file(&name, template)?;
        }

        Ok(Self {
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
    fn render_tim_document(&self, _: &TIMDocument) -> Result<PreparedDocumentMarkdown> {
        // This processor produces only one document.
        // Idea:
        // 1. Iterate over all project files and pass them through the Handlebars renderer
        // 2. Collect the rendered documents and insert them as plugin paragraphs with the correct plugin type and any extra attributes
        // 3. Return the prepared markdown

        let mut result_buf: Vec<u8> = Vec::new();

        for (uid, task_info) in self.files.iter() {
            let proj_file_path = task_info.file.path();
            let contents = task_info.file.contents_without_front_matter()?;

            let mut ctx = self
                .global_context
                .get()
                .expect("Global context not set")
                .handlebars_context();
            ctx.extend_with_json(&task_info.file.front_matter_json()?);
            ctx.extend_with_json(&json!({
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

            self.renderer
                .render_template_with_context_to_write(&contents, &ctx, &mut result_buf)
                .context("Could not render plugin YAML")?;

            write!(result_buf, "\n\n```\n\n").context("Could not write plugin paragraph")?;
        }

        let result_str =
            String::from_utf8(result_buf).expect("Could not convert result buffer to string");

        Ok(result_str.into())
    }

    fn get_project_file_metadata(&self, _: &TIMDocument) -> Result<GeneralProjectFileMetadata> {
        // This processor produces only one document, so we can return the same metadata
        Ok(GeneralProjectFileMetadata {
            processor: None,
            uid: Some(TASKS_UID.to_string()),
        })
    }
}
