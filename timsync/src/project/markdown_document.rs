use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lazy_regex::regex;
use markdown::mdast::{Node, Root, Yaml};
use markdown::{Constructs, ParseOptions};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::Digest;
use sha1::Sha1;
use url::{ParseError, Url};

use crate::util::templating::ExtendableContext;

/// A single Markdown document in the project
pub struct MarkdownDocument {
    /// Absolute path to the document
    pub path: PathBuf,
    contents: String,
    mdast: Root,
}

#[derive(Debug, Deserialize)]
/// Settings for a document
/// The settings are stored in the front matter of the document
pub struct DocumentSettings {
    /// The human-readable title of the document
    /// The title is displayed in the navigation bar of TIM
    pub title: Option<String>,
}

// TODO: Use &String instead
struct DocumentLink<'a>(usize, usize, &'a String);

impl MarkdownDocument {
    /// Reads a markdown document from the given path.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to the markdown file
    ///
    /// returns: Result<MarkdownDocument, Error>
    pub fn read_from(path: &PathBuf) -> Result<Self> {
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Could not read markdown file: {}", path.display()))?;

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

        Ok(Self {
            path: path.to_path_buf(),
            contents,
            mdast: root,
        })
    }

    fn find_links(&self) -> Vec<DocumentLink> {
        let mut result: Vec<DocumentLink> = Vec::new();
        fn find_impl<'a>(result: &mut Vec<DocumentLink<'a>>, children: &'a Vec<Node>) {
            for child in children {
                match child {
                    Node::Link(link) => {
                        let pos = link.position.as_ref().unwrap();
                        let url_end = pos.end.offset - 1;
                        let url_start = url_end - link.url.len();
                        result.push(DocumentLink(url_start, url_end, &link.url));
                    }
                    _ => {
                        if let Some(children) = child.children() {
                            find_impl(result, children);
                        }
                    }
                }
            }
        }

        find_impl(&mut result, &self.mdast.children);

        // Sort by start position
        result.sort_unstable_by_key(|link| link.0);

        result
    }

    fn find_front_matter(&self) -> Option<&Yaml> {
        let res = self.mdast.children.iter().find(|node| match node {
            Node::Yaml(_) => true,
            _ => false,
        });

        match res {
            Some(Node::Yaml(yaml)) => Some(yaml),
            _ => None,
        }
    }

    /// Returns the front matter of the document as a DocumentSettings struct.
    /// Returns None if the front matter could not be parsed or if there is no front matter.
    ///
    /// returns: Option<DocumentSettings>
    pub fn settings(&self) -> Option<DocumentSettings> {
        // TODO: This should return Result instead
        let yaml = self.find_front_matter()?;
        let settings = serde_yaml::from_str(&yaml.value).ok()?;
        Some(settings)
    }

    /// Returns the front matter of the document as a serde_json::Value.
    /// Returns None if the front matter could not be parsed or if there is no front matter.
    ///
    /// returns: Option<Value>
    pub fn front_matter_json(&self) -> Option<Value> {
        // TODO: This should return Result instead
        let yaml = self.find_front_matter()?;
        let front_matter = serde_yaml::from_str(&yaml.value).ok()?;
        Some(front_matter)
    }

    /// Converts the markdown document to a TIM markdown document.
    /// This transforms the markdown as follows:
    ///
    /// - Removes the front matter
    /// - Resolves relative links to absolute links
    ///
    /// # Arguments
    ///
    /// * `project_dir`: The absolute path to the project directory
    /// * `root_url`: The root URL of the TIM target folder
    /// * `global_context`: The global context to use for rendering the markdown
    ///
    /// returns: String
    pub fn to_tim_markdown(
        &self,
        project_dir: &Path,
        root_url: &String,
        global_context: Option<&handlebars::Context>,
        renderer: &handlebars::Handlebars,
    ) -> Result<PreparedMarkdown> {
        let mut res = self.contents.clone();
        let mut start_offset = 0isize;

        if let Some(front_matter) = self.find_front_matter() {
            if let Some(pos) = &front_matter.position {
                let (start, end) = (pos.start.offset, pos.end.offset);
                res.replace_range(start..end, "");
                start_offset = start as isize - end as isize;
            }
        }

        let links = self.find_links();
        let project_url_str = Url::from_directory_path(project_dir).unwrap().to_string();
        for DocumentLink(start, end, url) in links {
            let parse_result = Url::parse(&url);
            match parse_result {
                Err(ParseError::RelativeUrlWithoutBase) => {
                    let (fixed_url, base_url) = if url.starts_with("/") {
                        let url = &url[1..];
                        (url, Url::from_directory_path(project_dir).unwrap())
                    } else {
                        (url.as_str(), Url::from_file_path(&self.path).unwrap())
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
                    res.replace_range(start..end, &final_url);

                    // Update the start offset
                    start_offset += final_url.len() as isize - (end as isize - start as isize);
                }
                _ => {
                    continue;
                }
            }
        }

        let mut ctx = handlebars::Context::wraps(json!("{}")).unwrap();
        if let Some(global_context) = global_context {
            ctx.extend_with_json(global_context.data());
        }
        let front_matter_ctx = self.front_matter_json();
        if let Some(front_matter_ctx) = front_matter_ctx {
            ctx.extend_with_json(&front_matter_ctx);
        }

        // Init default tera instance if none is given
        res = renderer
            .render_template_with_context(&res, &ctx)
            .with_context(|| format!("Could not render markdown file {}", self.path.display()))?;

        Ok(res.into())
    }
}

