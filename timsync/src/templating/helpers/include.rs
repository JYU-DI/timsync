use crate::templating::ext_context::ContextExtension;
use crate::templating::util::{get_local_project_dir, resolve_full_file_path};
use crate::util::path::RelativizeExtension;
use handlebars::{
    Context, Handlebars, Helper, HelperResult, JsonTruthy, Output, RenderContext, RenderErrorReason,
};
use serde_json::json;
use std::path::Path;

/// Include helper.
/// Includes the content of a file in the current document with optional templating.
/// The file path can be either relative or absolute to the project root (by using `/` as a prefix).
///
/// **Note**: To use relative paths, the local file path variable must be set in the context.
///
/// Example:
///
/// ```md
/// Relative include to the current file {{include "path/to/file.md"}}
///
/// Absolute include {{include "/path/to/file.md"}}
///
/// Include with templating {{include "path/to/file.md" template=true}}
/// ```
pub fn include_helper<'reg, 'rc>(
    h: &Helper<'rc>,
    r: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    _: &mut RenderContext<'reg, 'rc>,
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

    let do_template = h
        .hash_get("template")
        .map(|v| v.value().is_truthy(true))
        .unwrap_or(false);

    let local_project_dir = get_local_project_dir(ctx)?;
    let target_file_path = resolve_full_file_path(ctx, file_path, local_project_dir)?;

    if !target_file_path.is_file() {
        return Err(RenderErrorReason::Other(format!(
            "File '{}' does not exist",
            target_file_path.display()
        ))
        .into());
    }

    let file_contents = std::fs::read_to_string(&target_file_path).map_err(|e| {
        RenderErrorReason::Other(format!(
            "Could not read file '{}': {}",
            target_file_path.display(),
            e
        ))
    })?;

    let file_contents = if do_template {
        let new_local_file_path = target_file_path
            .relativize(Path::new(local_project_dir))
            .to_string_lossy()
            .to_string();
        // Create a new context with the local file path set to the included file
        // This allows the included file to use include helper itself
        let mut ctx = ctx.clone();
        ctx.extend_with_json(&json!({
            "local_file_path": new_local_file_path
        }));

        r.render_template_with_context(&file_contents, &ctx)
            .map_err(|e| {
                RenderErrorReason::Other(format!(
                    "Could not render included file '{}': {}",
                    target_file_path.display(),
                    e
                ))
            })?
    } else {
        file_contents
    };

    out.write(&file_contents)?;

    Ok(())
}
