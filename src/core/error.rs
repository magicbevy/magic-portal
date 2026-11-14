use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("network error: {0}")]
    Network(String),

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid manifest at {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("usage: {0}")]
    Usage(String),

    #[error("checksum mismatch for {path}")]
    ChecksumMismatch { path: PathBuf },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("unsupported platform: {0}")]
    Unsupported(String),

    #[error("install failed: {0}")]
    Install(String),

    #[error("runtime error: {0}")]
    Runtime(String),

    #[error("cancelled")]
    Cancelled,
}

impl PortalError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Network(_) => "network",
            Self::Io { .. } => "io",
            Self::InvalidManifest { .. } => "manifest",
            Self::InvalidInput(_) => "invalid_input",
            Self::Usage(_) => "usage",
            Self::ChecksumMismatch { .. } => "checksum",
            Self::NotFound(_) => "not_found",
            Self::Unsupported(_) => "unsupported",
            Self::Install(_) => "install",
            Self::Runtime(_) => "runtime",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Cancelled => 3,
            _ => 1,
        }
    }
}

impl From<ureq::Error> for PortalError {
    fn from(error: ureq::Error) -> Self {
        match error {
            ureq::Error::Status(code, _) => {
                Self::NotFound(format!("server returned HTTP status {code}"))
            }
            ureq::Error::Transport(error) => Self::Network(error.to_string()),
        }
    }
}

pub type PortalResult<T> = Result<T, PortalError>;
