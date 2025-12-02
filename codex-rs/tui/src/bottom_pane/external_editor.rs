use std::env;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;

use tempfile::Builder;
use tempfile::TempPath;
use tracing::warn;

#[derive(Debug)]
pub(crate) enum ExternalEditorError {
    NoEditorConfigured,
    InvalidCommand(String),
    TerminalRestore(io::Error),
    TempFileCreate(io::Error),
    TempFileWrite {
        path: PathBuf,
        error: io::Error,
    },
    EditorLaunch {
        command: String,
        path: PathBuf,
        error: io::Error,
    },
    EditorExit {
        command: String,
        path: PathBuf,
        status: ExitStatus,
    },
    TempFileRead {
        path: PathBuf,
        error: io::Error,
    },
    TempFileDecode {
        path: PathBuf,
        error: std::string::FromUtf8Error,
    },
    Extension(String),
}

impl ExternalEditorError {
    pub(crate) fn user_message(&self) -> String {
        match self {
            ExternalEditorError::NoEditorConfigured => {
                "Cannot open an external editor: set $VISUAL or $EDITOR, or install a default editor."
                    .to_string()
            }
            ExternalEditorError::InvalidCommand(cmd) => format!(
                "Unable to parse editor command \"{cmd}\". Check $VISUAL / $EDITOR."
            ),
            ExternalEditorError::TerminalRestore(error) => format!(
                "Could not hand control to the editor (restoring terminal state failed): {error}"
            ),
            ExternalEditorError::TempFileCreate(error) => {
                format!("Failed to create a temporary file for the editor: {error}")
            }
            ExternalEditorError::TempFileWrite { path, error } => format!(
                "Failed to write composer text to temporary file {path:?}: {error}"
            ),
            ExternalEditorError::EditorLaunch {
                command,
                path,
                error,
            } => format!(
                "Failed to launch editor \"{command}\" for temporary file {path:?}: {error}"
            ),
            ExternalEditorError::EditorExit {
                command,
                path,
                status,
            } => {
                let code = status
                    .code()
                    .map_or_else(|| "signal".to_string(), |c| c.to_string());
                format!(
                    "Editor \"{command}\" exited with status {code} for temporary file {path:?}; keeping the existing text."
                )
            }
            ExternalEditorError::TempFileRead { path, error } => {
                format!("Failed to read edited text from {path:?}: {error}")
            }
            ExternalEditorError::TempFileDecode { path, error } => {
                format!("Edited file {path:?} is not valid UTF-8: {error}")
            }
            ExternalEditorError::Extension(msg) => {
                format!("Extension error: {msg}")
            }
        }
    }
}

pub(crate) fn launch_external_editor(
    initial_text: &str,
    override_command: &Option<Vec<String>>,
) -> Result<String, ExternalEditorError> {
    let editor_command = resolve_editor_command(override_command)?;
    let (temp_path, path_buf) = create_temp_file(initial_text)?;

    let mut terminal_guard = TerminalModeGuard::new();
    if let Some(error) = terminal_guard.take_restore_error() {
        return Err(ExternalEditorError::TerminalRestore(error));
    }

    run_editor(&editor_command, &path_buf)?;

    // Re-enable TUI modes immediately after the editor closes.
    drop(terminal_guard);

    let edited = fs::read(&path_buf).map_err(|error| ExternalEditorError::TempFileRead {
        path: path_buf.clone(),
        error,
    })?;
    let mut text =
        String::from_utf8(edited).map_err(|error| ExternalEditorError::TempFileDecode {
            path: path_buf.clone(),
            error,
        })?;

    text = trim_trailing_newline(text);

    // Clean up the temporary file.
    drop(temp_path);

    Ok(text)
}

fn resolve_editor_command(
    override_command: &Option<Vec<String>>,
) -> Result<Vec<String>, ExternalEditorError> {
    if let Some(command) = override_command {
        return Ok(command.clone());
    }
    if let Ok(value) = env::var("VISUAL")
        && let Some(command) = normalize_editor_value(value)?
    {
        return Ok(command);
    }
    if let Ok(value) = env::var("EDITOR")
        && let Some(command) = normalize_editor_value(value)?
    {
        return Ok(command);
    }
    if let Some(command) = default_editor_command() {
        return Ok(command);
    }
    Err(ExternalEditorError::NoEditorConfigured)
}

fn normalize_editor_value(value: String) -> Result<Option<Vec<String>>, ExternalEditorError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed =
        shlex::split(trimmed).ok_or_else(|| ExternalEditorError::InvalidCommand(value.clone()))?;
    Ok(Some(parsed))
}

fn default_editor_command() -> Option<Vec<String>> {
    if cfg!(windows) {
        return Some(vec!["notepad".to_string()]);
    }
    Some(vec!["nano".to_string()])
}

fn create_temp_file(initial_text: &str) -> Result<(TempPath, PathBuf), ExternalEditorError> {
    let mut file = Builder::new()
        .prefix("codex-compose-")
        .suffix(".txt")
        .tempfile()
        .map_err(ExternalEditorError::TempFileCreate)?;
    let path = file.path().to_path_buf();

    file.write_all(initial_text.as_bytes()).map_err(|error| {
        ExternalEditorError::TempFileWrite {
            path: path.clone(),
            error,
        }
    })?;
    file.flush()
        .map_err(|error| ExternalEditorError::TempFileWrite {
            path: path.clone(),
            error,
        })?;

    let temp_path = file.into_temp_path();
    Ok((temp_path, path))
}

fn run_editor(command: &[String], path: &PathBuf) -> Result<(), ExternalEditorError> {
    let status = Command::new(&command[0])
        .args(&command[1..])
        .arg(path)
        .status()
        .map_err(|error| ExternalEditorError::EditorLaunch {
            command: join_command(command),
            path: path.clone(),
            error,
        })?;

    if !status.success() {
        return Err(ExternalEditorError::EditorExit {
            command: join_command(command),
            path: path.clone(),
            status,
        });
    }
    Ok(())
}

fn join_command(command: &[String]) -> String {
    command.join(" ")
}

fn trim_trailing_newline(mut text: String) -> String {
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    } else if text.ends_with('\r') {
        text.pop();
    }
    text
}

struct TerminalModeGuard {
    restore_error: Option<io::Error>,
}

impl TerminalModeGuard {
    fn new() -> Self {
        let restore_error = crate::tui::restore().err();
        Self { restore_error }
    }

    fn take_restore_error(&mut self) -> Option<io::Error> {
        self.restore_error.take()
    }
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        if let Err(error) = crate::tui::set_modes() {
            warn!(
                ?error,
                "failed to re-enable terminal modes after external editor"
            );
        }
    }
}
