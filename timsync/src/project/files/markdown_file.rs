use std::path::PathBuf;

use anyhow::{Context, Result};
use lazy_init::Lazy;
use markdown::{Constructs, ParseOptions};
use markdown::mdast::{Node, Root};

use crate::processing::processors::FileProcessorType;
use crate::project::files::project_files::ProjectFileAPI;

pub struct MarkdownFile {
    path: PathBuf,
    contents: Lazy<anyhow::Result<String>>,
    front_matter_position: Lazy<Option<(usize, usize)>>,
}

impl ProjectFileAPI for MarkdownFile {
    fn path(&self) -> &PathBuf {
        &self.path
    }

    fn front_matter_pos(&self) -> Option<(usize, usize)> {
        let res = self.front_matter_position.get_or_create(|| {
            let mdast = self.md_ast();
            match mdast {
                Ok(mdast) => {
                    let res = mdast.children.iter().find(|node| match node {
                        Node::Yaml(_) => true,
                        _ => false,
                    });

                    match res {
                        Some(Node::Yaml(yaml)) => Some((
                            yaml.position.as_ref().unwrap().start.offset,
                            yaml.position.as_ref().unwrap().end.offset,
                        )),
                        _ => None,
                    }
                }
                Err(_) => None,
            }
        });

        res.clone()
    }

    fn contents(&self) -> anyhow::Result<&str> {
        let res = self.contents.get_or_create(|| {
            std::fs::read_to_string(&self.path)
                .with_context(|| format!("Could not read file: {}", self.path.display()))
        });

        match res {
            Ok(contents) => Ok(contents.as_str()),
            Err(err) => Err(anyhow::anyhow!(format!("{}", err))),
        }
    }

    fn processor_type(&self) -> FileProcessorType {
        FileProcessorType::Markdown
    }
}

impl MarkdownFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            contents: Lazy::new(),
            front_matter_position: Lazy::new(),
        }
    }

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

    pub fn md_ast(&self) -> Result<Root> {
        let contents = self.contents()?;
        self.get_md_ast(contents)
    }

    pub fn md_ast_contents_only(&self) -> Result<Root> {
        let api: &dyn ProjectFileAPI = self;
        let contents = api.contents_without_front_matter()?;
        self.get_md_ast(contents)
    }
}
