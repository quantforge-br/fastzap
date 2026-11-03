//! IPC surface invoked by the local frontend.

pub mod accounts;
pub mod settings;

use serde::Serialize;

/// Error returned to the frontend; never contains secrets.
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub message: String,
}

impl<E: std::fmt::Display> From<E> for CommandError {
    fn from(err: E) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
