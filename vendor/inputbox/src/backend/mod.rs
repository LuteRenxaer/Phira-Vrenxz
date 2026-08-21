//! Backend implementations for different platforms.
//!
//! This module provides platform-specific backends for showing input dialogs:
//! - `PSScript`: PowerShell script backend for Windows (Windows only)
//! - `JXAScript`: JavaScript for Automation backend for macOS (macOS only)
//! - `Android`: Android backend using JNI (Android only)
//! - `IOS`: iOS backend using UIKit (iOS only)
//! - `Yad`: [`yad`](https://github.com/v1cont/yad) backend.
//! - `Zenity`: Zenity backend.
//!
//! # Default behaviors
//!
//! Many fields in [`InputBox`] are optional and have default behavior. For
//! example, if `title` is not set, a default title will be used. However the
//! exact default values and behaviors may vary between backends (some uses
//! constants defined in this crate like
//! [`DEFAULT_OK_LABEL`](crate::DEFAULT_OK_LABEL) which ae not localized, while
//! others uses system defaults). See the *Defaults* section in each backend's
//! documentation for details.

use std::{
    borrow::Cow,
    io::{self, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
};

use cfg_if::cfg_if;

use crate::InputBox;

mod general;
pub use general::{Yad, Zenity};

#[cfg(target_os = "windows")]
mod windows;
use which::which;
#[cfg(target_os = "windows")]
pub use windows::PSScript;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::JXAScript;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::Android;

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
pub use ios::IOS;

#[cfg(target_env = "ohos")]
mod ohos;
#[cfg(target_env = "ohos")]
pub use ohos::OHOS;

/// Trait for platform-specific input box backends.
///
/// Implement this trait to add support for different dialog implementations.
/// See [`Zenity`] for an example (other backends are available on their
/// respective platforms).
pub trait Backend {
    /// Executes the input box with the given configuration, and calls the callback
    /// with the result when done.
    ///
    /// The callback will be called with `Ok(Some(input))` if the user clicked
    /// OK and entered text, `Ok(None)` if the user clicked Cancel or closed the
    /// dialog, or `Err(error)` if there was an error showing the dialog.
    fn execute_async(
        &self,
        input: &InputBox,
        callback: Box<dyn FnOnce(io::Result<Option<String>>) + Send>,
    ) -> io::Result<()>;

    /// Synchronous version of `execute_async` that blocks the calling thread
    /// until the user responds.
    ///
    /// The default implementation calls `execute_async` and then blocks on a
    /// channel receive. Some backends may override this with a more efficient
    /// implementation.
    fn execute(&self, input: &InputBox) -> io::Result<Option<String>> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.execute_async(
            input,
            Box::new(move |result| {
                let _ = tx.send(result);
            }),
        )?;
        match rx.recv() {
            Ok(result) => result,
            Err(_) => Ok(None),
        }
    }
}

/// Backends that utilize command-line tools.
trait CommandBackend {
    fn build_command<'a>(&self, input: &'a InputBox<'a>) -> (Command, Option<Cow<'a, str>>);
}

fn spawn_command((mut cmd, stdin): (Command, Option<Cow<str>>), quiet: bool) -> io::Result<Child> {
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(if quiet {
        Stdio::null()
    } else {
        Stdio::inherit()
    });
    let mut child = cmd.spawn()?;
    if let Some(input) = stdin {
        child.stdin.take().unwrap().write_all(input.as_bytes())?;
    }
    Ok(child)
}
fn wait_child(child: Child) -> io::Result<Option<String>> {
    let output = child.wait_with_output();
    output.map(|output| {
        if output.status.success() {
            Some(
                String::from_utf8_lossy(&output.stdout)
                    .trim_end()
                    .to_string(),
            )
        } else {
            None
        }
    })
}

impl<T: CommandBackend> Backend for T {
    fn execute_async(
        &self,
        input: &InputBox,
        callback: Box<dyn FnOnce(io::Result<Option<String>>) + Send>,
    ) -> io::Result<()> {
        let child = spawn_command(self.build_command(input), input.quiet)?;
        thread::spawn(move || {
            callback(wait_child(child));
        });
        Ok(())
    }

    fn execute(&self, input: &InputBox) -> io::Result<Option<String>> {
        let child = spawn_command(self.build_command(input), input.quiet)?;
        wait_child(child)
    }
}

pub fn default_backend() -> Box<dyn Backend> {
    if which("yad").is_ok() {
        return Box::new(Yad::default());
    }

    cfg_if! {
        if #[cfg(target_os = "windows")] {
            Box::new(PSScript::default())
        } else if #[cfg(target_os = "macos")] {
            Box::new(JXAScript::default())
        } else if #[cfg(target_os = "android")] {
            Box::new(Android::default())
        } else if #[cfg(target_os = "ios")] {
            Box::new(IOS::default())
        } else if #[cfg(target_env = "ohos")] {
            Box::new(OHOS::default())
        } else {
            Box::new(Zenity::default())
        }
    }
}
