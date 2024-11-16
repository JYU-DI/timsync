use std::path::PathBuf;

use anyhow::Result;
use lazy_init::Lazy;

use crate::processing::processors::FileProcessorType;
use crate::project::files::project_files::ProjectFileAPI;
use crate::project::files::util::{get_or_read_file_contents, get_or_set_front_matter_position};

/// A basic markdown file.
/// This represents a project file that contains markdown content.
pub struct MarkdownFile {
    path: PathBuf,
    contents: Lazy<Result<String>>,
    front_matter_position: Lazy<Option<(usize, usize)>>,
}

impl ProjectFileAPI for MarkdownFile {
    fn path(&self) -> &PathBuf {
        &self.path
    }

    fn front_matter_pos(&self) -> Option<(usize, usize)> {
        get_or_set_front_matter_position(&self.contents, &self.front_matter_position, "---", "---")
    }

    fn contents(&self) -> Result<&str> {
        get_or_read_file_contents(&self.path, &self.contents)
    }

    fn processor_type(&self) -> FileProcessorType {
        FileProcessorType::Markdown
    }
}

impl MarkdownFile {
    /// Create a new markdown file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the markdown file.
    ///
    /// Returns: MarkdownFile
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            contents: Lazy::new(),
            front_matter_position: Lazy::new(),
        }
    }
}
