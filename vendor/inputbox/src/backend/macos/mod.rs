use std::{borrow::Cow, path::Path, process::Command};

use crate::{
    backend::CommandBackend, InputBox, DEFAULT_CANCEL_LABEL, DEFAULT_OK_LABEL, DEFAULT_TITLE,
};

const JXA_SCRIPT: &str = include_str!("inputbox.jxa.js");

/// JXA (JavaScript for Automation) backend for macOS.
///
/// This backend uses the `osascript` command to execute a JavaScript for
/// Automation (JXA) script that displays an input dialog.
///
/// # Limitations
///
/// - Does not support [`InputMode::Multiline`] mode (falls back to single-line
///   input).
///
/// # Defaults
///
/// - `title`: `DEFAULT_TITLE`
/// - `prompt`: empty
/// - `cancel_label`: `DEFAULT_CANCEL_LABEL`
/// - `ok_label`: `DEFAULT_OK_LABEL`
#[derive(Clone, Debug)]
pub struct JXAScript {
    path: Cow<'static, Path>,
}

impl JXAScript {
    /// Creates a new backend using the default `osascript` command.
    pub fn new() -> Self {
        Self {
            path: Path::new("osascript").into(),
        }
    }

    /// Creates a new backend with a custom path to the osascript executable.
    pub fn custom(path: impl Into<Cow<'static, Path>>) -> Self {
        Self { path: path.into() }
    }
}

impl Default for JXAScript {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandBackend for JXAScript {
    fn build_command<'a>(&self, input: &'a InputBox<'a>) -> (Command, Option<Cow<'a, str>>) {
        let cancel_label = input
            .cancel_label
            .as_deref()
            .unwrap_or(DEFAULT_CANCEL_LABEL);
        let ok_label = input.ok_label.as_deref().unwrap_or(DEFAULT_OK_LABEL);
        let value = serde_json::json!({
            "title": input.title.as_deref().unwrap_or(DEFAULT_TITLE),
            "prompt": input.prompt,
            "default": input.default,
            "mode": input.mode.as_str(),
            "width": input.width,
            "height": input.height,
            "cancel_label": cancel_label,
            "ok_label": ok_label,
            "auto_wrap": input.auto_wrap,
            "scroll_to_end": input.scroll_to_end,
        });
        let stdin = value.to_string();

        let mut cmd = Command::new(&*self.path);
        cmd.args(["-l", "JavaScript", "-e", JXA_SCRIPT]);

        (cmd, Some(Cow::Owned(stdin)))
    }
}
