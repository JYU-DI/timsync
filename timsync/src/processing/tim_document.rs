use anyhow::Result;

use crate::processing::prepared_markdown::PreparedDocumentMarkdown;
use crate::processing::processors::FileProcessorRenderAPI;

/// Struct representing a TIM document that is produced by the processor.
pub struct TIMDocument<'a> {
    /// The renderer used to render the TIM document.
    pub(in crate::processing) renderer: &'a dyn FileProcessorRenderAPI,

    /// The title of the TIM document.
    pub title: &'a str,

    /// The path of the TIM document.
    pub path: &'a str,

    /// The ID of the TIM document if present.
    pub id: Option<u64>,
}

impl TIMDocument<'_> {
    /// Get the contents of the TIM document.
    pub fn get_contents(&self) -> Result<PreparedDocumentMarkdown> {
        self.renderer.render_tim_document(&self)
    }
}
