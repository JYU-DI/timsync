use std::cell::OnceCell;
use std::collections::{hash_map::Entry, HashMap, HashSet, LinkedList};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Error, Result};
use clap::Args;
use futures::future::try_join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::Itertools;
use serde_json::{json, Map, Value};
use simplelog::__private::paris::LogIcon;
use simplelog::info;
use thiserror::Error;
use walkdir::WalkDir;

use crate::processing::markdown_processor::MarkdownProcessor;
use crate::processing::processors::{FileProcessor, FileProcessorAPI, FileProcessorType};
use crate::processing::style_theme_processor::StyleThemeProcessor;
use crate::processing::task_processor::TaskProcessor;
use crate::processing::tim_document::TIMDocument;
use crate::project::files::project_files::{ProjectFile, ProjectFileAPI};
use crate::project::global_ctx::GlobalContext;
use crate::project::project::Project;
use crate::util::json::Merge;
use crate::util::path::LanguageCodeExtraction;
use crate::util::tim_client::{ItemType, TimClient, TimClientBuilder, TimClientErrors};

#[derive(Debug, Args)]
pub struct SyncOpts {
    #[arg(default_value = "default")]
    /// The name of the sync target to send document to. Defaults to "default".
    target: String,

    #[arg(long, short)]
    /// The language to sync. If not specified, all non-language-specific files are synced.
    language: Option<String>,
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.') || s.starts_with('_'))
        .unwrap_or(false)
}

#[derive(Debug, Error)]
enum SyncError {
    #[error("The sync target path {0} does not exist in TIM. Create the folder first in TIM and set appropriate permissions before syncing files.")]
    SyncTargetDoesNotExist(String),
    #[error("The sync target path {0} is not a folder in TIM. The target path must be a folder for sync to work.")]
    SyncTargetNotAFolder(String),
    // TODO: Include paths of all files that map to the same TIM document
    #[error("Multiple documents found for the same TIM path '{0}'. Make sure there are no duplicate paths in the project.")]
    ItemNameConflict(String),
    #[error("There is a document and a folder with the same path '{0}'. TIM requires that all items (folders, documents) have a unique path.")]
    ItemTypeConflict(String),
}

/// A single item entry. Used as a helper struct to manage item creation in TIM.
/// The path is split into two parts: the current base and the unprocessed rest of the path.
/// The path_rest is split more as the folder structure is processed.
struct ItemEntry<'a> {
    /// Path base. In other words, the part of the path before the first `/`.
    path_base: &'a str,
    /// Rest of the path. In other words, the part of the path after the first `/`.
    path_rest: &'a str,
    doc: TIMDocument<'a>,
}

impl ItemEntry<'_> {
    /// Check whether the current path represents the final document path.
    /// When path_rest is empty, the path_base contains the final name of the document.
    fn is_document_path(&self) -> bool {
        self.path_rest.is_empty()
    }
}

/// The possible entries that can be created in TIM.
/// Used as a helper for managing the creation of items in TIM.
enum ItemEntries<'a> {
    Document(ItemEntry<'a>),
    DocumentsInFolder(Vec<ItemEntry<'a>>),
}

/// The pipeline for synchronizing the project with a remote TIM target.
/// TODO: Perhaps refactor into a proper pipeline pattern (using enums) to ensure order in which pipeline steps execute.
struct SyncPipeline<'a> {
    project: &'a Project,
    global_context: Rc<OnceCell<GlobalContext>>,
    sync_target: &'a str,
    language: Option<&'a str>,
    processors: HashMap<FileProcessorType, FileProcessor<'a>>,
    progress: MultiProgress,
}

