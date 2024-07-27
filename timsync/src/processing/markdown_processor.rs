use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, Result};
use handlebars::Handlebars;
use markdown::mdast::Node;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use url::{ParseError, Url};

use crate::processing::prepared_markdown::PreparedDocumentMarkdown;
use crate::processing::processors::{FileProcessorAPI, FileProcessorInternalAPI};
use crate::processing::tim_document::TIMDocument;
use crate::project::files::markdown_file::MarkdownFile;
use crate::project::files::project_files::{
    GeneralProjectFileMetadata, ProjectFile, ProjectFileAPI,
};
use crate::project::global_ctx::GlobalContext;
use crate::project::project::Project;
use crate::util::path::{Relativize, WithSetExtension};
use crate::util::templating::{ExtendableContext, TimRendererExt};

/// Helper struct to store metadata about a document and a reference to the
/// file in the project folder.
struct TIMDocInfo {
    path: Rc<str>,
    title: Rc<str>,
    proj_file: ProjectFile,
}

#[derive(Debug, Deserialize)]
/// Settings for a document
/// The settings are stored in the front matter of the document
pub struct DocumentSettings {
    /// The human-readable title of the document
    /// The title is displayed in the navigation bar of TIM
    pub title: Option<String>,

    /// The path of the document in TIM
    /// If not specified, the path of the file will be used
    pub tim_path: Option<String>,
}

/// Processor for markdown files.
pub struct MarkdownProcessor<'a> {
    /// Map of all files to process with their metadata.
    /// Keyed using the final path of the document in TIM.
    files: HashMap<Rc<str>, TIMDocInfo>,

    /// Reference to the project that is being processed.
    project: &'a Project,

    /// Sync target to which the documents are being synced.
    sync_target: String,

    /// Handlebars renderer to render the Markdown files.
    renderer: Handlebars<'a>,

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
        let mut renderer = Handlebars::new().with_tim_doc_templates();
        for (name, template) in project.get_template_files()? {
            renderer.register_template_file(&name, template)?;
        }
        Ok(Self {
            files: HashMap::new(),
            project,
            sync_target: sync_target.to_string(),
            renderer,
            global_context,
        })
    }

    /// Find all links in a Markdown document.
    ///
    /// # Arguments
    ///
    /// * `md_file` - The Markdown file to search for links.
    ///
    /// Returns: Vec<DocumentLink>
    fn find_links(&self, md_file: &MarkdownFile) -> Vec<DocumentLink> {
        let mut result: Vec<DocumentLink> = Vec::new();
        fn find_impl(result: &mut Vec<DocumentLink>, children: &Vec<Node>) {
            for child in children {
                match child {
                    Node::Link(link) => {
                        let pos = link.position.as_ref().unwrap();
                        let url_end = pos.end.offset - 1;
                        let url_start = url_end - link.url.len();
                        result.push(DocumentLink(url_start, url_end, link.url.clone()));
                    }
                    _ => {
                        if let Some(children) = child.children() {
                            find_impl(result, children);
                        }
                    }
                }
            }
        }

        let mdast = md_file.md_ast().unwrap();

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
    fn resolve_relative_urls(
        &self,
        contents: &mut String,
        project_dir: &Path,
        proj_file_path: &PathBuf,
        root_url: &String,
        md_file: &MarkdownFile,
    ) {
        let links = self.find_links(md_file);
        let mut start_offset = 0isize;

        for DocumentLink(start, end, url) in links {
            let parse_result = Url::parse(&url);
            let project_url_str = Url::from_directory_path(project_dir).unwrap().to_string();

            match parse_result {
                Err(ParseError::RelativeUrlWithoutBase) => {
                    let (fixed_url, base_url) = if url.starts_with("/") {
                        let url = &url[1..];
                        (url, Url::from_directory_path(project_dir).unwrap())
                    } else {
                        (url.as_str(), Url::from_file_path(proj_file_path).unwrap())
                    };
                    let mut joined = base_url.join(fixed_url).unwrap();

                    let path_part = joined.path().to_string();

                    if path_part.ends_with(".md") {
                        joined.set_path(&path_part[..path_part.len() - 3]);
                    }

                    let final_url = joined.to_string().replace(&project_url_str, "");
                    let final_url = format!("/view/{}/{}", root_url, final_url);

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
    fn render_tim_document(&self, tim_document: &TIMDocument) -> Result<PreparedDocumentMarkdown> {
        // This unwrap is safe because the file was added to the processor
        // Because internal API is only called by TIMDocument, the file should always exist
        let info = self.files.get(tim_document.path).unwrap();

        let mut contents = info.proj_file.contents_without_front_matter()?.to_string();
        let project_dir = self.project.get_root_path();
        let proj_file_path = info.proj_file.path();
        let root_url = &self
            .project
            .config
            .get_target(&self.sync_target)
            .ok_or_else(|| anyhow::anyhow!("Could not find target: {}", self.sync_target))?
            .folder_root;

        // TODO: Remove when other types are supported
        #[allow(irrefutable_let_patterns)]
        if let ProjectFile::Markdown(md_file) = &info.proj_file {
            self.resolve_relative_urls(
                &mut contents,
                project_dir,
                proj_file_path,
                root_url,
                md_file,
            );
        }

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
            "local_file_path": proj_file_path.to_string_lossy(),
        }));

        let res = self
            .renderer
            .render_template_with_context(&contents, &ctx)
            .with_context(|| {
                format!(
                    "Could not render markdown document: {}",
                    proj_file_path.display()
                )
            })?;

        Ok(res.into())
    }

    fn get_project_file_metadata(
        &self,
        tim_document: &TIMDocument,
    ) -> Result<GeneralProjectFileMetadata> {
        // This unwrap is safe because the file was added to the processor
        // Because internal API is only called by TIMDocument, the file should always exist
        let info = self.files.get(tim_document.path).unwrap();
        info.proj_file.read_general_metadata()
    }
}
