use crate::templating::util::WriteOutput;
use anyhow::Result;
use handlebars::template::Template;
use handlebars::{
    Context, Handlebars, Output, RenderContext, RenderError, Renderable, StringOutput,
};
use std::io::Write;
use std::ops::Deref;

/// Result of a Handlebars rendering operation.
pub struct RenderResult<T> {
    /// Rendered output.
    pub rendered: T,
    /// Modified context after rendering. If None, the context was not modified.
    pub modified_context: Option<Context>,
}

pub trait RendererExtension {
    /// Render a template string with a context to an output.
    ///
    /// # Arguments
    ///
    /// * `template_string`: The template string to render
    /// * `ctx`: The context to render the template with
    /// * `output`: The output to write the rendered template to
    ///
    /// returns: Result<RenderResult<()>, RenderError>
    fn render_template_with_context_to_output_return_new_context(
        &self,
        template_string: &str,
        ctx: &Context,
        output: &mut impl Output,
    ) -> Result<RenderResult<()>, RenderError>;

    /// Render a template string with a context and return the rendered output.
    ///
    /// # Arguments
    ///
    /// * `template_string`: The template string to render
    /// * `ctx`: The context to render the template with
    ///
    /// returns: Result<RenderResult<String>, RenderError>
    fn render_template_with_context_return_new_context(
        &self,
        template_string: &str,
        ctx: &Context,
    ) -> Result<RenderResult<String>, RenderError> {
        let mut out = StringOutput::new();
        let res = self.render_template_with_context_to_output_return_new_context(
            template_string,
            ctx,
            &mut out,
        )?;
        Ok(RenderResult {
            rendered: out.into_string().map_err(RenderError::from)?,
            modified_context: res.modified_context,
        })
    }

    /// Render a template string with a context to a writer
    /// and return the render result.
    ///
    /// # Arguments
    ///
    /// * `template_string`: The template string to render
    /// * `ctx`: The context to render the template with
    /// * `writer`: The writer to write the rendered template to
    ///
    /// returns: Result<RenderResult<()>, RenderError>
    fn render_template_with_context_to_write_return_new_context<W>(
        &self,
        template_string: &str,
        ctx: &Context,
        writer: W,
    ) -> Result<RenderResult<()>, RenderError>
    where
        W: Write,
    {
        let mut out = WriteOutput::new(writer);
        let res = self.render_template_with_context_to_output_return_new_context(
            template_string,
            ctx,
            &mut out,
        )?;
        Ok(RenderResult {
            rendered: (),
            modified_context: res.modified_context,
        })
    }
}

impl RendererExtension for Handlebars<'_> {
    fn render_template_with_context_to_output_return_new_context(
        &self,
        template_string: &str,
        ctx: &Context,
        output: &mut impl Output,
    ) -> Result<RenderResult<()>, RenderError> {
        let tpl = Template::compile(template_string).map_err(RenderError::from)?;
        let mut render_context = RenderContext::new(tpl.name.as_ref());
        tpl.render(self, ctx, &mut render_context, output)?;

        Ok(RenderResult {
            rendered: (),
            modified_context: render_context.context().map(|c| c.deref().clone()),
        })
    }
}
