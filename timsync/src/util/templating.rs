use crate::processing::task_processor::{TASKS_REF_MAP_KEY, TASKS_UID};
use crate::project::project::Project;
use crate::util::path;
use crate::util::path::{NormalizeExtension, RelativizeExtension};
use anyhow::{Context as AnyhowCtx, Result};
use handlebars::{
    Context, Handlebars, Helper, HelperResult, JsonTruthy, Output, RenderContext, RenderError,
    RenderErrorReason, Renderable, StringOutput, Template,
};
use nanoid::nanoid;
use serde_json::{json, Map, Value};
use std::io::Error as IOError;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};

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

    out.write(&format!("#- {{area_end=\"{}\"}}", area_name))?;

    Ok(())
}

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
fn docsettings_block<'reg, 'rc>(
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

/// Task helper.
/// Inserts a reference to a specific task plugin based on the task UID.
///
/// **Note**: The helper requires that there is at least one task (`*.task.yml` file) in the project.
///
/// Example:
///
/// `task-example.task.yml`:
///
/// ```yaml
/// ---
/// uid: task1
/// plugin: csPlugin
/// ---
/// type: text
/// header: Task Test
/// rows: 10
/// ```
///
/// `task-example.md`:
///
/// ```md
/// Task 1:
///
/// {{task "task1"}}
/// ```
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

    let site_ctx_json = _get_site_ctx_json(ctx)?;

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
/// Include without templating {{include "path/to/file.md" template=false}}
/// ```
fn include_helper<'reg, 'rc>(
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
        .unwrap_or(true);

    let local_project_dir = _get_local_project_dir(ctx)?;
    let target_file_path = _resolve_full_file_path(ctx, file_path, local_project_dir)?;

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

fn _resolve_full_file_path(
    ctx: &Context,
    file_path: &str,
    local_project_dir: &str,
) -> Result<PathBuf, RenderError> {
    let target_file_path = if file_path.starts_with("/") {
        // Absolute path, resolve from project root
        Path::new(local_project_dir).join(&file_path[1..])
    } else {
        // Relative path, resolve from current file
        let local_file_path = ctx
            .data()
            .get("local_file_path")
            .ok_or_else(|| RenderErrorReason::Other("Local file path is not set".to_string()))?
            .as_str()
            .ok_or_else(|| {
                RenderErrorReason::Other("Local file path is not a string".to_string())
            })?;

        Path::new(local_project_dir)
            .join(local_file_path)
            .parent()
            .ok_or_else(|| {
                RenderErrorReason::Other(
                    "Could not get parent directory of the local file path to resolve relative path".to_string(),
                )
            })?
            .join(&file_path)
            .to_path_buf()
    };
    // Normalize by removing redundant components and up-level references
    let target_file_path = target_file_path.normalize();
    Ok(target_file_path)
}

fn _get_local_project_dir(ctx: &Context) -> Result<&str, RenderError> {
    let site_ctx_json = _get_site_ctx_json(ctx)?;
    let local_project_dir = site_ctx_json
        .get("local_project_dir")
        .expect("Local project directory is not set")
        .as_str()
        .expect("Local project directory is not a string");
    Ok(local_project_dir)
}

pub const FILE_MAP_ATTRIBUTE: &str = "$_timsync_upload_files";

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
fn file_helper<'reg, 'rc>(
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

    let site_ctx_json = _get_site_ctx_json(ctx)?;
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

    let local_project_dir = _get_local_project_dir(ctx)?;
    let target_file_path = _resolve_full_file_path(ctx, file_path, local_project_dir)?;
    let tim_file_name = path::generate_hashed_filename(&target_file_path)
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

fn _get_site_ctx_json(ctx: &Context) -> Result<&Map<String, Value>, RenderErrorReason> {
    ctx.data()
        .get("site")
        .ok_or_else(|| RenderErrorReason::Other("Site context data is not set".to_string()))?
        .as_object()
        .ok_or_else(|| RenderErrorReason::Other("Site context data is not an object".to_string()))
}

pub trait TimRendererExt
where
    Self: Sized,
{
    /// Extend the renderer instance with the TIM templates for documents.
    ///
    /// returns: &Self
    fn with_tim_doc_templates(self) -> Self;

    /// Extend the renderer instance with the file helpers.
    ///
    /// returns: &Self
    fn with_file_helpers(self) -> Self;

    /// Extend the renderer instance with the project templates.
    /// The templates may be used as partials in the rendering process.
    ///
    /// Templates are scanned from the `_templates` folder in a project.
    /// All files in the folder are registered as templates.
    ///
    /// # Arguments
    ///
    /// * `project`: The project to get the templates from.
    ///
    /// returns: Result<Self, Error>
    fn with_project_templates(self, project: &Project) -> Result<Self>;

    /// Extend the renderer instance with the project helpers.
    /// The helpers are used to extend the templating engine with custom scripts.
    ///
    /// Helpers are scanned from the `_helpers` folder in a project.
    /// The helpers must be written in the Rhai scripting language (file extension `.rhai`).
    ///
    /// # Arguments
    ///
    /// * `project`: The project to get the helpers from.
    ///
    /// returns: Result<Self, Error>
    fn with_project_helpers(self, project: &Project) -> Result<Self>;
}

const TEMPLATE_FOLDER: &str = "_templates";
const HELPERS_FOLDER: &str = "_helpers";

impl TimRendererExt for Handlebars<'_> {
    fn with_tim_doc_templates(mut self) -> Self {
        self.register_escape_fn(handlebars::no_escape);
        self.register_helper("area", Box::new(area_block));
        self.register_helper("docsettings", Box::new(docsettings_block));
        self.register_helper("ref_area", Box::new(ref_area_helper));
        self.register_helper("task", Box::new(task_helper));
        handlebars_misc_helpers::register(&mut self);
        self.with_file_helpers()
    }

    fn with_file_helpers(mut self) -> Self {
        self.register_helper("include", Box::new(include_helper));
        self.register_helper("file", Box::new(file_helper));
        self
    }

    fn with_project_templates(mut self, project: &Project) -> Result<Self> {
        let template_files = project
            .find_files(TEMPLATE_FOLDER, "*")
            .with_context(|| format!("Could not find templates from folder {}", TEMPLATE_FOLDER))?;
        for (name, template) in template_files {
            self.register_template_file(&name, template)?;
        }

        Ok(self)
    }

    fn with_project_helpers(mut self, project: &Project) -> Result<Self> {
        let helper_files = project
            .find_files(HELPERS_FOLDER, "*.rhai")
            .with_context(|| format!("Could not find helpers from folder {}", HELPERS_FOLDER))?;
        for (name, helper) in helper_files {
            let name = name.trim_end_matches(".rhai");
            self.register_script_helper_file(&name, helper)?;
        }

        Ok(self)
    }
}

pub trait Merge {
    /// Merge a JSON value into the current value.
    ///
    /// # Arguments
    ///
    /// * `other`: The JSON value to merge
    ///
    /// returns: ()
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

pub trait ContextExtension {
    /// Extend the context with a JSON value.
    /// Updates the context data with the JSON value, possibly overwriting existing values.
    ///
    /// # Arguments
    ///
    /// * `other`: The JSON value to extend the context with
    ///
    /// returns: ()
    fn extend_with_json(&mut self, other: &Value);
}

impl ContextExtension for Context {
    fn extend_with_json(&mut self, other: &Value) {
        self.data_mut().merge(other);
    }
}

pub struct RenderResult<T> {
    pub rendered: T,
    pub modified_context: Option<Context>,
}

pub trait RendererExtension {
    fn render_template_with_context_to_output_return_new_context(
        &self,
        template_string: &str,
        ctx: &Context,
        output: &mut impl Output,
    ) -> Result<RenderResult<()>, RenderError>;

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

pub struct WriteOutput<W: Write> {
    write: W,
}

impl<W: Write> Output for WriteOutput<W> {
    fn write(&mut self, seg: &str) -> Result<(), IOError> {
        self.write.write_all(seg.as_bytes())
    }

    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> Result<(), IOError> {
        self.write.write_fmt(args)
    }
}

impl<W: Write> WriteOutput<W> {
    pub fn new(write: W) -> WriteOutput<W> {
        WriteOutput { write }
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
        let mut render_context = RenderContext::new(None);
        tpl.render(self, ctx, &mut render_context, output)?;

        Ok(RenderResult {
            rendered: (),
            modified_context: render_context.context().map(|c| c.deref().clone()),
        })
    }
}
