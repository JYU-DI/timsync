use crate::project::config::{SyncConfig, CONFIG_FILE_NAME, CONFIG_FOLDER};
use crate::project::global_ctx::{GlobalContextBuilder, GLOBAL_DATA_CONFIG_FILE};
use simplelog::warn;
use std::path::{Path, PathBuf};

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

    pub fn get_data_context(&self) -> anyhow::Result<tera::Context> {
        let mut builder = GlobalContextBuilder::new();

        let global_config_path = self.root_path.join(GLOBAL_DATA_CONFIG_FILE);
        if global_config_path.exists() {
            builder.add_global_data(&global_config_path)?;
        }

        Ok(builder.build())
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
    pub fn resolve_from_directory(dir_path: &Path) -> anyhow::Result<Self> {
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
