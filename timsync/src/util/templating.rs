use std::collections::HashMap;

use lazy_static::lazy_static;
use serde_json::Value;

use tera::Context;
use tera::Error;
use tera::Result;
use tera::Tera;

fn area_filter(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    match value {
        Value::String(s) => {
            let area = args.get("name").ok_or("area name is missing")?;
            let area_name = area.as_str().ok_or("area name is not a string")?;
            let is_collapsible = args
                .get("collapse")
                .map(|v| v.as_bool().unwrap_or(false))
                .unwrap_or(false);
            let collapse = if is_collapsible {
                "collapse=\"true\""
            } else {
                ""
            };
            Ok(Value::String(format!(
                "#- {{area=\"{}\" {}}}\n{}\n#- {{area_end=\"{}\"}}",
                area_name, collapse, s, area_name
            )))
        }
        _ => Err(Error::msg("area filter only works on strings")),
    }
}

lazy_static! {
    static ref TIM_TEMPLATES: Tera = {
        let mut tera = Tera::default();
        tera.register_filter("area", area_filter);
        tera
    };
    static ref EMPTY_CONTEXT: Context = Context::new();
}

pub trait TeraExt {
    /// Extend the Tera instance with the TIM templates.
    ///
    /// returns: &Self
    fn with_tim_templates(self) -> Self;
}

impl TeraExt for Tera {
    fn with_tim_templates(mut self) -> Self {
        self.extend(&TIM_TEMPLATES).unwrap();
        self
    }
}
