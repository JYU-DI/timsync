use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use futures::future::try_join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use simplelog::__private::paris::LogIcon;
use simplelog::info;
use thiserror::Error;
use walkdir::WalkDir;

use crate::project::config::SyncTarget;
use crate::project::project::Project;
use crate::util::path::{Relativize, WithSetExtension};
use crate::util::tim_client::{ItemType, TimClient, TimClientBuilder, TimClientErrors};

#[derive(Debug, Args)]
pub struct SyncOpts {
    #[arg(default_value = "default")]
    /// The name of the sync target to send document to. Defaults to "default".
    target: String,
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.') || s.starts_with('_'))
        .unwrap_or(false)
}

fn is_markdown(entry: &PathBuf) -> bool {
    entry.is_file() && entry.extension().map(|ext| ext == "md").unwrap_or(false)
}

#[derive(Debug, Error)]
enum SyncError {
    #[error("The item {0} is not a document, but a {1}")]
    ItemIsNotADocument(String, String),
}

// TODO: Move to own file
// TODO: Multiprocessing?
async fn process_markdown(
    doc_path: PathBuf,
    md_path: PathBuf,
    client: &TimClient,
    target_info: &SyncTarget,
    tick_progress: &ProgressBar,
) -> Result<()> {
    let doc_path = doc_path.to_string_lossy();
    tick_progress.set_message(format!("{}: Preparing", doc_path));
    tick_progress.tick();

    let markdown_content = std::fs::read_to_string(&md_path)
        .with_context(|| format!("Could not open {}", md_path.display()))?;

    // TODO: Read title from front matter of the markdown
    let file_name = md_path.file_stem().unwrap().to_string_lossy();
    let doc_path_tim = format!("{}/{}", target_info.folder_root, doc_path);

    tick_progress.set_message(format!("{}: Checking item info", doc_path));
    tick_progress.tick();

    let item_info_result = client.get_item_info(&doc_path_tim).await;

    if let Err(e) = item_info_result {
        match e.downcast_ref::<TimClientErrors>() {
            Some(TimClientErrors::ItemNotFound(_, _)) => {
                // Item does not exist, create it
                tick_progress.set_message(format!("{}: Creating item", doc_path));
                tick_progress.tick();

                client
                    .create_item(ItemType::Document, &doc_path_tim, &file_name)
                    .await?;
            }
            _ => {
                return Err(e);
            }
        }
    } else {
        let item_info = item_info_result.unwrap();

        match item_info.item_type {
            ItemType::Document => (),
            _ => {
                return Err(SyncError::ItemIsNotADocument(
                    doc_path.to_string(),
                    item_info.item_type.to_string(),
                )
                .into());
            }
        }
    }

    tick_progress.set_message(format!("{}: Uploading new markdown", doc_path));
    tick_progress.tick();

    client
        .upload_markdown(&doc_path_tim, &markdown_content)
        .await?;

    Ok(())
}

/// Synchronize the project with a remote TIM target.
///
/// # Arguments
///
/// * `opts`: Synchronization options
///
/// returns: Result<(), Error>
pub async fn sync_target(opts: SyncOpts) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let project =
        Project::resolve_from_directory(&current_dir).context("Could not resolve project")?;

    let target_info = project.config.get_target(&opts.target).context(format!(
        "Could not find sync target {}. Use `timsync target add` to add the target.",
        opts.target
    ))?;

    info!("Syncing to {} ({})...", opts.target, target_info.host);

    let root = project.get_root_path();
    // Find all .md files in the project, skip folders starting with a dot or underscore

    let md_files = WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|e| e.ok().map(|e| e.path().to_path_buf()))
        .filter(|p| is_markdown(p))
        .map(|p| (p.relativize(&root).with_set_extension(""), p))
        .collect::<Vec<_>>();

    println!("Syncing {} documents", md_files.len());

    let multi_progress = MultiProgress::new();
    let total_progress = multi_progress.add(
        ProgressBar::new(md_files.len() as u64).with_style(
            ProgressStyle::default_bar()
                .template("{msg} [{wide_bar}] {pos:>3}/{len:3}")
                .unwrap()
                .progress_chars("##-"),
        ),
    );
    let tick_progress = multi_progress.add(ProgressBar::new_spinner());

    total_progress.set_message("Sync progress");

    tick_progress.set_message("Logging in");
    tick_progress.enable_steady_tick(Duration::from_millis(100));

    let client = TimClientBuilder::new()
        .tim_host(&target_info.host)
        .build()
        .await
        .context("Could not connect to TIM")?;

    client
        .login_basic(&target_info.username, &target_info.password)
        .await
        .context("Could not log in to TIM")?;

    tick_progress.disable_steady_tick();
    tick_progress.set_message("Processing documents");

    let futures = md_files
        .into_iter()
        .map(|(doc_path, md_path)| async {
            let res =
                process_markdown(doc_path, md_path, &client, &target_info, &tick_progress).await;
            total_progress.inc(1);
            res
        })
        .collect::<Vec<_>>();

    // TODO: Maybe separate into a pipeline that requires success on all futures before continuing
    //   Steps:
    //    1. Prepare all documents without uploading (evaluate templates, files that need to be uploaded)
    //    2. Upload all prepared documents and files
    try_join_all(futures)
        .await
        .context("Could not sync documents")?;

    total_progress.finish_and_clear();
    tick_progress.finish_and_clear();

    info!(
        "{} Syncing complete! View the documents at {}/view/{}",
        LogIcon::Tick,
        target_info.host,
        target_info.folder_root
    );

    Ok(())
}
