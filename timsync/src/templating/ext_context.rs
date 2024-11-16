use serde_json::Value;
use handlebars::Context;
use crate::util::json::Merge;

pub trait ContextExtension {
    /// Extend the context with a JSON value.
    /// Updates the context data with the JSON value, possibly overwriting existing values.
    ///
    /// # Arguments
    ///
    /// * `other`: The JSON value to extend the context with
    ///
    /// returns: ()
    fn extend_with_json(&mut self, other: &Value);
}

impl ContextExtension for Context {
    fn extend_with_json(&mut self, other: &Value) {
        self.data_mut().merge(other);
    }
}