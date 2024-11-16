use serde_json::Value;

pub trait Merge {
    /// Merge a JSON value into the current value.
    ///
    /// # Arguments
    ///
    /// * `other`: The JSON value to merge
    ///
    /// returns: ()
    fn merge(&mut self, other: &Value);
}

impl Merge for Value {
    fn merge(&mut self, other: &Value) {
        match (self, other) {
            (Value::Object(self_map), Value::Object(other_map)) => {
                for (key, other_value) in other_map {
                    self_map
                        .entry(key)
                        .and_modify(|self_value| self_value.merge(other_value))
                        .or_insert(other_value.clone());
                }
            }
            (self_value, other_value) => {
                *self_value = other_value.clone();
            }
        }
    }
}
