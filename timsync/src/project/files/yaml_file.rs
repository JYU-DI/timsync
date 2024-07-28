use std::path::PathBuf;

use lazy_init::Lazy;

use crate::processing::processors::FileProcessorType;
use crate::project::files::project_files::ProjectFileAPI;
use crate::project::files::util::{get_or_read_file_contents, get_or_set_front_matter_position};

/// A basic YAML file.
/// The file contains a YAML object.
pub struct YAMLFile {
    path: PathBuf,
    default_file_processor: FileProcessorType,
    contents: Lazy<anyhow::Result<String>>,
    front_matter_position: Lazy<Option<(usize, usize)>>,
}

impl YAMLFile {
    /// Create a new YAML file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the YAML file.
    /// * `default_file_processor` - The default file processor to use for the file.
    ///
    /// Returns: YAMLFile
    pub fn new(path: PathBuf, default_file_processor: FileProcessorType) -> Self {
        Self {
            path,
            default_file_processor,
            contents: Lazy::new(),
            front_matter_position: Lazy::new(),
        }
    }
}

impl ProjectFileAPI for YAMLFile {
    fn path(&self) -> &PathBuf {
        &self.path
    }

    fn front_matter_pos(&self) -> Option<(usize, usize)> {
        get_or_set_front_matter_position(&self.contents, &self.front_matter_position, "---", "---")
    }

    fn contents(&self) -> anyhow::Result<&str> {
        get_or_read_file_contents(&self.path, &self.contents)
    }

    fn processor_type(&self) -> FileProcessorType {
        self.default_file_processor
    }
}
