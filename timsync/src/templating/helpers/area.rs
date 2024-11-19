use handlebars::{
    Context, Handlebars, Helper, HelperResult, JsonTruthy, Output, RenderContext,
    RenderErrorReason, Renderable,
};
use nanoid::nanoid;
use serde_json::value::Value;

/// Area block helper.
/// Surrounds the content into an area. Areas can be collapsed.
/// All areas must be named. If no name is specified, the helper generates a random UUID for the name.
///
/// Example:
/// ```md
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
pub fn area_block<'reg, 'rc>(
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

    let class = h
        .hash_get("class")
        .and_then(|v| v.value().as_str())
        .unwrap_or("");

    out.write(&format!(
        "#- {{area=\"{}\" {} {}}}\n",
        area_name,
        if collapse { "collapse=\"true\"" } else { "" },
        class
    ))?;

    if !collapse {
        out.write("\n#-\n")?;
    }

    if let Some(tmpl) = h.template() {
        tmpl.render(r, ctx, rc, out)?;
    }

    if let Some(tmpl) = h.inverse() {
        out.write("#-\n")?;
        tmpl.render(r, ctx, rc, out)?;
    }

    out.write(&format!("#- {{area_end=\"{}\"}}\n\n#-\n", area_name))?;

    Ok(())
}
