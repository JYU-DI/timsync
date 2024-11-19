use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, Result};
use handlebars::Handlebars;
use markdown::mdast::{Node, Root};
use markdown::{Constructs, ParseOptions};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use url::{ParseError, Url};

use crate::processing::prepared_document::PreparedDocument;
use crate::processing::processors::{FileProcessorAPI, FileProcessorInternalAPI};
use crate::processing::tim_document::TIMDocument;
use crate::project::files::project_files::{ProjectFile, ProjectFileAPI};
use crate::project::global_ctx::GlobalContext;
use crate::project::project::Project;
use crate::templating::ext_context::ContextExtension;
use crate::templating::ext_render_with_context::RendererExtension;
use crate::templating::tim_handlebars::{TimRendererExt, FILE_MAP_ATTRIBUTE};
use crate::util::path::{generate_hashed_filename, RelativizeExtension, WithSetExtension};

/// Helper struct to store metadata about a document and a reference to the
/// file in the project folder.
struct TIMDocInfo {
    path: Rc<str>,
    title: Rc<str>,
    proj_file: ProjectFile,
}

/// Settings for a document
/// The settings are stored in the front matter of the document
#[derive(Debug, Deserialize)]
pub struct DocumentSettings {
    /// The human-readable title of the document
    /// The title is displayed in the navigation bar of TIM
    pub title: Option<String>,

    /// The path of the document in TIM
    /// If not specified, the path of the file will be used
    pub tim_path: Option<String>,
}

/// Processor for markdown files.
/// The processor generates a TIM document for each project file added to the processor.
/// The contents of the file are passed to the templating engine and the result is stored in the TIM document.
pub struct MarkdownProcessor<'a> {
    /// Map of all files to process with their metadata.
    /// Keyed using the final path of the document in TIM.
    files: HashMap<Rc<str>, TIMDocInfo>,

    /// Reference to the project that is being processed.
    pub(in crate::processing) project: &'a Project,

    /// Sync target to which the documents are being synced.
    pub(in crate::processing) sync_target: String,

    /// Handlebars renderer to render the Markdown files.
    renderer: Handlebars<'a>,

    /// Reference to the shared global context of the project.
    global_context: Rc<OnceCell<GlobalContext>>,
}

/// Struct to store a link (relative or absolute) in a Markdown document.
struct DocumentLink(usize, usize, String);

impl<'a> MarkdownProcessor<'a> {
    /// Create a new MarkdownProcessor.
    ///
    /// # Arguments
    ///
    /// * `project` - Reference to the project that is being processed.
    /// * `sync_target` - Sync target to which the documents are being synced.
    ///
    /// Returns: MarkdownProcessor
    pub fn new(
        project: &'a Project,
        sync_target: &str,
        global_context: Rc<OnceCell<GlobalContext>>,
    ) -> Result<Self> {
        let renderer = Handlebars::new()
            .with_tim_doc_helpers()
            .with_project_templates(project)?
            .with_project_helpers(project)?;

        Ok(Self {
            files: HashMap::new(),
            project,
            sync_target: sync_target.to_string(),
            renderer,
            global_context,
        })
    }

    /// Parse the Markdown document into an AST.
    ///
    /// # Arguments
    ///
    /// * `contents` - The contents of the Markdown document.
    ///
    /// Returns: Root
    fn get_md_ast(&self, contents: &str) -> Result<Root> {
        // This cannot fail, see https://docs.rs/markdown/1.0.0-alpha.14/markdown/fn.to_mdast.html
        let mdast = markdown::to_mdast(
            &contents,
            &ParseOptions {
                constructs: Constructs {
                    frontmatter: true,
                    ..Constructs::default()
                },
                ..ParseOptions::default()
            },
        )
        .unwrap();

        let root = match mdast {
            Node::Root(root) => root,
            _ => {
                return Err(anyhow::anyhow!(
                    "Could not parse root node of markdown file"
                ))
            }
        };

        Ok(root)
    }

