use std::{borrow::Cow, path::Path, process::Command};

use crate::{backend::CommandBackend, InputBox, InputMode};

/// Zenity backend.
///
/// This backend uses the `zenity` command-line tool to display input dialogs.
/// It requires `zenity` to be installed on the system.
///
/// # Limitations
///
/// - Prompt text is not shown when [`InputMode::Multiline`] is used.
///
/// # Defaults
///
/// - `title`, `prompt`, `cancel_label`, `ok_label`: uses system defaults
#[derive(Clone, Debug)]
pub struct Zenity {
    path: Cow<'static, Path>,
}

impl Zenity {
    /// Creates a new backend using the default `zenity` command.
    pub fn new() -> Self {
        Self {
            path: Path::new("zenity").into(),
        }
    }

    /// Creates a new backend with a custom path to the zenity executable.
    pub fn custom(path: impl Into<Cow<'static, Path>>) -> Self {
        Self { path: path.into() }
    }
}

impl Default for Zenity {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandBackend for Zenity {
    fn build_command<'a>(&self, input: &'a InputBox<'a>) -> (Command, Option<Cow<'a, str>>) {
        let mut cmd = Command::new(&*self.path);
        let stdin = match input.mode {
            InputMode::Text | InputMode::Password => {
                cmd.arg("--entry");
                if input.mode == InputMode::Password {
                    cmd.arg("--hide-text");
                }
                cmd.args(["--entry-text", &*input.default]);
                None
            }
            InputMode::Multiline => {
                cmd.args(["--text-info", "--editable"]);
                if input.scroll_to_end {
                    cmd.arg("--auto-scroll");
                }
                Some(Cow::Borrowed(&*input.default))
            }
        };
        if let Some(title) = &input.title {
            cmd.args(["--title", title]);
        }
        if let Some(prompt) = &input.prompt {
            cmd.args(["--text", prompt]);
        }
        if let Some(label) = &input.cancel_label {
            cmd.args(["--cancel-label", label]);
        }
        if let Some(label) = &input.ok_label {
            cmd.args(["--ok-label", label]);
        }
        if let Some(width) = input.width {
            cmd.args(["--width", &width.to_string()]);
        }
        if let Some(height) = input.height {
            cmd.args(["--height", &height.to_string()]);
        }

        (cmd, stdin)
    }
}

/// [`yad`](https://github.com/v1cont/yad) backend.
///
/// This backend uses the `yad` command-line tool to display input dialogs. It
/// requires `yad` to be installed on the system.
///
/// # Limitations
///
/// - [`ok_label`](InputBox::ok_label) and
///   [`cancel_label`](InputBox::cancel_label) MUST NOT contain the item
///   separator character (default `!`). You can change the item separator using
///   [`with_item_separator`](Yad::with_item_separator).
///
/// # Defaults
///
/// - `title`, `prompt`, `cancel_label`, `ok_label`: uses system defaults
#[derive(Clone, Debug)]
pub struct Yad {
    path: Cow<'static, Path>,
    item_separator: u8,
}

impl Yad {
    /// Creates a new backend using the default `yad` command.
    pub fn new() -> Self {
        Self::custom(Path::new("yad"))
    }

    /// Creates a new backend with a custom path to the yad executable.
    pub fn custom(path: impl Into<Cow<'static, Path>>) -> Self {
        Self {
            path: path.into(),
            item_separator: b'!',
        }
    }

    /// Sets the item separator for the Yad backend.
    ///
    /// Yad uses a custom item separator to distinguish between button labels,
    /// and this separator MUST NOT appear in the button labels themselves. By
    /// default, it is set to `!` (ASCII 33).
    pub fn with_item_separator(mut self, sep: u8) -> Self {
        self.item_separator = sep;
        self
    }
}

impl Default for Yad {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandBackend for Yad {
    fn build_command<'a>(&self, input: &'a InputBox<'a>) -> (Command, Option<Cow<'a, str>>) {
        let mut cmd = Command::new(&*self.path);
        let stdin = match input.mode {
            InputMode::Text | InputMode::Password => {
                cmd.arg("--entry");
                if input.mode == InputMode::Password {
                    cmd.arg("--hide-text");
                }
                if let Some(prompt) = &input.prompt {
                    cmd.args(["--entry-label", prompt]);
                }
                cmd.args(["--entry-text", &*input.default]);
                None
            }
            InputMode::Multiline => {
                cmd.args(["--text-info", "--editable"]);
                if let Some(prompt) = &input.prompt {
                    cmd.args(["--text", prompt]);
                }
                if input.auto_wrap {
                    cmd.arg("--wrap");
                }
                if input.scroll_to_end {
                    cmd.arg("--auto-scroll");
                }
                Some(Cow::Borrowed(&*input.default))
            }
        };
        if let Some(title) = &input.title {
            cmd.args(["--title", title]);
        }
        if input.cancel_label.is_some() || input.ok_label.is_some() {
            let sep = char::from_u32(self.item_separator as _).unwrap();
            cmd.args(["--item-separator", &sep.to_string()]);

            cmd.arg("--button");
            if let Some(label) = &input.cancel_label {
                cmd.arg(format!("{label}{sep}gtk-cancel:1"));
            } else {
                cmd.arg("yad-cancel:1");
            }

            cmd.arg("--button");
            if let Some(label) = &input.ok_label {
                cmd.arg(format!("{label}{sep}gtk-ok:0"));
            } else {
                cmd.arg("yad-ok:0");
            }
        }
        if let Some(width) = input.width {
            cmd.args(["--width", &width.to_string()]);
        }
        if let Some(height) = input.height {
            cmd.args(["--height", &height.to_string()]);
        }

        (cmd, stdin)
    }
}
