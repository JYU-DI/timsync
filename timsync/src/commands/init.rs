use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{arg, Args};
use dialoguer::Confirm;
use path_absolutize::*;
use simplelog::{error, info};
use thiserror::Error;

use crate::commands::target::prompt_user_details_interactive;
use crate::project::config::{SyncConfig, SyncTarget, CONFIG_FILE_NAME, CONFIG_FOLDER};
use crate::project::global_ctx::{DEFAULT_GLOBAL_DATA, GLOBAL_DATA_CONFIG_FILE};

#[derive(Debug, Args)]
pub struct InitOptions {
    #[arg()]
    /// The path to the project directory. If not specified, the current directory is used.
    path: Option<PathBuf>,
    #[arg(short, long)]
    /// Force the initialization, even if the directory is already initialized.
    /// This will overwrite the existing configuration.
    force: bool,
    #[arg(short, long)]
    /// Do not prompt for user details.
    /// This will create an empty configuration.
    no_prompt: bool,
}

#[derive(Debug, Error)]
enum InitError {
    #[error("The path {0} is not a directory.")]
    PathIsNotADirectory(PathBuf),
    #[error("The project {0} is already initialized. Use --force to recreate the configuration.")]
    AlreadyInitialized(PathBuf),
}

const DEFAULT_GITIGNORE_CONTENT: &str = r#"# TIMSync tool
.timsync
"#;

async fn get_default_sync_target(no_prompt: bool) -> Result<Option<SyncTarget>> {
    if no_prompt || !console::user_attended() {
        info!("Skipping default sync target setup. Use `timsync target add` to add a sync target.");
        return Ok(None);
    }
    let setup_default_target = Confirm::new()
        .with_prompt(
            "Do you want to set up the default sync target?\nYou can add a sync target later.",
        )
        .default(true)
        .wait_for_newline(true)
        .interact()
        .context("Could not get user input")?;

    if !setup_default_target {
        return Ok(None);
    }

    prompt_user_details_interactive()
        .await
        .context("Could not get user details")
}

/// Initialize a new TIMSync project.
///
/// # Arguments
///
/// * `opts`: Initialization options
///
/// returns: Result<(), Error>
pub async fn init_repo(opts: InitOptions) -> Result<()> {
    let target_path = match opts.path {
        Some(path) => {
            if path.exists() && !path.is_dir() {
                return Err(InitError::PathIsNotADirectory(path).into());
            }
            path.absolutize()
                .context("Could not resolve the full path")?
                .to_path_buf()
        }
        None => std::env::current_dir()?,
    };

    let timsync_path = target_path.join(&CONFIG_FOLDER);
    if timsync_path.exists() {
        if !opts.force {
            return Err(InitError::AlreadyInitialized(timsync_path).into());
        }
        std::fs::remove_dir_all(&timsync_path)
            .context("Could not remove the existing configuration")?;
    }

    let mut config = SyncConfig::new();
    let sync_data = get_default_sync_target(opts.no_prompt).await?;
    if let Some(sync_data) = sync_data {
        let config = &mut config;
        config.set_target("default", sync_data);
    }

    info!("Initializing new project to {}", target_path.display());

    std::fs::create_dir_all(&timsync_path).context("Could not create the target directory")?;
    let timsync_config_file = timsync_path.join(&CONFIG_FILE_NAME);

    config.write_file(&timsync_config_file)?;

    let gitignore_file = target_path.join(".gitignore");

    // Create or update the .gitignore file
    if gitignore_file.exists() {
        let gitignore_content = std::fs::read_to_string(&gitignore_file)
            .context("Could not read the .gitignore file")?;
        if !gitignore_content.contains(&CONFIG_FOLDER) {
            let mut gitignore_file = std::fs::OpenOptions::new()
                .append(true)
                .open(&gitignore_file)
                .context("Could not open the .gitignore file for appending")?;
            gitignore_file
                .write_all(DEFAULT_GITIGNORE_CONTENT.as_bytes())
                .context("Could not append to the .gitignore file")?;
        }
    } else {
        std::fs::write(&gitignore_file, DEFAULT_GITIGNORE_CONTENT)
            .context("Could create .gitignore file")?;
    }

    let global_config_file = target_path.join(&GLOBAL_DATA_CONFIG_FILE);

    // Create or update the _config.yml file
    if !global_config_file.exists() {
        std::fs::write(&global_config_file, &DEFAULT_GLOBAL_DATA)
            .context("Could not create global data config file")?;
    }

    Ok(())
}
