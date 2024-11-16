use crate::templating::tim_handlebars::FILE_MAP_ATTRIBUTE;
use crate::templating::util::{get_local_project_dir, get_site_ctx_json, resolve_full_file_path};
use crate::util::path::generate_hashed_filename;
use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderErrorReason,
};
use serde_json::map::Map;
use serde_json::value::Value;

/// File helper.
/// The helper is used to convert a file path to the final URL of the file and to
/// explicitly mark the file to be uploaded into the current document.
///
/// In Markdown files, this CLI tool will try to automatically find references to files
/// and upload them to the TIM server. In cases where automatic detection fails, the file helper
/// can be used to explicitly mark the file for upload.
///
/// Example:
///
/// ```md
/// Relative import: ![]({{file "path/to/file.ext"}})
///
/// Absolute import: ![]({{file "/path/to/file.ext"}})
/// ```
pub fn file_helper<'reg, 'rc>(
    h: &Helper<'rc>,
    _: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    rc: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    let file_path = h
        .param(0)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("path", 0))?
        .value()
        .as_str()
        .ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                "path",
                "0".to_string(),
                "string".to_string(),
            )
        })?;

    let site_ctx_json = get_site_ctx_json(ctx)?;
    let base_path = site_ctx_json
        .get("base_path")
        .expect("Base path is not set")
        .as_str()
        .expect("Base path is not a string");
    let tim_doc_path =
        ctx.data().get("path").ok_or_else(|| {
            RenderErrorReason::Other(
                "To use the 'file' helper, the template must have 'path' attribute available in context".to_string(),
            )
        })?.as_str().ok_or_else(|| {
            RenderErrorReason::Other(
                "To use the 'file' helper, the 'path' attribute in context must be a string".to_string(),
            )
        })?;

    let local_project_dir = get_local_project_dir(ctx)?;
    let target_file_path = resolve_full_file_path(ctx, file_path, local_project_dir)?;
    let tim_file_name = generate_hashed_filename(&target_file_path)
        .map_err(|e| RenderErrorReason::Other(e.to_string()))?;

    let mut ctx = rc.context().as_deref().unwrap_or(ctx).clone();
    if let Some(ref mut m) = ctx.data_mut().as_object_mut() {
        let files_map = m
            .entry(FILE_MAP_ATTRIBUTE)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .ok_or_else(|| RenderErrorReason::Other("Files map is not an object".to_string()))?;
        files_map.insert(
            target_file_path.to_string_lossy().to_string(),
            Value::String(tim_file_name.clone()),
        );
    }
    rc.set_context(ctx);

    out.write(&format!(
        "/files/{}/{}/{}",
        base_path, tim_doc_path, tim_file_name
    ))?;

    Ok(())
}
