use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext, Renderable};

/// Docsettings block helper.
/// Surrounds the content into a docsettings block.
///
/// Example:
///
/// ```md
///
/// {{#docsettings}}
/// foo: bar
/// baz: qux
/// {{/docsettings}}
/// ```
pub fn docsettings_block<'reg, 'rc>(
    h: &Helper<'rc>,
    r: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    rc: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    out.write("``` {settings=\"\"}\n\n")?;
    if let Some(tmpl) = h.template() {
        tmpl.render(r, ctx, rc, out)?;
    }
    out.write("\n```\n\n")?;

    Ok(())
}
