use std::path::PathBuf;

use anyhow::{Context, Result};
use lazy_init::Lazy;

pub fn get_or_read_file_contents<'a>(
    path: &'a PathBuf,
    lazy: &'a Lazy<Result<String>>,
) -> Result<&'a str> {
    let res = lazy.get_or_create(|| {
        std::fs::read_to_string(&path)
            .with_context(|| format!("Could not read file: {}", path.display()))
    });

    match res {
        Ok(contents) => Ok(contents.as_str()),
        Err(err) => Err(anyhow::anyhow!(format!("{}", err))),
    }
}

pub fn get_or_set_front_matter_position<'a>(
    contents: &'a Lazy<Result<String>>,
    lazy: &'a Lazy<Option<(usize, usize)>>,
    start_delimiter: &str,
    end_delimiter: &str,
) -> Option<(usize, usize)> {
    let res = lazy.get_or_create(|| {
        let Some(Ok(contents)) = contents.get() else {
            return None;
        };
        find_front_matter_simple(contents, start_delimiter, end_delimiter)
    });

    res.clone()
}

/// Find the front matter in a file.
///
/// This is a basic naive implementation that looks for any string of format
///
/// ```
/// <start_delimiter>
/// .*
/// <end_delimiter>
/// ```
///
/// # Arguments
///
/// * `contents` - The contents of the file to search in.
/// * `start_delimiter` - The start delimiter of the front matter.
/// * `end_delimiter` - The end delimiter of the front matter.
///
/// Returns: Option<(usize, usize)>
pub fn find_front_matter_simple(
    contents: &str,
    start_delimiter: &str,
    end_delimiter: &str,
) -> Option<(usize, usize)> {
    let mut start = 0;
    let mut end = 0;
    let mut found_start = false;

    for l in contents.split_inclusive('\n') {
        if !found_start {
            let trimmed = l.trim_end();
            if trimmed.is_empty() {
                start += l.len();
                continue;
            } else if trimmed.starts_with(start_delimiter) {
                found_start = true;
                end = start + l.len();
                continue;
            } else {
                return None;
            }
        } else {
            let trimmed = l.trim_end();
            if trimmed.starts_with(end_delimiter) {
                // Here, we add just the length of the end delimiter to the end position
                // to ensure that the last newline is not included as it is not part of the front matter
                return Some((start, end + trimmed.len()));
            } else {
                end += l.len();
            }
        }
    }

    None
}
