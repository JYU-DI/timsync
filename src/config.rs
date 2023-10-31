use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simplelog::warn;

/// Default TIM host to use if no host is specified
pub const DEFAULT_SYNC_TARGET_HOST: &str = "https://tim.jyu.fi";
/// Folder in which all TIMSync files are stored
pub const CONFIG_FOLDER: &str = ".timsync";
/// Name of the config file for TIMSync
pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Deserialize, Serialize)]
/// The configuration for TIMSync
///
/// TIMSync stores its configuration in a TOML file in `<project_root>/.timsync/config.toml`.
pub struct SyncConfig {
    /// The targets to which documents are synced to
    ///
    /// When syncing the documents with `timsync sync`, the user can specify
    /// the target to which the documents are synced to.
    /// The default target is called `default`.
    targets: HashMap<String, SyncTarget>,
}

#[derive(Debug, Deserialize, Serialize)]
/// Information about a single sync target
///
/// The sync target contains all information needed to upload the files to a TIM instance.
pub struct SyncTarget {
    /// TIM hostname. Must include the protocol, e.g. `https://tim.jyu.fi`
    pub host: String,

    /// The root folder path to which the documents are synced to in TIM.
    /// Must not contain trailing or leading slashes.
    ///
    /// For example, if the folder is visible at
    ///
    ///     https://tim.jyu.fi/view/kurssit/tie/kurssi
    ///
    /// then the folder root is `kurssit/tie/kurssi`.
    pub folder_root: String,

    /// The username to use when authenticating to TIM.
    ///
    /// **Do not use your personal account for this!**
    /// Currently, authentication information is stored in plain text in the config file.
    /// Instead, create a separate, new TIM account for this purpose.
    pub username: String,

    /// The password to use when authenticating to TIM.
    ///
    /// **Do not use your personal account for this!**
    /// Currently, authentication information is stored in plain text in the config file.
    /// Instead, create a separate, new TIM account for this purpose.
    pub password: String,
}

impl SyncConfig {
    /// Create a new, empty configuration
    pub fn new() -> Self {
        SyncConfig {
            targets: HashMap::new(),
        }
    }

    /// Get a sync target by name.
    ///
    /// # Arguments
    ///
    /// * `name`: Sync target name
    ///
    /// returns: Option<&SyncTarget>
    pub fn get_target(&self, name: &str) -> Option<&SyncTarget> {
        self.targets.get(name)
    }

    /// Set a sync target by name.
    ///
    /// # Arguments
    ///
    /// * `name`: Sync target name to set
    /// * `target`: Sync target config
    pub fn set_target(&mut self, name: &str, target: SyncTarget) {
        self.targets.insert(name.to_string(), target);
    }

    /// Read a SyncConfig from a TOML file.
    /// The read might fail if it is not a valid TIMSync config file in TOML format.
    ///
    /// # Arguments
    ///
    /// * `path`: Path to the file to read.
    ///
    /// returns: Result<SyncConfig, Error>
    pub fn read_file(path: &Path) -> Result<Self> {
        let toml_str = std::fs::read_to_string(path)
            .with_context(|| format!("Could not open file {} for reading", path.display()))?;
        let res: Self = toml::from_str(&toml_str)
            .with_context(|| format!("Could not parse TIMSync config file {}", path.display()))?;
        Ok(res)
    }

    /// Write the SyncConfig to a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path`: Path to the file to write.
    ///
    /// returns: Result<(), Error>
    pub fn write_file(&self, path: &Path) -> Result<()> {
        let toml_str = toml::to_string_pretty(self).with_context(|| {
            format!("Could not serialize TIMSync config file {}", path.display())
        })?;
        std::fs::write(path, toml_str)
            .with_context(|| format!("Could not write file {} for writing", path.display()))?;
        Ok(())
    }
}

// TODO: Maybe move to a separate module?

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
