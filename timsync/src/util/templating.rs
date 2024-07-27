use handlebars::{
    Context, Handlebars, Helper, HelperResult, JsonTruthy, Output, Renderable,
    RenderContext, RenderErrorReason,
};
use nanoid::nanoid;
use serde_json::Value;

use crate::processing::task_processor::{TASKS_REF_MAP_KEY, TASKS_UID};

/// Area block helper.
///
/// Example:
/// ```
/// {{#area}}
/// Areas can also be unnamed. In that case, the area name is generated using a random UUID.
/// {{/area}}
///
/// {{#area "content-example"}}
/// This is the content area.
/// {{/area}}
///
/// {{#area "collapse-example" collapse=true}}
/// Collapse button contents
/// {{else}}
/// Collapsed contents
/// {{/area}}
/// ```
fn area_block<'reg, 'rc>(
    h: &Helper<'rc>,
    r: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    rc: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    let area_name = match h.param(0) {
        Some(v) => match v.value() {
            Value::String(s) => s.clone(),
            _ => {
                return Err(RenderErrorReason::ParamTypeMismatchForName(
                    "name",
                    "0".to_string(),
                    "string".to_string(),
                )
                .into())
            }
        },
        None => format!("area-{}", nanoid!(8)),
    };

    let collapse = h
        .hash_get("collapse")
        .map(|v| v.value().is_truthy(true))
        .unwrap_or(false);

    out.write(&format!(
        "#- {{area=\"{}\" {}}}\n",
        area_name,
        if collapse { "collapse=\"true\"" } else { "" }
    ))?;

    if let Some(tmpl) = h.template() {
        tmpl.render(r, ctx, rc, out)?;
    }

    if let Some(tmpl) = h.inverse() {
        out.write("#-\n")?;
        tmpl.render(r, ctx, rc, out)?;
    }

    out.write(&format!("#- {{area_end=\"{}\"}}", area_name))?;

    Ok(())
}

/// Reference area helper.
///
/// Example:
///
/// ```
/// {{#area "area-example"}}
/// This is the content area.
/// {{/area}}
///
/// This is a reference to the area within the same document:
///
/// {{ref_area doc_id "area-example"}}
///
/// Note that the ref_area requires the document ID and the area name.
/// ```
fn ref_area_helper<'reg, 'rc>(
    h: &Helper<'rc>,
    _: &'reg Handlebars<'reg>,
    _: &'rc Context,
    _: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    let doc_id_param = h
        .param(0)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("doc_id", 0))?;

    let doc_id = match doc_id_param.value() {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
    .ok_or_else(|| {
        RenderErrorReason::ParamTypeMismatchForName(
            "doc_id",
            "0".to_string(),
            "non-negative integer".to_string(),
        )
    })?;

    let area_name = h
        .param(1)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("area_name", 1))?
        .value()
        .as_str()
        .ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                "area_name",
                "1".to_string(),
                "string".to_string(),
            )
        })?;

    out.write(&format!("#- {{rd=\"{}\" ra=\"{}\"}}", doc_id, area_name))?;

    Ok(())
}

fn task_helper<'reg, 'rc>(
    h: &Helper<'rc>,
    _: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    _: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    let task_id = h
        .param(0)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("task_id", 0))?
        .value()
        .as_str()
        .ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                "task_id",
                "0".to_string(),
                "string".to_string(),
            )
        })?;

    let site_ctx_json = ctx
        .data()
        .as_object()
        .map(|v| {
            v.get("site")
                .expect("Site context is not set")
                .as_object()
                .expect("Site context is not an object")
        })
        .ok_or_else(|| RenderErrorReason::Other("Site context data is not set".to_string()))?;

    let task_ref_map = site_ctx_json.get(TASKS_REF_MAP_KEY).ok_or_else(|| {
        RenderErrorReason::Other("There are no tasks registered in the project. Add tasks (`.task.yml` files) to the project to use the task helper.".to_string())
    })?.as_object().expect("Task reference map is not an object");
    let doc_map = site_ctx_json
        .get("doc")
        .expect("Document map is not set")
        .as_object()
        .expect("Document map is not an object");

    let task_doc_id = doc_map
        .get(TASKS_UID)
        .map(|v| v.as_object().expect("Task document is not an object"))
        .map(|v| {
            v.get("doc_id")
                .expect("Task document ID is not set")
                .as_u64()
                .expect("Task document ID is not a number")
        })
        .expect("Task document is missing from the document list.");

    let task_par_id = task_ref_map.get(task_id).map(|v| v.as_str().expect("Par ID is not a string")).ok_or_else(|| {
        RenderErrorReason::Other(format!("Task with UID '{}' is not registered in the project. Check that the UID is written correctly.", task_id))
    })?;

    out.write(&format!(
        "#- {{ rd=\"{}\" rp=\"{}\" }}\n",
        task_doc_id, task_par_id
    ))?;

    Ok(())
}

pub trait TimRendererExt {
    /// Extend the renderer instance with the TIM templates for documents.
    ///
    /// returns: &Self
    fn with_tim_doc_templates(self) -> Self;
}

impl TimRendererExt for Handlebars<'_> {
    fn with_tim_doc_templates(mut self) -> Self {
        self.register_escape_fn(handlebars::no_escape);
        self.register_helper("area", Box::new(area_block));
        self.register_helper("ref_area", Box::new(ref_area_helper));
        self.register_helper("task", Box::new(task_helper));
        handlebars_misc_helpers::register(&mut self);
        self
    }
}

pub trait Merge {
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

pub trait ExtendableContext {
    fn extend_with_json(&mut self, other: &Value);
}

impl ExtendableContext for handlebars::Context {
    fn extend_with_json(&mut self, other: &Value) {
        self.data_mut().merge(other);
    }
}