    /// Find all links in a Markdown document.
    ///
    /// # Arguments
    ///
    /// * `md_file` - The Markdown file to search for links.
    ///
    /// Returns: Vec<DocumentLink>
    fn find_links(&self, contents: &str) -> Vec<DocumentLink> {
        let mut result: Vec<DocumentLink> = Vec::new();
        fn find_impl(result: &mut Vec<DocumentLink>, children: &Vec<Node>) {
            for child in children {
                match child {
                    // Normal link in form [a](b)
                    Node::Link(link) => {
                        let pos = link.position.as_ref().unwrap();
                        let url_end = pos.end.offset - 1;
                        let url_start = url_end - link.url.len();
                        result.push(DocumentLink(url_start, url_end, link.url.clone()));
                    }
                    // Images in form ![a](b)
                    Node::Image(image) => {
                        let pos = image.position.as_ref().unwrap();
                        let url_end = pos.end.offset - 1;
                        let url_start = url_end - image.url.len();
                        result.push(DocumentLink(url_start, url_end, image.url.clone()));
                    }
                    _ => {
                        if let Some(children) = child.children() {
                            find_impl(result, children);
                        }
                    }
                }
            }
        }

        let mdast = self.get_md_ast(contents).unwrap();

        find_impl(&mut result, &mdast.children);

        // Sort by start position
        result.sort_unstable_by_key(|link| link.0);

        result
    }

    /// Rewrite relative URLs in the Markdown document into absolute TIM URLs.
    ///
    /// # Arguments
    ///
    /// * `contents` - The contents of the Markdown document.
    /// * `project_dir` - The root directory of the project.
    /// * `proj_file_path` - The path of the Markdown file.
    /// * `root_url` - The root URL of the target in TIM.
    /// * `md_file` - Information about the Markdown file to process.
    /// * `upload_files_map` - Map of files to upload to TIM and their uploaded filenames.
    fn resolve_relative_urls(
        &self,
        contents: &mut String,
        project_dir: &Path,
        proj_file_path: &PathBuf,
        root_url: &String,
        tim_path: &str,
    ) -> HashMap<String, String> {
        let links = self.find_links(contents);
        let mut start_offset = 0isize;
        let mut upload_files_map = HashMap::new();

        for DocumentLink(start, end, url) in links {
            let parse_result = Url::parse(&url);
            let project_url_str = Url::from_directory_path(project_dir).unwrap().to_string();

            match parse_result {
                Err(ParseError::RelativeUrlWithoutBase) => {
                    let (base_url, path_part) = if url.starts_with("/") {
                        let url = &url[1..];
                        (Url::from_directory_path(project_dir).unwrap(), url)
                    } else {
                        (Url::from_file_path(proj_file_path).unwrap(), url.as_str())
                    };
                    let mut full_url = base_url.join(path_part).unwrap();
                    let path_part = full_url.path().to_string();

                    // TODO: This may not be enough, because we do not know if the
                    //   .md file is being processed as a TIM document or not.
                    //   Also, some other non-Markdown files may be processed as TIM documents.
                    //   We need to check if the file is being processed as a TIM document
                    //   and from there consider whether make it a relative URL or mark it
                    //   as an upload file.
                    let final_url = if path_part.ends_with(".md") {
                        full_url.set_path(&path_part[..path_part.len() - 3]);
                        let final_url = full_url.to_string().replace(&project_url_str, "");
                        format!("/view/{}/{}", root_url, final_url)
                    } else {
                        // Safety: The URL is guaranteed to be a file path, and other
                        // requirements are met for to_file_path to be safe.
                        let full_path = full_url.to_file_path().unwrap();
                        // Try to find and hash the file, otherwise silently skip it
                        let Ok(tim_file_name) = generate_hashed_filename(&full_path) else {
                            continue;
                        };
                        upload_files_map.insert(
                            full_path.to_string_lossy().to_string(),
                            tim_file_name.clone(),
                        );
                        format!("/files/{}/{}/{}", root_url, tim_path, tim_file_name)
                    };

                    // Replace the url in the markdown from the start to the end position
                    let start = (start as isize + start_offset) as usize;
                    let end = (end as isize + start_offset) as usize;
                    contents.replace_range(start..end, &final_url);

                    // Update the start offset
                    start_offset += final_url.len() as isize - (end as isize - start as isize);
                }
                _ => {
                    continue;
                }
            }
        }

        upload_files_map
    }
}

