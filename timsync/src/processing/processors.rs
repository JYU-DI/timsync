use anyhow::Result;
use enum_dispatch::enum_dispatch;
use serde_json::{Map, Value};

use crate::processing::markdown_processor::MarkdownProcessor;
use crate::processing::prepared_markdown::PreparedDocumentMarkdown;
use crate::processing::style_theme_processor::StyleThemeProcessor;
use crate::processing::task_processor::TaskProcessor;
use crate::processing::tim_document::TIMDocument;
use crate::project::files::project_files::ProjectFile;

/// Enum representing the different types of file processors.
/// Used to determine which processor to use for a given file.
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub enum FileProcessorType {
    /// Markdown file processor.
    Markdown,
    /// Task plugin processor.
    TaskPlugin,
    /// Style theme processor.
    StyleTheme,
}

/// Enum of the different file processors.
/// Used as abstraction over all available file processor implementations.
///
/// Dispatches calls to the correct processor implementation based on the enum variant.
#[enum_dispatch(FileProcessorAPI, FileProcessorRenderAPI)]
pub enum FileProcessor<'a> {
    /// Markdown file processor.
    Markdown(MarkdownProcessor<'a>),
    /// Task plugin processor.
    Task(TaskProcessor<'a>),
    // Style theme processor.
    StyleTheme(StyleThemeProcessor<'a>),
}

/// Public API for the file processors.
#[enum_dispatch]
pub trait FileProcessorAPI {
    /// Add a file to the processor.
    /// The file is registered to the processor.
    ///
    /// # Arguments
    ///
    /// * `file` - The file to add to the processor.
    fn add_file(&mut self, file: ProjectFile) -> Result<()>;

    /// Get additional context that should be included into project context.
    /// The added context is can be used in templating under the `site` variable.
    ///
    /// returns: Option<Map<String, Value>>
    fn get_processor_context(&self) -> Option<Map<String, Value>>;

    /// Get information about the TIM documents that the processor produces.
    /// Depending on the processor, this list might contain different number of documents
    /// than the number of files added to the processor.
    /// The implementation of the processor determines how many documents should be produced.
    ///
    /// returns: Vec<TIMDocument>
    fn get_tim_documents(&self) -> Vec<TIMDocument>;
}

/// Private internal API for the file processors. Used by the TIMDocument to delegate calls to the processor.
#[enum_dispatch]
pub(in crate::processing) trait FileProcessorInternalAPI {
    /// Render a TIM document.
    ///
    /// # Arguments
    ///
    /// * `tim_document` - The TIM document to render. The reference is guaranteed to be from the same processor.
    ///
    /// returns: Result<PreparedDocumentMarkdown>
    fn render_tim_document(&self, tim_document: &TIMDocument) -> Result<PreparedDocumentMarkdown>;

    /// Get the metadata for the TIM document (i.e., the front matter) as a JSON object.
    ///
    /// # Arguments
    /// * `tim_document` - The TIM document to get the metadata for.
    ///
    /// returns: Result<Value>
    fn get_project_file_front_matter_json(&self, tim_document: &TIMDocument) -> Result<Value>;

    /// Get the local path of the TIM document.
    ///
    /// If the TIM document is a local file, this method returns the local path of the file
    /// relative to the project root.
    /// Otherwise, it returns None.
    ///
    /// # Arguments
    /// * `tim_document` - The TIM document to get the local path for.
    ///
    /// returns: Option<String>
    fn get_project_file_local_path(&self, tim_document: &TIMDocument) -> Option<String>;
}
