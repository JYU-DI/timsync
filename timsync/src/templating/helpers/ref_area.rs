use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderErrorReason,
};
use serde_json::Value;

/// Reference area helper.
/// Inserts a reference to a named area in the same or another document.
///
/// Example:
///
/// ```md
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
pub fn ref_area_helper<'reg, 'rc>(
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
