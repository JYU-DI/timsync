use std::path::PathBuf;

use anyhow::Context;
use serde_json::{Map, Value};

/// The name of the global data config file
pub const GLOBAL_DATA_CONFIG_FILE: &str = "_config.yml";

pub const DEFAULT_GLOBAL_DATA: &str = r#"#
# This config file is meant for settings that affect your whole TIM page. 
# You can access these values throughout all documents by using the `site` variable.
# For example, you can use `{{ site.title }}` to access the title of your page.

# The title of your page
title: My TIM page
"#;

/// A builder for the global context
pub struct GlobalContextBuilder {
    global_data: Map<String, Value>,
}

impl GlobalContextBuilder {
    /// Create a new GlobalContextBuilder
    pub fn new() -> Self {
        Self {
            global_data: Map::new(),
        }
    }

    /// Load global site data from a YAML file.
    /// The date is merged with the existing site data.
    ///
    /// # Arguments
    ///
    /// * `yaml_file`: The path to the YAML file to load
    ///
    /// returns: Result<&GlobalContextBuilder, Error>
    pub fn add_global_data(
        &mut self,
        yaml_file: &PathBuf,
    ) -> anyhow::Result<&GlobalContextBuilder> {
        let yaml_str = std::fs::read_to_string(yaml_file)?;
        let yaml_data: Map<String, Value> = serde_yaml::from_str(&yaml_str)
            .context("Could not parse global data config as a YAML document.")?;
        self.global_data.extend(yaml_data);
        Ok(self)
    }

    /// Build the Tera context from the loaded data.
    ///
    /// returns: Context
    pub fn build(&self) -> tera::Context {
        let mut res: Map<String, Value> = Map::new();
        res.insert("site".to_string(), Value::Object(self.global_data.clone()));
        let ctx = tera::Context::from_value(Value::Object(res)).unwrap();
        ctx
    }
}
