#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    
    #[error("Corrupt archive: entry '{entry}' - {reason}")]
    CorruptArchive { entry: String, reason: String },
    
    #[error("Password required")]
    PasswordRequired,
    
    #[error("Wrong password")]
    WrongPassword,
    
    #[error("Entry not found: {0}")]
    EntryNotFound(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(std::path::PathBuf),
    
    #[error("Path traversal attempt: {0}")]
    PathTraversal(String),
    
    #[error("{0}")]
    Custom(String),
}

impl Error {
    pub fn custom(s: impl Into<String>) -> Self {
        Error::Custom(s.into())
    }
}