use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Config(String),
    Parse {
        path: PathBuf,
        message: String,
    },
    Codebase(entl_codebase::Error),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Config(message) => write!(formatter, "invalid configuration: {message}"),
            Self::Parse { path, message } => {
                write!(formatter, "could not parse {}: {message}", path.display())
            }
            Self::Codebase(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Codebase(error) => Some(error),
            Self::Config(_) | Self::Parse { .. } => None,
        }
    }
}

impl From<entl_codebase::Error> for Error {
    fn from(error: entl_codebase::Error) -> Self {
        Self::Codebase(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
