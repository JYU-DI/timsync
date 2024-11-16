use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use anyhow::Context;
use path_absolutize::Absolutize;
use sha1::Digest;
use std::fs::File;
use std::io;

pub trait RelativizeExtension {
    /// Resolve the relative path portion of this path in relation to the given path.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to relativize against.
    ///
    /// returns: PathBuf
    fn relativize(&self, path: &Path) -> PathBuf;
}

impl RelativizeExtension for Path {
    fn relativize(&self, path: &Path) -> PathBuf {
        let self_absolute: Cow<Path> = if self.is_absolute() {
            self.into()
        } else {
            self.absolutize().unwrap()
        };
        let path_absolute: Cow<Path> = if path.is_absolute() {
            path.into()
        } else {
            path.absolutize().unwrap()
        };
        self_absolute
            .strip_prefix(path_absolute)
            .unwrap()
            .to_path_buf()
    }
}

pub trait WithSetExtension {
    /// Set an extension of the path and return the path itself.
    ///
    /// # Arguments
    ///
    /// * `ext`: The extension to set to the path.
    ///
    /// returns: PathBuf
    fn with_set_extension<S: AsRef<OsStr>>(self, ext: S) -> PathBuf;
}

impl WithSetExtension for PathBuf {
    fn with_set_extension<S: AsRef<OsStr>>(mut self, ext: S) -> PathBuf {
        self.set_extension(ext);
        self
    }
}

pub trait FullExtension {
    /// Get the full extension of the path. That is, all parts after the first dot.
    ///
    /// returns: String
    fn full_extension(&self) -> Option<&OsStr>;
}

// Yanked from Rust core as it is not public API.
fn split_file_at_dot(file: &OsStr) -> (&OsStr, Option<&OsStr>) {
    let slice = file.as_encoded_bytes();
    if slice == b".." {
        return (file, None);
    }

    // The unsafety here stems from converting between &OsStr and &[u8]
    // and back. This is safe to do because (1) we only look at ASCII
    // contents of the encoding and (2) new &OsStr values are produced
    // only from ASCII-bounded slices of existing &OsStr values.
    let i = match slice[1..].iter().position(|b| *b == b'.') {
        Some(i) => i + 1,
        None => return (file, None),
    };
    let before = &slice[..i];
    let after = &slice[i + 1..];
    unsafe {
        (
            OsStr::from_encoded_bytes_unchecked(before),
            Some(OsStr::from_encoded_bytes_unchecked(after)),
        )
    }
}

impl FullExtension for PathBuf {
    fn full_extension(&self) -> Option<&OsStr> {
        self.file_name()
            .map(split_file_at_dot)
            .and_then(|(_, after)| after)
    }
}

pub trait NormalizeExtension {
    /// Normalize the path by removing any `.` and `..` components.
    ///
    /// returns: PathBuf
    fn normalize(&self) -> PathBuf;
}

impl NormalizeExtension for PathBuf {
    fn normalize(&self) -> PathBuf {
        let mut components = self.components().peekable();
        let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek() {
            let buf = PathBuf::from(c.as_os_str());
            components.next();
            buf
        } else {
            PathBuf::new()
        };

        for component in components {
            match component {
                Component::Prefix(..) => unreachable!(),
                Component::RootDir => {
                    ret.push(component.as_os_str());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    ret.pop();
                }
                Component::Normal(c) => {
                    ret.push(c);
                }
            }
        }

        ret
    }
}

/// Generate a hashed filename based on the file extension and the contents of the file.
/// The hash is calculated using the SHA1 algorithm.
///
/// # Arguments
///
/// * `target_file_path`: The path to the file to generate the hashed filename for.
///
/// returns: Result<String>
pub fn generate_hashed_filename(target_file_path: &PathBuf) -> anyhow::Result<String> {
    if !target_file_path.is_file() {
        return Err(anyhow::anyhow!(
            "File does not exist: {}",
            target_file_path.display()
        ));
    }

    let file_ext = target_file_path
        .full_extension()
        .and_then(|ext| ext.to_str().map(|s| format!(".{}", s)))
        .unwrap_or("".to_string());

    let file_sha1 = {
        let mut file = File::open(&target_file_path)
            .with_context(|| format!("Could not open file '{}'", target_file_path.display()))?;
        let mut hasher = sha1::Sha1::new();
        io::copy(&mut file, &mut hasher)?;
        format!("{:x}", hasher.finalize())
    };

    Ok(format!("{}{}", file_sha1, file_ext))
}