impl<'a> FileProcessorAPI for MarkdownProcessor<'a> {
    fn add_file(&mut self, file: ProjectFile) -> Result<()> {
        let root_path = self.project.get_root_path();

        let document_settings = match file.front_matter() {
            Ok(front_matter) => serde_yaml::from_str::<DocumentSettings>(front_matter)
                .with_context(|| {
                    format!(
                        "Could not parse front matter of file: {}",
                        file.path().display()
                    )
                })?,
            _ => DocumentSettings {
                title: None,
                tim_path: None,
            },
        };

        let title = match document_settings.title {
            Some(title) => title,
            None => file
                .path()
                .file_stem()
                .ok_or_else(|| {
                    anyhow::anyhow!(format!(
                        "Could not get file name from path: {}",
                        file.path().display()
                    ))
                })?
                .to_string_lossy()
                .to_string(),
        };

        let path = match document_settings.tim_path {
            Some(path) => path,
            None => file
                .path()
                .relativize(root_path)
                .with_set_extension("")
                .to_string_lossy()
                .to_string(),
        }
        .replace("\\", "/")
        .to_lowercase();

        let title: Rc<str> = Rc::from(title);
        let path: Rc<str> = Rc::from(path);

        self.files.insert(
            path.clone(),
            TIMDocInfo {
                path,
                title,
                proj_file: file,
            },
        );

        Ok(())
    }

    fn get_processor_context(&self) -> Option<Map<String, Value>> {
        None
    }

    fn get_tim_documents(&self) -> Vec<TIMDocument> {
        self.files
            .values()
            .map(|info| TIMDocument {
                renderer: self,
                title: info.title.as_ref(),
                path: info.path.as_ref(),
                id: None,
            })
            .collect()
    }
}

impl<'a> FileProcessorInternalAPI for MarkdownProcessor<'a> {
    fn render_tim_document(&self, tim_document: &TIMDocument) -> Result<PreparedDocument> {
        // This unwrap is safe because the file was added to the processor
        // Because internal API is only called by TIMDocument, the file should always exist
        let info = self.files.get(tim_document.path).unwrap();

        let contents = info.proj_file.contents_without_front_matter()?.to_string();
        let project_dir = self.project.get_root_path();
        let proj_file_path = info.proj_file.path();
        let root_url = &self
            .project
            .config
            .get_target(&self.sync_target)
            .ok_or_else(|| anyhow::anyhow!("Could not find target: {}", self.sync_target))?
            .folder_root;

        let mut ctx = self
            .global_context
            .get()
            .expect("Global context was not initialized")
            .handlebars_context();
        ctx.extend_with_json(&info.proj_file.front_matter_json()?);
        ctx.extend_with_json(&json!({
            "title": tim_document.title,
            "path": tim_document.path,
            "doc_id": tim_document.id.unwrap_or(0),
            "local_file_path": tim_document.get_local_file_path(),
        }));

        let res = self
            .renderer
            .render_template_with_context_return_new_context(&contents, &ctx)
            .with_context(|| {
                format!(
                    "Could not render markdown document: {}",
                    proj_file_path.display()
                )
            })?;

        // TODO: Make a general context extension for this
        let mut upload_files_map = res
            .modified_context
            .and_then(|c| {
                c.data()
                    .get(FILE_MAP_ATTRIBUTE)
                    .and_then(|v| serde_json::from_value::<HashMap<String, String>>(v.clone()).ok())
            })
            .unwrap_or_default();

        let mut contents = res.rendered;

        // TODO: Remove when other types are supported
        #[allow(irrefutable_let_patterns)]
        if let ProjectFile::Markdown(_) = &info.proj_file {
            // For markdown files, we resolve relative URLs whenever possible.
            // While resolving, we may find additional files to upload
            let additional_upload_files = self.resolve_relative_urls(
                &mut contents,
                project_dir,
                proj_file_path,
                root_url,
                tim_document.path,
            );
            upload_files_map.extend(additional_upload_files);
        }

        Ok(PreparedDocument {
            markdown: contents,
            upload_files: upload_files_map,
        })
    }

    fn get_project_file_front_matter_json(&self, tim_document: &TIMDocument) -> Result<Value> {
        // This unwrap is safe because the file was added to the processor
        // Because internal API is only called by TIMDocument, the file should always exist
        let info = self.files.get(tim_document.path).unwrap();
        info.proj_file.front_matter_json()
    }

    fn get_project_file_local_path(&self, tim_document: &TIMDocument) -> Option<String> {
        // This unwrap is safe because the file was added to the processor
        // Because internal API is only called by TIMDocument, the file should always exist
        let info = self.files.get(tim_document.path).unwrap();
        Some(
            info.proj_file
                .path()
                .relativize(self.project.get_root_path())
                .to_string_lossy()
                .to_string(),
        )
    }
}
