use crate::util::tim_client::hashed_par_id;
use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext};

/// Par ID generate helper.
/// Generates a paragraph ID.
/// If called without parameters, generates a random paragraph ID.
/// If given a string parameter, generates a paragraph ID hashed from the string.
///
/// Example:
///
/// ```md
/// Random paragraph ID: {{gen_par_id}}
///
/// Hashed paragraph ID: {{gen_par_id "my-unique-id"}}
///
/// ```
pub fn gen_par_id_helper<'reg, 'rc>(
    h: &Helper<'rc>,
    _: &'reg Handlebars<'reg>,
    _: &'rc Context,
    _: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    let hash = h
        .param(0)
        .map(|v| v.value().as_str())
        .unwrap_or(None)
        .map(|s| s.to_string());

    let par_id = hashed_par_id(hash.as_deref());

    out.write(&par_id)?;

    Ok(())
}
