use std::path::PathBuf;

use lazy_init::Lazy;

use crate::processing::processors::FileProcessorType;
use crate::project::files::project_files::ProjectFileAPI;
use crate::project::files::util::{get_or_read_file_contents, get_or_set_front_matter_position};

/// A basic CSS file.
/// The file contains a CSS/SCSS style.
pub struct CSSFile {
    path: PathBuf,
    contents: Lazy<anyhow::Result<String>>,
    front_matter_position: Lazy<Option<(usize, usize)>>,
}

impl CSSFile {
    /// Create a new CSS file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the (S)CSS file.
    ///
    /// Returns: CSSFile
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            contents: Lazy::new(),
            front_matter_position: Lazy::new(),
        }
    }
}

impl ProjectFileAPI for CSSFile {
    fn path(&self) -> &PathBuf {
        &self.path
    }

    fn front_matter_pos(&self) -> Option<(usize, usize)> {
        get_or_set_front_matter_position(&self.contents, &self.front_matter_position, "/*", "*/")
    }

    fn contents(&self) -> anyhow::Result<&str> {
        get_or_read_file_contents(&self.path, &self.contents)
    }

    fn processor_type(&self) -> FileProcessorType {
        FileProcessorType::StyleTheme
    }
}
