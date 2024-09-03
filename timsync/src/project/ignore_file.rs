use anyhow::Result;
use std::path::{Path, PathBuf};

/// Filename of the ignore file
pub const SYNC_IGNORE_FILE_NAME: &str = ".timsyncignore";
/// Default content of the ignore file
pub const DEFAULT_SYNC_IGNORE_FILE: &str = r#"
# This file is used to ignore files and directories in the project.
# You can use glob patterns to match files and directories.
# These patterns will apply in addition to the default TIMSync ignore
# rules (dirs/files starting with _ or .).

README.md
"#;

/// A file that contains sync ignore patterns.
///
/// Any files that match the glob patterns defined in the ignore file are not processed.
pub struct IgnoreFile {
    ignore_patterns: Vec<glob::Pattern>,
}

impl IgnoreFile {
    /// Create a new IgnoreFile
    ///
    /// Returns: IgnoreFile
    pub fn new() -> Self {
        Self {
            ignore_patterns: Vec::new(),
        }
    }

    /// Create a new IgnoreFile and load ignore patterns from a file.
    /// Parses the file as a basic .gitignore file. Basic comments and empty lines are ignored.
    ///
    /// # Arguments
    ///
    /// * `project_path`: The path to the project directory
    ///
    /// Returns: Result<IgnoreFile, Error>
    pub fn for_project(project_path: &PathBuf) -> Result<Self> {
        let ignore_file_path = project_path.join(SYNC_IGNORE_FILE_NAME);
        let mut ignore_file = Self::new();

        if ignore_file_path.is_file() {
            ignore_file.add_ignore_patterns(&ignore_file_path)?;
        }

        Ok(ignore_file)
    }

    /// Add ignore patterns from a file.
    /// Any empty lines or lines starting with # are ignored.
    ///
    /// # Arguments
    ///
    /// * `ignore_file_path`: The path to the ignore file
    ///
    /// Returns: Result<(), Error>
    pub fn add_ignore_patterns(&mut self, ignore_file_path: &PathBuf) -> Result<()> {
        if !ignore_file_path.is_file() {
            return Ok(());
        }

        // SAFETY: The parent of a file path is always a directory
        let base_path = ignore_file_path.parent().unwrap();
        let ignore_file_contents = std::fs::read_to_string(ignore_file_path)?;
        self.ignore_patterns.extend(
            ignore_file_contents
                .lines()
                .filter(|line| {
                    let line = line.trim();
                    !line.is_empty() && !line.starts_with('#')
                })
                .map(|line| {
                    glob::Pattern::new(base_path.join(line).to_string_lossy().as_ref()).unwrap()
                }),
        );
        Ok(())
    }

    /// Check if a path is ignored by the ignore file.
    ///
    /// # Arguments
    ///
    /// * `path`: Full path to a file or directory
    ///
    /// Returns: bool
    pub fn is_ignored(&self, path: impl AsRef<Path>) -> bool {
        self.ignore_patterns
            .iter()
            .any(|pattern| pattern.matches_path(path.as_ref()))
    }
}
