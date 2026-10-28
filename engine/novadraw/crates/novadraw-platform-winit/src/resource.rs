use std::io;
use std::path::Path;

/// Native file-system resource provider.
///
/// The provider owns only file I/O. Callers decode the returned bytes with Core
/// resource types and submit the result through a Runtime request token.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileResourceProvider;

impl FileResourceProvider {
    pub const fn new() -> Self {
        Self
    }

    pub fn load(&self, path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
}
