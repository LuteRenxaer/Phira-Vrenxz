use std::{borrow::Cow, path::Path, process::Command};

use serde_json::json;

use crate::{backend::CommandBackend, InputBox, DEFAULT_CANCEL_LABEL, DEFAULT_OK_LABEL};

const PS_SCRIPT: &str = include_str!("inputbox.ps1");

/// PowerShell script backend for Windows.
///
/// This backend uses PowerShell with WinForms to display input dialogs. It
/// works on Windows systems with PowerShell and .NET Framework installed.
///
/// # Caveats
///
/// - When [`InputMode::Multiline`](crate::InputMode::Multiline) is used, `\r\n`
///   might be used as line breaks in the returned string. This behavior is
///   intentionally preserved to maintain consistency with how multiline input
///   is typically handled in Windows applications.
///
/// # Defaults
///
/// - `title`: `DEFAULT_TITLE`
/// - `prompt`: empty
/// - `cancel_label`: `DEFAULT_CANCEL_LABEL`
/// - `ok_label`: `DEFAULT_OK_LABEL`
#[derive(Clone, Debug)]
pub struct PSScript {
    path: Cow<'static, Path>,
}

impl PSScript {
    /// Creates a new backend using the default `powershell` command.
    pub fn new() -> Self {
        Self {
            path: Path::new("powershell").into(),
        }
    }

    /// Creates a new backend with a custom path to the PowerShell executable.
    pub fn custom(path: impl Into<Cow<'static, Path>>) -> Self {
        Self { path: path.into() }
    }
}

impl Default for PSScript {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandBackend for PSScript {
    fn build_command<'a>(&self, input: &'a InputBox<'a>) -> (Command, Option<Cow<'a, str>>) {
        let cancel_label = input
            .cancel_label
            .as_deref()
            .unwrap_or(DEFAULT_CANCEL_LABEL);
        let ok_label = input.ok_label.as_deref().unwrap_or(DEFAULT_OK_LABEL);
        let value = json!({
            "title": input.title,
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
        cmd.args(["-NoProfile", "-NoLogo", "-Command", PS_SCRIPT]);

        (cmd, Some(Cow::Owned(stdin)))
    }
}
