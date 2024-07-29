use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use anyhow::Result;
use indoc::indoc;
use serde_json::{Map, Value};

use crate::processing::markdown_processor::MarkdownProcessor;
use crate::processing::prepared_markdown::PreparedDocumentMarkdown;
use crate::processing::processors::{FileProcessorAPI, FileProcessorInternalAPI};
use crate::processing::tim_document::TIMDocument;
use crate::project::files::project_files::{ProjectFile, ProjectFileAPI};
use crate::project::global_ctx::GlobalContext;
use crate::project::project::Project;

/// A processor for style themes.
/// The processor is mainly the same as the Markdown processor, but
/// it adds the necessary document settings for style themes.
///
/// Additionally, the processor registers a `site.themes` global context variable
/// which maps theme names to their full TIM paths.
pub struct StyleThemeProcessor<'a> {
    markdown_processor: MarkdownProcessor<'a>,
    file_paths_by_name: HashMap<String, String>,
}

impl<'a> StyleThemeProcessor<'a> {
    pub fn new(
        project: &'a Project,
        sync_target: &str,
        global_context: Rc<OnceCell<GlobalContext>>,
    ) -> Result<Self> {
        Ok(Self {
            markdown_processor: MarkdownProcessor::new(project, sync_target, global_context)?,
            file_paths_by_name: HashMap::new(),
        })
    }
}

impl<'a> FileProcessorAPI for StyleThemeProcessor<'a> {
    fn add_file(&mut self, file: ProjectFile) -> Result<()> {
        let file_name = file
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        if let Some(existing_file) = self.file_paths_by_name.get(&file_name) {
            return Err(anyhow::anyhow!(
                "Cannot add file '{}' as a style theme. Another theme file with the same name is already registered from '{}'. Theme file names must be unique within the project.",
                existing_file,
                file.path().display()
            ));
        }

        self.file_paths_by_name
            .insert(file_name, file.path().to_string_lossy().to_string());

        self.markdown_processor.add_file(file)
    }

    fn get_processor_context(&self) -> Option<Map<String, Value>> {
        let sync_target = self
            .markdown_processor
            .project
            .config
            .get_target(&self.markdown_processor.sync_target)
            .unwrap();

        let mut res = Map::new();
        let mut themes = Map::new();

        for doc in self.markdown_processor.get_tim_documents() {
            let path_filename = doc
                .path
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(doc.path);
            themes.insert(
                path_filename.to_string(),
                Value::String(format!("/{}/{}", sync_target.folder_root, doc.path)),
            );
        }

        res.insert("style_themes".to_string(), Value::Object(themes));

        Some(res)
    }

    fn get_tim_documents(&self) -> Vec<TIMDocument> {
        self.markdown_processor
            .get_tim_documents()
            .into_iter()
            .map(|x| TIMDocument {
                renderer: self,
                ..x
            })
            .collect()
    }
}

impl<'a> FileProcessorInternalAPI for StyleThemeProcessor<'a> {
    fn render_tim_document(&self, tim_document: &TIMDocument) -> Result<PreparedDocumentMarkdown> {
        let processed_style: String = self
            .markdown_processor
            .render_tim_document(tim_document)?
            .into();

        let res = format!(
            indoc! {r#"
            ``` {{settings=""}}
            description: "{}"
            ```

            ```scss
            {}
            ```"#
            },
            tim_document.title,
            processed_style.trim()
        );

        Ok(res.into())
    }

    fn get_project_file_front_matter_json(&self, tim_document: &TIMDocument) -> Result<Value> {
        self.markdown_processor
            .get_project_file_front_matter_json(tim_document)
    }
}
