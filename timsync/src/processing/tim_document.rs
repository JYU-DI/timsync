use anyhow::{Context, Result};
use serde_json::Value;

use crate::processing::prepared_document::PreparedDocument;
use crate::processing::processors::FileProcessorInternalAPI;
use crate::project::files::project_files::GeneralProjectFileMetadata;

/// Struct representing a TIM document that is produced by the processor.
pub struct TIMDocument<'a> {
    /// The renderer used to render the TIM document.
    pub(in crate::processing) renderer: &'a dyn FileProcessorInternalAPI,

    /// The title of the TIM document.
    pub title: &'a str,

    /// The path of the TIM document.
    pub path: &'a str,

    /// The ID of the TIM document if present.
    pub id: Option<u64>,
}

impl TIMDocument<'_> {
    /// Get the contents of the TIM document.
    pub fn render_contents(&self) -> Result<PreparedDocument> {
        self.renderer.render_tim_document(&self)
    }

    /// Get the general metadata associated with the TIM document.
    pub fn general_metadata(&self) -> Result<GeneralProjectFileMetadata> {
        let json = self.renderer.get_project_file_front_matter_json(&self)?;
        serde_json::from_value(json).context("Failed to deserialize general metadata")
    }

    /// Get the front matter associated with the TIM document.
    pub fn front_matter_json(&self) -> Result<Value> {
        self.renderer.get_project_file_front_matter_json(&self)
    }

    /// Get the local file path of the TIM document if it is a local file.
    ///
    /// If the TIM document is a local file, this method returns the local path of the file
    /// relative to the project root.
    /// Otherwise, it returns None.
    ///
    /// returns: Option<String>
    pub fn get_local_file_path(&self) -> Option<String> {
        self.renderer.get_project_file_local_path(&self)
    }
}
