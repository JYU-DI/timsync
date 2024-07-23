use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glob::glob;
use simplelog::warn;

use crate::project::config::{CONFIG_FILE_NAME, CONFIG_FOLDER, SyncConfig};
use crate::project::global_ctx::{GLOBAL_DATA_CONFIG_FILE, GlobalContext};
use crate::util::path::Relativize;

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

const TEMPLATE_FOLDER: &str = "_templates";

impl Project {
    /// Get the root path of the project
    pub fn get_root_path(&self) -> &Path {
        &self.root_path
    }
    
    /// Get the global context prefilled with data defined in the global data config file (`_config.yml`).
    ///
    /// returns: Result<GlobalContext, Error> 
    pub fn global_context(&self) -> Result<GlobalContext> {
        let global_data_path = self.root_path.join(GLOBAL_DATA_CONFIG_FILE);
        GlobalContext::for_project(&global_data_path)
    }

    /// Get the template files in the project.
    ///
    /// The template files are read from the `_templates` folder in the project's root folder.
    ///
    /// returns: Result<Vec<(String, PathBuf)>, Error>
    pub fn get_template_files(&self) -> Result<Vec<(String, PathBuf)>> {
        let template_folder = self.root_path.join(TEMPLATE_FOLDER);
        let mut files = Vec::new();

        if template_folder.is_dir() {
            let glob_pattern = template_folder.join("**").join("*");
            for entry in glob(glob_pattern.to_string_lossy().as_ref()).with_context(|| {
                format!(
                    "Could not find templates from folder {}",
                    template_folder.display()
                )
            })? {
                match entry {
                    Ok(path) => {
                        if path.is_file() {
                            // Get path without the template folder prefix
                            let relative = path.relativize(&template_folder);
                            let template_name = relative.to_string_lossy().replace("\\", "/");
                            files.push((template_name, path));
                        }
                    }
                    Err(_) => {}
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
