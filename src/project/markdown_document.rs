use anyhow::{Context, Result};
use markdown::mdast::{Node, Root, Yaml};
use markdown::{Constructs, ParseOptions};
use serde::Deserialize;
use std::path::PathBuf;

pub struct MarkdownDocument {
    pub path: PathBuf,
    contents: String,
    mdast: Root,
}

#[derive(Debug, Deserialize)]
pub struct DocumentSettings {
    pub title: Option<String>,
}

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

    pub fn to_tim_markdown(&self) -> String {
        let mut res = self.contents.clone();

        if let Some(front_matter) = self.find_front_matter() {
            if let Some(pos) = &front_matter.position {
                let (start, end) = (pos.start.offset, pos.end.offset);
                res.replace_range(start..end, "");
            }
        }

        res
    }
}
