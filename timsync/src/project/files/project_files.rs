use std::path::PathBuf;

use anyhow::{Context, Result};
use enum_dispatch::enum_dispatch;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::processing::processors::FileProcessorType;
use crate::project::files::css_file::CSSFile;
use crate::project::files::markdown_file::MarkdownFile;
use crate::project::files::yaml_file::YAMLFile;
use crate::util::path::{FullExtension, LanguageCodeExtraction};

/// Enum representing the different types of project files.
/// Used as an abstraction over all available project file implementations.
/// The specific implementation of each file type is declared in a separate file.
#[enum_dispatch(ProjectFileAPI)]
pub enum ProjectFile {
    /// Markdown file.
    Markdown(MarkdownFile),
    /// YAML file.
    YAML(YAMLFile),
    /// CSS file.
    CSS(CSSFile),
}

impl TryFrom<PathBuf> for ProjectFile {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> std::result::Result<Self, Self::Error> {
        let lang_code = path.extract_language_code();
        let path = path.strip_language_code();
        let ext = path
            .full_extension()
            .ok_or(anyhow::anyhow!("No extension"))?
            .to_str()
            .ok_or(anyhow::anyhow!("Could not convert extension to string"))?;

        match ext {
            "md" | "markdown" => Ok(MarkdownFile::new(path, lang_code).into()),
            "scss" | "css" => Ok(CSSFile::new(path, lang_code).into()),
            "task.yaml" | "task.yml" => {
                Ok(YAMLFile::new(path, FileProcessorType::TaskPlugin, lang_code).into())
            }
            _ => Err(anyhow::anyhow!("No matching file for extension: {}", ext)),
        }
    }
}

#[enum_dispatch]
/// Public API for the project files.
pub trait ProjectFileAPI {
    /// Get the path of the project file.
    fn path(&self) -> &PathBuf;
    /// Get the language code of the project file.
    fn lang_code(&self) -> Option<&str>;
    /// Get the position of the front matter in the project file.
    fn front_matter_pos(&self) -> Option<(usize, usize)>;
    /// Get the contents of the project file.
    fn contents(&self) -> Result<&str>;
    /// Get the processor type to use for the project file.
    fn processor_type(&self) -> FileProcessorType;
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

    /// Get the normalized path of the project file (i.e. without language code).
    ///
    /// Returns: PathBuf
    fn normalized_path(&self) -> PathBuf {
        self.path().strip_language_code()
    }

    /// Get the full path of the project file (i.e. with language code if there is one).
    ///
    /// Returns: PathBuf
    pub fn full_path(&self) -> PathBuf {
        match self.lang_code() {
            Some(lang_code) => self.path().with_language_code(lang_code),
            None => self.path().clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GeneralProjectFileMetadata {
    // TODO: Check if needed, technically we can allow any type to specify a custom processor
    #[allow(dead_code)]
    pub processor: Option<String>,
    pub uid: Option<String>,
}

impl ProjectFile {
    pub fn read_general_metadata(&self) -> Result<GeneralProjectFileMetadata> {
        let Ok(front_matter) = self.front_matter() else {
            return Ok(GeneralProjectFileMetadata {
                processor: None,
                uid: None,
            });
        };
        let settings: GeneralProjectFileMetadata = serde_yaml::from_str(front_matter)
            .with_context(|| {
                format!(
                    "Could not parse front matter of file: {}",
                    self.path().display()
                )
            })
            .unwrap();
        Ok(settings)
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
            return Ok(Value::Object(Map::new()));
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
