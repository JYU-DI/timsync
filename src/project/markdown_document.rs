use anyhow::{Context, Result};
use markdown::mdast::{Node, Root, Yaml};
use markdown::{Constructs, ParseOptions};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use url::{ParseError, Url};

pub struct MarkdownDocument {
    pub path: PathBuf,
    contents: String,
    mdast: Root,
}

#[derive(Debug, Deserialize)]
pub struct DocumentSettings {
    pub title: Option<String>,
}

// TODO: Use &String instead
struct DocumentLink<'a>(usize, usize, &'a String);

impl MarkdownDocument {
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

    pub fn front_matter(&self) -> Option<DocumentSettings> {
        let yaml = self.find_front_matter()?;
        let settings = serde_yaml::from_str(&yaml.value).ok()?;
        Some(settings)
    }

    pub fn to_tim_markdown(&self, project_dir: &Path, root_url: &String) -> String {
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

        res
    }
}
