use std::path::PathBuf;

use anyhow::{Context, Result};
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

/// Global context that contains that is shared across all documents.
/// Used primarily for templating, but can be used for sharing data between processors in general.
#[derive(Debug)]
pub struct GlobalContext {
    global_data: Map<String, Value>,
}

impl GlobalContext {
    /// Create a new GlobalContextBuilder
    pub fn new() -> Self {
        Self {
            global_data: Map::new(),
        }
    }

    /// Create a new GlobalContextBuilder and preload the global data from a YAML file.
    ///
    /// # Arguments
    ///
    /// * `project_path`: The path to the project directory
    ///
    /// returns: Result<Self, Error>
    pub fn for_project(project_path: &PathBuf) -> Result<Self> {
        let global_config_path = project_path.join(GLOBAL_DATA_CONFIG_FILE);
        let mut builder = Self::new();

        if global_config_path.is_file() {
            builder.add_global_data(&global_config_path)?;
        }

        Ok(builder)
    }

    /// Load global site data from a YAML file.
    /// The date is merged with the existing site data.
    ///
    /// # Arguments
    ///
    /// * `yaml_file`: The path to the YAML file to load
    ///
    /// returns: Result<&GlobalContextBuilder, Error>
    pub fn add_global_data(&mut self, yaml_file: &PathBuf) -> Result<&GlobalContext> {
        let yaml_str = std::fs::read_to_string(yaml_file)?;
        let yaml_data: Map<String, Value> = serde_yaml::from_str(&yaml_str)
            .context("Could not parse global data config as a YAML document.")?;
        self.global_data.extend(yaml_data);
        Ok(self)
    }

    /// Add a value to the global data.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the value
    /// * `value`: The value to add
    ///
    /// returns: ()
    pub fn insert(&mut self, key: &str, value: Value) {
        self.global_data.insert(key.to_string(), value);
    }

    /// Extend the global data with a map of values.
    ///
    /// # Arguments
    ///
    /// * `data`: The map of values to extend the global data with
    ///
    /// returns: ()
    pub fn extend(&mut self, data: Map<String, Value>) {
        self.global_data.extend(data);
    }

    /// Convert the global data to a Handlebars context.
    ///
    /// returns: Context
    pub fn handlebars_context(&self) -> handlebars::Context {
        let mut res: Map<String, Value> = Map::new();
        res.insert("site".to_string(), Value::Object(self.global_data.clone()));
        handlebars::Context::wraps(Value::Object(res)).unwrap()
    }
}
