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
