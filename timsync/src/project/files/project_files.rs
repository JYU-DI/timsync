use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::processing::processors::FileProcessorType;
use crate::project::files::markdown_file::MarkdownFile;

/// Enum representing the different types of project files.
/// Used as an abstraction over all available project file implementations.
pub enum ProjectFile {
    /// Markdown file.
    Markdown(MarkdownFile),
}

impl TryFrom<PathBuf> for ProjectFile {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> std::result::Result<Self, Self::Error> {
        let ext = path
            .extension()
            .ok_or(anyhow::anyhow!("No extension"))?
            .to_str()
            .ok_or(anyhow::anyhow!("Could not convert extension to string"))?;

        match ext {
            "md" => Ok(ProjectFile::Markdown(MarkdownFile::new(path))),
            _ => Err(anyhow::anyhow!("No matching file for extension: {}", ext)),
        }
    }
}

/// Public API for the project files.
pub trait ProjectFileAPI {
    /// Get the path of the project file.
    fn path(&self) -> &PathBuf;
    /// Get the position of the front matter in the project file.
    fn front_matter_pos(&self) -> Option<(usize, usize)>;
    /// Get the contents of the project file.
    fn contents(&self) -> Result<&str>;
    /// Get the processor type to use for the project file.
    fn processor_type(&self) -> FileProcessorType;
}

impl ProjectFileAPI for ProjectFile {
    fn path(&self) -> &PathBuf {
        match self {
            ProjectFile::Markdown(file) => file.path(),
        }
    }

    fn front_matter_pos(&self) -> Option<(usize, usize)> {
        match self {
            ProjectFile::Markdown(file) => file.front_matter_pos(),
        }
    }

    fn contents(&self) -> Result<&str> {
        match self {
            ProjectFile::Markdown(file) => file.contents(),
        }
    }

    fn processor_type(&self) -> FileProcessorType {
        match self {
            ProjectFile::Markdown(file) => file.processor_type(),
        }
    }
}

impl dyn ProjectFileAPI {
    /// Get the contents of the project file without the front matter.
    ///
    /// Returns: Result<&str>
    pub fn contents_without_front_matter(&self) -> Result<&str> {
        let contents = self.contents()?;
        match self.front_matter_pos() {
            Some((_, end)) => Ok(&contents[end..]),
            None => Ok(contents),
        }
    }
}

// TODO: Do we need to even allow a custom processor?
#[derive(Debug, Deserialize)]
pub struct GeneralProjectFileSettings {
    pub processor: Option<String>,
}

impl ProjectFile {
    // TODO: Is this needed?
    #[allow(dead_code)]
    pub fn read_general_settings(&self) -> Result<GeneralProjectFileSettings> {
        let front_matter = self.front_matter();
        match front_matter {
            Ok(front_matter) => {
                let settings: GeneralProjectFileSettings = serde_yaml::from_str(front_matter)
                    .with_context(|| {
                        format!(
                            "Could not parse front matter of file: {}",
                            self.path().display()
                        )
                    })
                    .unwrap();
                Ok(settings)
            }
            _ => Ok(GeneralProjectFileSettings { processor: None }),
        }
    }

    /// Get the front matter of the project file.
    ///
    /// Returns: Result<&str>
    pub fn front_matter(&self) -> Result<&str> {
        let contents = self.contents()?;
        let front_matter_pos = self.front_matter_pos();
        match front_matter_pos {
            Some((start, end)) => {
                // The front matter includes front matter markers as the first and last lines
                // Filter them away to get the actual front matter contents
                // This assumes that the front matter is already trimmed
                let res = &contents[start..end];
                let first_newline = res.find('\n').unwrap_or(0);
                let last_newline = res.rfind('\n').unwrap_or(res.len());
                Ok(&res[first_newline..last_newline])
            }
            None => Ok(""),
        }
    }

    /// Get the parsed front matter of the project file as JSON.
    ///
    /// Returns: Result<Value>
    pub fn front_matter_json(&self) -> Result<Value> {
        let front_matter = self.front_matter().with_context(|| {
            format!(
                "Could not read front matter of file: {}",
                self.path().display()
            )
        })?;
        if front_matter.is_empty() {
            return Ok(Value::Object(serde_json::Map::new()));
        }
        let front_matter = serde_yaml::from_str(&front_matter).with_context(|| {
            format!(
                "Could not parse front matter of file: {}",
                self.path().display()
            )
        })?;
        Ok(front_matter)
    }

    /// Get the contents of the project file without the front matter.
    pub fn contents_without_front_matter(&self) -> Result<&str> {
        let api: &dyn ProjectFileAPI = self;
        api.contents_without_front_matter()
    }
}