impl<'a> SyncPipeline<'a> {
    /// Create a new synchronization pipeline.
    ///
    /// # Arguments
    ///
    /// * `project`: The project to sync.
    /// * `sync_target`: The name of the sync target to send documents to.
    /// * `language`: The language to sync.
    /// * `progress`: The multi-progress bar to display progress.
    ///
    /// returns: Result<SyncPipeline<'a>, Error>
    fn new(
        project: &'a Project,
        sync_target: &'a str,
        language: Option<&'a str>,
        progress: MultiProgress,
    ) -> Result<Self> {
        let global_context = Rc::new(OnceCell::new());
        Ok(SyncPipeline {
            project,
            processors: HashMap::from([
                (
                    FileProcessorType::Markdown,
                    MarkdownProcessor::new(project, sync_target, language, global_context.clone())?
                        .into(),
                ),
                (
                    FileProcessorType::TaskPlugin,
                    TaskProcessor::new(project, language, global_context.clone())?.into(),
                ),
                (
                    FileProcessorType::StyleTheme,
                    StyleThemeProcessor::new(
                        project,
                        sync_target,
                        language,
                        global_context.clone(),
                    )?
                    .into(),
                ),
            ]),
            sync_target,
            language,
            progress,
            global_context,
        })
    }

    /// Step 1: Collect all files in the project and add them to the relevant processors.
    fn collect_tim_documents(&mut self) -> Result<()> {
        let progress = self.progress.add(ProgressBar::new_spinner());
        progress.set_message("Collecting files");
        progress.enable_steady_tick(Duration::from_millis(100));

        let root = self.project.get_root_path();
        let ignores = self.project.ignore_file()?;

        let project_files = WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| !is_hidden(e) && !ignores.is_ignored(e.path()))
            .filter_map(|e| e.ok().map(|e| e.path().to_path_buf()))
            .filter(|e| e.is_file())
            .filter_map(|e| ProjectFile::try_from(e.clone()).map(|p| (p, e)).ok());

        // Group files by their normalized path (without language code)
        // Store (ProjectFile, has_language_code) for each normalized path
        let mut file_map: HashMap<PathBuf, (ProjectFile, bool)> = HashMap::new();

        for (file, path) in project_files {
            let lang_code = path.extract_language_code();

            // Check if this file has the target language code
            let is_target_lang = if let Some(code) = &lang_code {
                self.language.map_or(false, |lang| code == lang)
            } else {
                false
            };

            // Get normalized path (without language code)
            let normalized_path = file.path().clone();

            match file_map.entry(normalized_path) {
                Entry::Occupied(mut entry) => {
                    let (_existing_file, existing_has_lang) = entry.get();

                    // Replace if: new file has target language AND existing doesn't have language
                    // This gives priority to language files over default files
                    if is_target_lang && !existing_has_lang {
                        entry.insert((file, true));
                    }
                    // Otherwise skip (keep existing file)
                }
                Entry::Vacant(entry) => {
                    // Insert if no language is set OR file has no language code OR file has target language
                    let should_insert =
                        self.language.is_none() || lang_code.is_none() || is_target_lang;

                    if should_insert {
                        entry.insert((file, lang_code.is_some()));
                    }
                }
            }
        }

        // Process selected files
        for (file, _) in file_map.into_values() {
            let processor_type = file.processor_type();
            if let Some(processor) = self.processors.get_mut(&processor_type) {
                processor.add_file(file)?;
            }
        }

        progress.finish_and_clear();
        self.progress.remove(&progress);

