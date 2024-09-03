use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use simplelog::warn;

use crate::project::config::{SyncConfig, CONFIG_FILE_NAME, CONFIG_FOLDER};
use crate::project::global_ctx::{GlobalContext};
use crate::project::ignore_file::IgnoreFile;
use crate::util::path::RelativizeExtension;

/// A TIMSync project
///
/// A TIMSync project is a directory that contains markdown files, images, files, templates,
/// and a .timsync folder with the TIMSync config.
pub struct Project {
    root_path: PathBuf,
    /// The TIMSync config for the project
    pub config: SyncConfig,
}

const MAX_SEARCH_DEPTH: usize = 10;

impl Project {
    /// Get the root path of the project
    pub fn get_root_path(&self) -> &Path {
        &self.root_path
    }

    /// Get the global context prefilled with data defined in the global data config file (`_config.yml`).
    ///
    /// returns: Result<GlobalContext, Error>
    pub fn global_context(&self) -> Result<GlobalContext> {
        GlobalContext::for_project(&self.root_path)
    }

    /// Get the ignore file for the project.
    /// The ignore file contains patterns to exclude files from the project.
    ///
    /// returns: Result<IgnoreFile, Error>
    pub fn ignore_file(&self) -> Result<IgnoreFile> {
        IgnoreFile::for_project(&self.root_path).context("Could not read the ignore file")
    }

    /// Find files in the project directory and its subdirectories.
    /// Returns a list of URL-safe names and the full paths to the files.
    ///
    /// # Arguments
    ///
    /// * `dir`: The directory to search for files in.
    /// * `glob`: The glob pattern to match files against.
    ///
    /// returns: Result<Vec<(String, PathBuf)>, Error>
    pub fn find_files(&self, dir: impl AsRef<Path>, glob: &str) -> Result<Vec<(String, PathBuf)>> {
        let base_folder = self.root_path.join(dir);
        let mut files = Vec::new();

        if base_folder.is_dir() {
            let glob_pattern = base_folder.join("**").join(glob);
            for entry in glob::glob(glob_pattern.to_string_lossy().as_ref())? {
                let Ok(path) = entry else {
                    continue;
                };
                if path.is_file() {
                    // Get path without the template folder prefix
                    let relative = path.relativize(&base_folder);
                    let template_name = relative.to_string_lossy().replace("\\", "/");
                    files.push((template_name, path));
                }
            }
        }

        Ok(files)
    }

    /// Resolve a project from a directory path.
    ///
    /// The project is determined by finding the `.timsync/config.toml` file in the given
    /// directory.
    /// If the config file is not found in the folder,
    /// the parent folders are also checked up to 10 levels.
    ///
    ///
    /// # Arguments
    ///
    /// * `dir_path`: Directory to search the project from.
    ///
    /// returns: Result<Project, Error>
    pub fn resolve_from_directory(dir_path: &Path) -> Result<Self> {
        if !dir_path.is_dir() {
            return Err(anyhow::anyhow!(
                "The given path is not a directory or does not exist: {}",
                dir_path.display()
            ));
        }

        // Search ancestors for the config folder up to MAX_SEARCH_DEPTH levels
        for parent in dir_path.ancestors().take(MAX_SEARCH_DEPTH) {
            let config_file = parent.join(CONFIG_FOLDER).join(CONFIG_FILE_NAME);
            if config_file.exists() {
                let result = SyncConfig::read_file(&config_file);
                match result {
                    Ok(config) => {
                        return Ok(Project {
                            root_path: parent.to_path_buf(),
                            config,
                        });
                    }
                    Err(e) => {
                        warn!(
                            "Could not read the config file at {}: {}",
                            config_file.display(),
                            e
                        );
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Could not find a valid TIMSync project in {} or its parents. Is the project initialized or are you in the correct directory?",
            dir_path.display()
        ))
    }
}