/// A markdown document that is ready to be uploaded to TIM.
pub struct PreparedMarkdown(String);

impl PreparedMarkdown {
    /// Calculates the SHA1 hash of the markdown.
    /// This is used to check if the markdown has changed.
    ///
    /// returns: String
    pub fn sha1(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.update(self.0.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// Prepends the timestamp to the markdown.
    /// The timestamp is stored in the settings block of the markdown.
    ///
    /// returns: PreparedMarkdown
    pub fn with_timestamp(&self) -> PreparedMarkdown {
        let sha1 = self.sha1();
        // prepend the timestamp to the markdown
        Self(format!(
            "{}\n\n{}",
            TimSyncDocSettings::new(sha1).to_markdown(),
            self.0
        ))
    }

    /// Checks if the timestamp in the markdown equals the hash in the given markdown.
    ///
    /// # Arguments
    ///
    /// * `md`: The markdown to check
    ///
    /// returns: bool
    pub fn timestamp_equals(&self, md: &str) -> bool {
        // Try to find the settings in the markdown with regex
        let re = regex!(r#"```\s*\{\s*?settings="timsync".*?\}\n(?P<settings>(?:.|\s)*?)```"#);
        let result = re.captures(md);
        match result {
            Some(captures) => {
                let settings_str = captures.name("settings").unwrap().as_str();
                TimSyncDocSettings::from_yaml(settings_str)
                    .map(|settings| settings.hash == self.sha1())
                    .unwrap_or(false)
            }
            None => false,
        }
    }
}

impl From<PreparedMarkdown> for String {
    fn from(markdown: PreparedMarkdown) -> Self {
        markdown.0
    }
}

impl Deref for PreparedMarkdown {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for PreparedMarkdown {
    fn from(markdown: String) -> Self {
        Self(markdown)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct TimSyncDocSettings {
    hash: String,
}

impl TimSyncDocSettings {
    fn new(hash: String) -> Self {
        Self { hash }
    }

    fn from_yaml(yaml: &str) -> Result<Self> {
        let settings: Self =
            serde_yaml::from_str(&yaml).with_context(|| "Could not parse timsync docsettings")?;
        Ok(settings)
    }

    fn to_markdown(&self) -> String {
        let yaml_str = serde_yaml::to_string(&self).unwrap();
        format!("``` {{settings=\"timsync\"}}\n{}```\n", yaml_str)
    }
}