        Ok(())
    }

    /// Step 3: Collect all documents from the processors.
    fn get_tim_documents(&self) -> Vec<TIMDocument> {
        self.processors
            .values()
            .flat_map(|processor| processor.get_tim_documents())
            .collect()
    }

    /// Step 3: Create the documents and folders in TIM.
    ///
    /// The items are created in the correct order, i.e. folders are created before documents.
    /// This is done to prevent any concurrency errors and to provide sanity checking.
    /// At the same time, the item IDs are collected so that they can be used in templates.
    async fn create_tim_documents(
        &self,
        client: &TimClient,
        documents: Vec<TIMDocument<'a>>,
    ) -> Result<Vec<TIMDocument<'a>>> {
        let progress = self.progress.add(ProgressBar::new_spinner());
        progress.set_message("Creating documents in TIM");
        progress.enable_steady_tick(Duration::from_millis(100));

        let progress_bar = self.progress.add(
            ProgressBar::new(documents.len() as u64).with_style(
                ProgressStyle::default_bar()
                    .template("{msg} [{wide_bar}] {pos:>3}/{len:3}")
                    .unwrap()
                    .progress_chars("##-"),
            ),
        );

        let sync_target = self.project.config.get_target(self.sync_target).unwrap();
        let mut result: Vec<ItemEntry> = Vec::with_capacity(documents.len());

        let tim_folder_root = sync_target.folder_root.clone();
        let tim_folder_root_length = tim_folder_root.len();

        let mut process_stack: LinkedList<(String, Vec<ItemEntry>)> = LinkedList::new();
        let mut item_id_hashmap = HashMap::new();
        let mut original_id_hashmap = HashMap::new();

        let current_path = tim_folder_root;
        let documents_with_paths = documents
            .into_iter()
            .map(|doc| ItemEntry {
                path_base: "",
                path_rest: doc.path,
                doc,
            })
            .collect::<Vec<_>>();
        process_stack.push_front((current_path, documents_with_paths));

        // Helper function to create item or translation
        async fn create_item(
            progress_bar: &ProgressBar,
            client: &TimClient,
            item_type: ItemType,
            path: String,
            title: &str,
            language: Option<&str>,
            main_language: Option<&str>,
        ) -> Result<(String, u64, Option<u64>)> {
            progress_bar.set_message(format!("Creating item: {}", path));

            // If language is set and it is a document, we create a translation
            // Otherwise we create a normal item
            // Folders are always created normally
            if let (Some(lang), ItemType::Document) = (language, &item_type) {
                // For translations, the path is already the base path (normalized)
                // We need to get the original document ID and append /{lang} for the translation path
                let original_item = client.get_item_info(&path).await.context(format!(
                    "Original document {} not found. Cannot create translation.",
                    path
                ))?;

                let translation_path = format!("{}/{}", path, lang);
                let info = client
                    .create_or_update_translation(original_item.id, &translation_path, lang, title)
                    .await?;

                progress_bar.inc(1);
                Ok((translation_path, info.id, Some(original_item.id)))
            } else {
                let item_info = client
                    .create_or_update_item(item_type, &path, title, main_language)
                    .await?;
                progress_bar.inc(1);
                Ok((path, item_info.id, None))
            }
        }

        while let Some((current_path, documents_with_paths)) = process_stack.pop_front() {
            let mut split_documents_paths = documents_with_paths
                .into_iter()
                .map(|de| {
                    let (new_prefix, new_suffix) =
                        de.path_rest.split_once("/").unwrap_or((de.path_rest, ""));
                    ItemEntry {
                        path_base: new_prefix,
                        path_rest: new_suffix,
                        doc: de.doc,
                    }
                })
                .collect::<Vec<_>>();

            let mut futures = Vec::new();

            // Sort by base to bring together items with the same base path
            split_documents_paths.sort_unstable_by_key(|de| de.path_base);

            // Chunk (i.e. group) by path base
            for (base, chunk) in &split_documents_paths
                .into_iter()
                .chunk_by(|de| de.path_base)
            {
                // Each chunk represents the items with the same base
                // There are two main options:
                // 1. All items have the rest part => they represent folders
                // 2. There is only one item without the rest part => it represents a document

                let mut items = chunk.collect::<Vec<_>>();

                // Sanity checking
                if items.len() > 1 {
                    let document_count = items.iter().filter(|de| de.is_document_path()).count();
                    let folder_count = items.len() - document_count;

                    // 1. If there are multiple documents and no folders, user is trying to create multiple documents with the same name in the same folder
                    if document_count > 1 && folder_count == 0 {
                        return Err(SyncError::ItemNameConflict(base.to_string()).into());
                    }

                    // 2. If there are folders and documents, user is trying to create a folder and a document with the same path
                    if document_count > 0 && folder_count > 0 {
                        return Err(SyncError::ItemTypeConflict(base.to_string()).into());
                    }
                }

                // After sanity checks, items will either be a single document
                // or a list of documents to create in the folder
                let item_entries = {
                    if items.len() == 1 {
                        let doc_entry = items.swap_remove(0);
                        if doc_entry.is_document_path() {
                            ItemEntries::Document(doc_entry)
                        } else {
                            ItemEntries::DocumentsInFolder(vec![doc_entry])
                        }
                    } else {
                        ItemEntries::DocumentsInFolder(items)
                    }
                };

                // Finally, create the relevant item and add possible subitems to the process stack
                match item_entries {
                    ItemEntries::Document(doc_entry) => {
                        let doc_path = format!("{}/{}", current_path, base);

                        futures.push(create_item(
                            &progress_bar,
                            client,
                            ItemType::Document,
                            doc_path,
                            doc_entry.doc.title,
                            self.language,
                            sync_target.main_language.as_deref(),
                        ));

                        result.push(doc_entry);
                    }
                    ItemEntries::DocumentsInFolder(folder_entries) => {
                        let folder_path = format!("{}/{}", current_path, base);

                        futures.push(create_item(
                            &progress_bar,
                            client,
                            ItemType::Folder,
                            folder_path.clone(),
                            base,
                            None,
                            None,
                        ));

                        process_stack.push_front((folder_path, folder_entries));
                    }
                }
            }

            // Before going deeper, evaluate all futures (create items for the current level)
            // and collect the resulting IDs to be merged with the documents
            let item_create_results = try_join_all(futures).await?;

            for (path, item_id, original_id) in item_create_results {
                // Convert full path back to item_path that can be used for item ID lookup
                // If language is set, we need to strip it from the path key in the map
                // because the document path in `TIMDocument` does NOT contain the language part
                let item_path = path[tim_folder_root_length + 1..].to_string();

                let item_path_key = if let Some(lang) = self.language {
                    if item_path.ends_with(&format!("/{}", lang)) {
                        item_path[..item_path.len() - lang.len() - 1].to_string()
                    } else {
                        item_path
                    }
                } else {
                    item_path
                };

                item_id_hashmap.insert(item_path_key.clone(), item_id);
                if let Some(oid) = original_id {
                    original_id_hashmap.insert(item_path_key, oid);
                }
            }
        }

        // Finally, obtain back the created documents and insert the document IDs
        Ok(result
            .into_iter()
            .map(|mut ie| {
                ie.doc.id = item_id_hashmap
                    .get(ie.doc.path)
                    .map(|id| Some(*id))
                    .unwrap();
                ie.doc.original_id = original_id_hashmap.get(ie.doc.path).copied();
                ie.doc
            })
            .collect())
    }

    /// Step 4: Update project context to include a full list of documents with their IDs.
    fn update_project_context(&self, documents: &Vec<TIMDocument<'a>>) -> Result<()> {
        let mut uid_to_info_map = Map::new();
        let mut all_documents_infos = Vec::new();

        for doc in documents {
            let general_meta = doc.general_metadata()?;

            let mut doc_meta_json = doc.front_matter_json()?;
            doc_meta_json.merge(&json!({
               "doc_id": doc.id,
               "original_doc_id": doc.original_id,
                "path": doc.path,
                "title": doc.title,
                "local_file_path": doc.get_local_file_path(),
            }));

            if let Some(doc_uid) = general_meta.uid {
                uid_to_info_map.insert(doc_uid, doc_meta_json.clone());
            }

            all_documents_infos.push(doc_meta_json.clone());
        }

        let mut global_context = self.project.global_context()?;
        global_context.insert("doc", Value::Object(uid_to_info_map));
        global_context.insert("docs", Value::Array(all_documents_infos));

        let sync_target = self.project.config.get_target(self.sync_target).unwrap();
        global_context.insert("host", Value::String(sync_target.host.clone()));
        global_context.insert("base_path", Value::String(sync_target.folder_root.clone()));
        global_context.insert(
            "local_project_dir",
            Value::String(self.project.get_root_path().display().to_string()),
        );
        global_context.insert("sync_target", Value::String(self.sync_target.to_string()));

        for (_, processor) in &self.processors {
            if let Some(context) = processor.get_processor_context() {
                global_context.extend(context);
            }
        }

        self.global_context
            .set(global_context)
            .expect("Global context was already set, this should not happen");

        Ok(())
    }

    /// Step 5: Generate documents content and sync them with TIM.
    async fn sync_tim_documents_contents(
        &self,
        client: &TimClient,
        documents: Vec<TIMDocument<'a>>,
    ) -> Result<()> {
        let progress = self.progress.add(ProgressBar::new_spinner());
        progress.set_message("Uploading document contents to TIM");
        progress.enable_steady_tick(Duration::from_millis(100));

        let progress_bar = self.progress.add(
            ProgressBar::new(documents.len() as u64).with_style(
                ProgressStyle::default_bar()
                    .template("{msg} [{wide_bar}] {pos:>3}/{len:3}")
                    .unwrap()
                    .progress_chars("##-"),
            ),
        );

        let sync_target = self.project.config.get_target(self.sync_target).unwrap();
        let tim_folder_root = sync_target.folder_root.clone();

        try_join_all(documents.iter().map(|doc| async {
            let mut doc_path = format!("{}/{}", tim_folder_root, doc.path);

            // Add language code
            if let Some(lang) = self.language {
                doc_path.push_str(&format!("/{}", lang));
            }

            progress_bar.set_message(format!("Uploading document: {}", doc_path));

            // Check if document is marked as external
            // If so, we skip the upload process completely
            // We still create the document (which happened in the previous step),
            // but we don't upload the content
            let front_matter = doc.front_matter_json()?;
            if let Some(true) = front_matter.get("external").and_then(|v| v.as_bool()) {
                info!("Skipping upload for external document: {}", doc_path);
                progress_bar.inc(1);
                return Ok(());
            }

            let prepared_doc = doc.render_contents()?;

            // Upload files
            if !prepared_doc.upload_files.is_empty() {
                let existing_files = client.get_document_uploads(&doc_path).await?;
                let existing_files = existing_files
                    .into_iter()
                    .map(|f| f.filename)
                    .collect::<HashSet<_>>();

                // TODO: Parallelize file uploads
                for (file_path, tim_file_name) in prepared_doc.upload_files.iter() {
                    // Don't re-upload files that already exist
                    if existing_files.contains(tim_file_name) {
                        continue;
                    }
                    client
                        .upload_file(&doc_path, file_path, tim_file_name)
                        .await?;
                }
            }

            let current_doc_markdown = client.download_markdown(&doc_path).await?;

            if !prepared_doc.timestamp_equals(&current_doc_markdown) {
                let doc_markdown = prepared_doc.with_timestamp();
                client
                    .upload_markdown(&doc_path, &doc_markdown.markdown)
                    .await?;
            }

            progress_bar.inc(1);

            Ok::<(), Error>(())
        }))
        .await
        .context("Could not sync documents")?;

        Ok(())
    }
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

    let multi_progress = MultiProgress::new();

    let tick_progress = multi_progress.add(ProgressBar::new_spinner());

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

    let folder_root_info = match client.get_item_info(&target_info.folder_root).await {
        Ok(info) => info,
        Err(e) => {
            return match e.downcast_ref::<TimClientErrors>() {
                Some(TimClientErrors::ItemNotFound(_, _)) => {
                    let tim_url = format!("{}/{}", target_info.host, target_info.folder_root);
                    Err(SyncError::SyncTargetDoesNotExist(tim_url).into())
                }
                _ => Err(e),
            }
        }
    };
    match folder_root_info.item_type {
        ItemType::Folder => (),
        _ => {
            let tim_url = format!("{}/{}", target_info.host, target_info.folder_root);
            return Err(SyncError::SyncTargetNotAFolder(tim_url).into());
        }
    }

    tick_progress.disable_steady_tick();
    tick_progress.set_message("Uploading project");

    let mut pipeline = SyncPipeline::new(
        &project,
        &opts.target,
        opts.language.as_deref(),
        multi_progress,
    )?;
    pipeline.collect_tim_documents()?;
    let documents = pipeline.get_tim_documents();
    let documents = pipeline.create_tim_documents(&client, documents).await?;
    pipeline.update_project_context(&documents)?;
    pipeline
        .sync_tim_documents_contents(&client, documents)
        .await?;

    info!(
        "{} Syncing complete! View the documents at {}/view/{}",
        LogIcon::Tick,
        target_info.host,
        target_info.folder_root
    );

    Ok(())
}
