//! IPC surface. `bridge` is callable from WhatsApp Web; the rest from the local UI.

pub mod accounts;
pub mod app;
pub mod bridge;
pub mod customizations;
pub mod downloads;
pub mod settings;
pub mod webview;

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
