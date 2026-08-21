use std::{
    io,
    ops::Deref,
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use block2::StackBlock;
use objc2::{rc::Retained, MainThreadMarker};
use objc2_core_foundation::{CGFloat, CGRect, CGSize};
use objc2_foundation::{ns_string, NSArray, NSObjectNSKeyValueCoding, NSRange, NSString};
use objc2_ui_kit::{
    NSLayoutConstraint, UIAlertAction, UIAlertActionStyle, UIAlertController,
    UIAlertControllerStyle, UIApplication, UIFont, UITextField, UITextInputTraits, UITextView,
    UIViewController, UIWindowScene,
};

use crate::{backend::Backend, InputMode, DEFAULT_CANCEL_LABEL, DEFAULT_OK_LABEL, DEFAULT_TITLE};

/// iOS backend for InputBox using `UIAlertController`.
///
/// # Warnings
///
/// - **Main thread only.** `execute_async` checks for the main thread via
///   [`MainThreadMarker`] and returns an error if called from any other thread.
/// - **Never use the sync methods.** Calling `execute` (or the `show` /
///   `show_with` helpers on [`InputBox`](crate::InputBox)) on the main thread
///   will block it while waiting for the user to respond. Because UIKit relies
///   on the main run loop to deliver events — including the user tapping a
///   button in the presented alert — the dialog will never appear or be
///   dismissable, and the call will **deadlock**. Always use `execute_async` or
///   the `show_async` / `show_with_async` helpers instead.
///
/// # Limitations
///
/// - `width` and `height` only affect the size of the text area when using
///   `InputMode::Multiline`.
/// - `auto_wrap` is ignored (iOS `UITextView` wraps by default).
///
/// # Defaults
///
/// - `title`: `DEFAULT_TITLE`
/// - `prompt`: empty
/// - `cancel_label`: `DEFAULT_CANCEL_LABEL`
/// - `ok_label`: `DEFAULT_OK_LABEL`
#[derive(Default)]
pub struct IOS {
    _priv: (),
}

impl IOS {
    /// Creates a new IOS backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Helper function to get the topmost view controller for presenting the alert.
    ///
    /// Returns `None` if no active window or view controller is found.
    pub fn get_top_view_controller(mtm: MainThreadMarker) -> Option<Retained<UIViewController>> {
        let key_window = UIApplication::sharedApplication(mtm)
            .connectedScenes()
            .iter()
            .filter_map(|scene| scene.downcast::<UIWindowScene>().ok())
            .find_map(|scene| scene.keyWindow())?;
        let mut top_vc = key_window.rootViewController()?;
        while let Some(presented) = top_vc.presentedViewController() {
            top_vc = presented;
        }
        Some(top_vc)
    }
}

impl Backend for IOS {
    fn execute_async(
        &self,
        input: &crate::InputBox,
        callback: Box<dyn FnOnce(io::Result<Option<String>>) + Send>,
    ) -> io::Result<()> {
        let callback = Arc::new(Mutex::new(Some(callback)));

        let mtm = MainThreadMarker::new().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "IOS backend can only be used on main thread",
            )
        })?;

        let title = input.title.as_deref().unwrap_or(DEFAULT_TITLE);
        let prompt_ns = input.prompt.as_deref().map(NSString::from_str);

        let alert = UIAlertController::alertControllerWithTitle_message_preferredStyle(
            Some(&NSString::from_str(title)),
            prompt_ns.as_deref(),
            UIAlertControllerStyle::Alert,
            mtm,
        );

        let mode = input.mode.clone();
        let default = input.default.to_string();
        let text_view = if mode == InputMode::Multiline {
            let vc = UIViewController::new(mtm);
            let w = input.width.map(|v| v as CGFloat).unwrap_or(270.0);
            let h = input.height.map(|v| v as CGFloat).unwrap_or(150.0);
            let size = CGSize {
                width: w,
                height: h,
            };
            vc.setPreferredContentSize(size);

            let root_view = vc.view().unwrap();

            let text_view = UITextView::new(mtm);
            text_view.setFrame(CGRect {
                origin: Default::default(),
                size,
            });
            text_view.setFont(Some(&UIFont::systemFontOfSize(16.0)));
            text_view.layer().setBorderWidth(0.5);
            text_view.layer().setCornerRadius(5.);

            let text_ns = NSString::from_str(&default);
            text_view.setText(Some(&text_ns));

            if input.scroll_to_end {
                let length = text_ns.length();
                text_view.scrollRangeToVisible(NSRange {
                    location: length,
                    length: 0,
                });
            }

            root_view.addSubview(&text_view);

            text_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            NSLayoutConstraint::activateConstraints(
                &NSArray::from_slice(&[
                    text_view
                        .leadingAnchor()
                        .constraintEqualToAnchor_constant(&root_view.leadingAnchor(), 10.0)
                        .deref(),
                    text_view
                        .trailingAnchor()
                        .constraintEqualToAnchor_constant(&root_view.trailingAnchor(), -10.0)
                        .deref(),
                    text_view
                        .topAnchor()
                        .constraintEqualToAnchor_constant(&root_view.topAnchor(), 10.0)
                        .deref(),
                    text_view
                        .bottomAnchor()
                        .constraintEqualToAnchor_constant(&root_view.bottomAnchor(), -10.0)
                        .deref(),
                ]),
                mtm,
            );

            unsafe {
                alert.setValue_forKey(Some(&vc), ns_string!("contentViewController"));
            }

            Some(text_view)
        } else {
            let mode_clone = mode.clone();
            let default_clone = default.clone();
            alert.addTextFieldWithConfigurationHandler(Some(&StackBlock::new(
                move |field: NonNull<UITextField>| {
                    let field = unsafe { field.as_ref() };
                    field.setText(Some(&NSString::from_str(&default_clone)));
                    if mode_clone == InputMode::Password {
                        field.setSecureTextEntry(true);
                    }
                },
            )));

            None
        };

        let cancel_label = input
            .cancel_label
            .as_deref()
            .unwrap_or(DEFAULT_CANCEL_LABEL);
        let cancel_action = UIAlertAction::actionWithTitle_style_handler(
            Some(&NSString::from_str(cancel_label)),
            UIAlertActionStyle::Cancel,
            Some(&StackBlock::new({
                let callback = callback.clone();
                move |_| {
                    if let Some(cb) = { callback.lock().unwrap().take() } {
                        cb(Ok(None));
                    }
                }
            })),
            mtm,
        );
        alert.addAction(&cancel_action);

        let ok_label = input.ok_label.as_deref().unwrap_or(DEFAULT_OK_LABEL);
        let ok_action = UIAlertAction::actionWithTitle_style_handler(
            Some(&NSString::from_str(ok_label)),
            UIAlertActionStyle::Default,
            Some(&StackBlock::new({
                let alert = alert.clone();
                let callback = callback.clone();
                move |_| {
                    if let Some(cb) = { callback.lock().unwrap().take() } {
                        let text = if let Some(tv) = &text_view {
                            tv.text().to_string()
                        } else {
                            let fields = alert.textFields().unwrap().firstObject().unwrap();
                            fields.text().map_or_else(String::new, |s| s.to_string())
                        };
                        cb(Ok(Some(text)));
                    }
                }
            })),
            mtm,
        );
        alert.addAction(&ok_action);

        let top_vc = Self::get_top_view_controller(mtm).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "no active window or view controller found to present the input dialog",
            )
        })?;
        top_vc.presentViewController_animated_completion(&alert, true, None);

        Ok(())
    }
}
