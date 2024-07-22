use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use path_absolutize::Absolutize;

pub trait Relativize {
    fn relativize(&self, path: &Path) -> PathBuf;
}

impl Relativize for Path {
    /// Resolve the relative path portion of this path in relation to the given path.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to relativize against.
    ///
    /// returns: PathBuf
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
    fn with_set_extension<S: AsRef<OsStr>>(self, ext: S) -> PathBuf;
}

impl WithSetExtension for PathBuf {
    /// Set an extension of the path and return the path itself.
    ///
    /// # Arguments
    ///
    /// * `ext`: The extension to set to the path.
    ///
    /// returns: PathBuf
    fn with_set_extension<S: AsRef<OsStr>>(mut self, ext: S) -> PathBuf {
        self.set_extension(ext);
        self
    }
}

pub trait FullExtension {
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
    /// Get the full extension of the path. That is, all parts after the first dot.
    ///
    /// returns: String
    fn full_extension(&self) -> Option<&OsStr> {
        self.file_name().map(split_file_at_dot).and_then(|(_, after)| after)
    }
}
