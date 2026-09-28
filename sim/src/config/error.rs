use std::{
    fmt,
    path::{Path, PathBuf},
};

/// Error raised while loading, parsing or validating a config
#[derive(Debug)]
pub enum ConfigError {
    /// The config file could not be read
    Io {
        /// Path of the offending file
        path: PathBuf,
        /// Underlying IO error
        source: std::io::Error,
    },
    /// The config file contains invalid RON
    Parse {
        /// Path of the offending file
        path: PathBuf,
        /// Located RON parse error
        source: ron::error::SpannedError,
    },
    /// The parsed config violates an invariant
    Invariant {
        /// Path of the offending file
        path: PathBuf,
        /// Human-readable detail of the broken invariant
        message: String,
    },
}

impl ConfigError {
    /// Path of the file the error relates to
    pub fn path(&self) -> &Path {
        match self {
            ConfigError::Io { path, .. }
            | ConfigError::Parse { path, .. }
            | ConfigError::Invariant { path, .. } => path,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "can't read {} : {}", path.display(), source)
            }
            ConfigError::Parse { path, source } => {
                write!(f, "RON invalid in {} : {}", path.display(), source)
            }
            ConfigError::Invariant { path, message } => {
                write!(f, "invalid config in {} : {}", path.display(), message)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            ConfigError::Parse { source, .. } => Some(source),
            ConfigError::Invariant { .. } => None,
        }
    }
}
