use std::fmt;

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum Error {
    #[error("{0}")]
    InvalidInput(String),

    #[error("{0}")]
    ConfigFormat(String),

    #[error("{0}")]
    Detection(String),

    #[error("{0}")]
    Lsp(String),

    #[error("{0}")]
    Network(String),

    #[error("{0}")]
    MissingExecutable(String),

    #[error("{0}")]
    Unexpected(String),
}

impl Error {
    fn message(&self) -> &str {
        match self {
            Self::InvalidInput(message)
            | Self::ConfigFormat(message)
            | Self::Detection(message)
            | Self::Lsp(message)
            | Self::Network(message)
            | Self::MissingExecutable(message)
            | Self::Unexpected(message) => message,
        }
    }

    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    pub(crate) fn config_format(message: impl Into<String>) -> Self {
        Self::ConfigFormat(message.into())
    }

    pub(crate) fn detection(message: impl Into<String>) -> Self {
        Self::Detection(message.into())
    }

    pub(crate) fn lsp(message: impl Into<String>) -> Self {
        Self::Lsp(message.into())
    }

    pub(crate) fn network(message: impl Into<String>) -> Self {
        Self::Network(message.into())
    }

    pub(crate) fn missing_executable(message: impl Into<String>) -> Self {
        Self::MissingExecutable(message.into())
    }

    pub(crate) fn unexpected(message: impl Into<String>) -> Self {
        Self::Unexpected(message.into())
    }

    pub(crate) fn with_prefix(self, prefix: impl fmt::Display) -> Self {
        match self {
            Self::InvalidInput(message) => Self::InvalidInput(format!("{prefix}: {message}")),
            Self::ConfigFormat(message) => Self::ConfigFormat(format!("{prefix}: {message}")),
            Self::Detection(message) => Self::Detection(format!("{prefix}: {message}")),
            Self::Lsp(message) => Self::Lsp(format!("{prefix}: {message}")),
            Self::Network(message) => Self::Network(format!("{prefix}: {message}")),
            Self::MissingExecutable(message) => {
                Self::MissingExecutable(format!("{prefix}: {message}"))
            }
            Self::Unexpected(message) => Self::Unexpected(format!("{prefix}: {message}")),
        }
    }

    #[must_use]
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidInput(_) => 2,
            Self::ConfigFormat(_)
            | Self::Detection(_)
            | Self::Lsp(_)
            | Self::Network(_)
            | Self::MissingExecutable(_)
            | Self::Unexpected(_) => 1,
        }
    }

    #[must_use]
    pub(crate) fn should_log_as_unexpected(&self) -> bool {
        matches!(self, Self::Unexpected(_))
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn contains(&self, needle: &str) -> bool {
        self.message().contains(needle)
    }

}

impl PartialEq<String> for Error {
    fn eq(&self, other: &String) -> bool {
        self.message() == other
    }
}

impl PartialEq<&str> for Error {
    fn eq(&self, other: &&str) -> bool {
        self.message() == *other
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn invalid_input_uses_exit_code_two() {
        assert_eq!(Error::invalid_input("bad").exit_code(), 2);
    }

    #[test]
    fn only_unexpected_is_logged_as_unexpected() {
        assert!(!Error::detection("detected").should_log_as_unexpected());
        assert!(Error::unexpected("boom").should_log_as_unexpected());
    }

}
