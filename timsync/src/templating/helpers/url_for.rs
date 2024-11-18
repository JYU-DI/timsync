use crate::templating::util::get_site_ctx_json;
use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderErrorReason,
};

/// URL generation helper,
/// Generates a full URL to the given document uid.
///
///
/// Example:
///
/// `doc1.md`:
/// ````
/// ---
/// uid: doc1
/// ---
///
/// Document 1
/// ````
///
/// `doc2.md`:
/// ````
/// [Link to Document 1]({{url_for "doc1"}})
/// ````
pub fn url_for_helper<'reg, 'rc>(
    h: &Helper<'rc>,
    _: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    _: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    let doc_uid = h
        .param(0)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("doc_id", 0))?
        .value()
        .as_str()
        .ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                "doc_id",
                "0".to_string(),
                "string".to_string(),
            )
        })?;

    let view_url = h
        .hash_get("view")
        .map(|v| v.value().as_str().unwrap_or(""))
        .unwrap_or("view");

    let site_ctx_json = get_site_ctx_json(ctx)?;

    let base_path = site_ctx_json
        .get("base_path")
        .expect("Base path is not set")
        .as_str()
        .expect("Base path is not a string");

    let doc_map = site_ctx_json
        .get("doc")
        .expect("Document map is not set")
        .as_object()
        .expect("Document map is not an object");

    let doc_path = doc_map
        .get(doc_uid)
        .map(|v| v.as_object().expect("Document info is not an object"))
        .map(|v| {
            v.get("path")
                .expect("Document TIM path is not set")
                .as_str()
                .expect("Document TIM path is not a string")
        })
        .ok_or_else(|| {
            RenderErrorReason::Other(format!(
                "Document with uid '{}' not found in the project",
                doc_uid
            ))
        })?;

    if view_url.is_empty() {
        out.write(&format!("{}/{}", base_path, doc_path))?;
    } else {
        out.write(&format!("/{}/{}/{}", view_url, base_path, doc_path))?;
    }

    Ok(())
}
