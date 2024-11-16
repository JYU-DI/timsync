use crate::util::path::NormalizeExtension;
use handlebars::{Context, Output, RenderError, RenderErrorReason};
use serde_json::{Map, Value};
use std::io::{Error as IOError, Write};
use std::path::{Path, PathBuf};

pub fn resolve_full_file_path(
    ctx: &Context,
    file_path: &str,
    local_project_dir: &str,
) -> anyhow::Result<PathBuf, RenderError> {
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

pub fn get_local_project_dir(ctx: &Context) -> anyhow::Result<&str, RenderError> {
    let site_ctx_json = get_site_ctx_json(ctx)?;
    let local_project_dir = site_ctx_json
        .get("local_project_dir")
        .expect("Local project directory is not set")
        .as_str()
        .expect("Local project directory is not a string");
    Ok(local_project_dir)
}

pub fn get_site_ctx_json(ctx: &Context) -> anyhow::Result<&Map<String, Value>, RenderErrorReason> {
    ctx.data()
        .get("site")
        .ok_or_else(|| RenderErrorReason::Other("Site context data is not set".to_string()))?
        .as_object()
        .ok_or_else(|| RenderErrorReason::Other("Site context data is not an object".to_string()))
}

// Copied from handlebars::output::WriteOutput as it is not public
pub struct WriteOutput<W: Write> {
    write: W,
}

impl<W: Write> Output for WriteOutput<W> {
    fn write(&mut self, seg: &str) -> anyhow::Result<(), IOError> {
        self.write.write_all(seg.as_bytes())
    }

    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> anyhow::Result<(), IOError> {
        self.write.write_fmt(args)
    }
}

impl<W: Write> WriteOutput<W> {
    pub fn new(write: W) -> WriteOutput<W> {
        WriteOutput { write }
    }
}
