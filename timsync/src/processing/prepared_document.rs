use std::collections::HashMap;

use anyhow::Context;
use lazy_regex::regex;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

/// A Markdown document contents that are ready to be uploaded to TIM.
pub struct PreparedDocument {
    /// Markdown contents of the document
    pub markdown: String,
    /// Map of files to upload.
    /// Keys are full resolved paths to the files, values are final filenames of the files in TIM
    pub upload_files: HashMap<String, String>,
}

impl PreparedDocument {
    /// Calculates the SHA1 hash of the markdown.
    /// This is used to check if the markdown has changed.
    ///
    /// returns: String
    pub fn sha1(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.update(self.markdown.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// Prepends the timestamp to the markdown.
    /// The timestamp is stored in the settings block of the markdown.
    ///
    /// returns: PreparedMarkdown
    pub fn with_timestamp(self) -> PreparedDocument {
        let sha1 = self.sha1();
        // prepend the timestamp to the markdown
        Self {
            markdown: format!(
                "{}\n\n{}",
                TimSyncDocSettings::new(sha1).to_markdown(),
                self.markdown
            ),
            upload_files: self.upload_files,
        }
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

#[derive(Debug, Deserialize, Serialize)]
struct TimSyncDocSettings {
    hash: String,
}

impl TimSyncDocSettings {
    fn new(hash: String) -> Self {
        Self { hash }
    }

    fn from_yaml(yaml: &str) -> anyhow::Result<Self> {
        let settings: Self =
            serde_yaml::from_str(&yaml).with_context(|| "Could not parse timsync docsettings")?;
        Ok(settings)
    }

    fn to_markdown(&self) -> String {
        let yaml_str = serde_yaml::to_string(&self).unwrap();
        format!("``` {{settings=\"timsync\"}}\n{}```\n", yaml_str)
    }
}
