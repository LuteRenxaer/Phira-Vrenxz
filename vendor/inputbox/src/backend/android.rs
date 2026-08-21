use std::io;

use jni::{
    errors::ThrowRuntimeExAndDefault,
    jni_sig, jni_str,
    objects::{JClass, JObject, JString, JValue},
    refs::Global,
    signature::MethodSignature,
    sys::{jlong, JNIEnv},
    Env, EnvUnowned, JavaVM, Outcome,
};
use once_cell::sync::OnceCell;

use crate::{InputBox, DEFAULT_CANCEL_LABEL, DEFAULT_OK_LABEL, DEFAULT_TITLE};

use super::Backend;

static JAVA_CLASS: OnceCell<Global<JClass>> = OnceCell::new();

struct IoErrorWrapper(io::Error);

impl From<jni::errors::Error> for IoErrorWrapper {
    fn from(value: jni::errors::Error) -> Self {
        IoErrorWrapper(io::Error::new(io::ErrorKind::Other, value))
    }
}
impl From<io::Error> for IoErrorWrapper {
    fn from(value: io::Error) -> Self {
        Self(value)
    }
}

/// Android backend for InputBox.
///
/// This backend uses JNI to call into the Android InputBox AAR library. The AAR
/// provides a native Android dialog implementation.
///
/// # Setup
///
/// To use this backend, you need to:
///
/// 1. Add the `inputbox-android` AAR to your Android project.
/// 2. Use `System.loadLibrary` to load your native library containing this
///    crate.
/// 3. Call [`Android::initialize`] to initialize the backend.
///
/// # Limitations
///
/// - `width` and `height` options are ignored.
///
/// # Defaults
///
/// - `title`: `DEFAULT_TITLE`
/// - `prompt`: empty
/// - `cancel_label`: `DEFAULT_CANCEL_LABEL`
/// - `ok_label`: `DEFAULT_OK_LABEL`
#[derive(Debug, Clone, Default)]
pub struct Android {
    _priv: (),
}

impl Android {
    /// Creates a new Android backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initializes the backend.
    pub fn initialize(env: &mut Env) -> jni::errors::Result<()> {
        JAVA_CLASS.get_or_try_init(|| -> jni::errors::Result<_> {
            let java_class = env.find_class(jni_str!("moe/mivik/inputbox/InputBox"))?;
            let java_class = env.new_global_ref(java_class)?;
            Ok(java_class)
        })?;
        Ok(())
    }

    /// Initializes the backend with a raw JNI environment pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the provided `JNIEnv` pointer is valid and
    /// that this function is called when current thread is attached to the JVM.
    pub unsafe fn initialize_raw(env: *mut JNIEnv) -> jni::errors::Result<()> {
        let mut env = unsafe { EnvUnowned::from_raw(env) };
        match env.with_env_no_catch(Self::initialize).into_outcome() {
            Outcome::Ok(()) => Ok(()),
            Outcome::Err(err) => Err(err),
            Outcome::Panic(_) => unreachable!(),
        }
    }

    fn show_dialog(
        &self,
        input: &InputBox,
        callback: Box<dyn FnOnce(io::Result<Option<String>>) + Send>,
    ) -> Result<(), IoErrorWrapper> {
        const SHOW_INPUT_SIG: MethodSignature = jni_sig!(
            (
                callback: jlong,
                title: JString,
                prompt: JString,
                default: JString,
                ok_label: JString,
                cancel_label: JString,
                mode: JString,
                auto_wrap: bool,
                scroll_to_end: bool
            ) -> JString
        );

        let java_class = JAVA_CLASS.get().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "Android activity not set. Call Android::initialize* first.",
            )
        })?;
        JavaVM::singleton()?.attach_current_thread(|env| -> Result<(), IoErrorWrapper> {
            let title = env.new_string(input.title.as_deref().unwrap_or(DEFAULT_TITLE))?;
            #[allow(clippy::redundant_closure)]
            let prompt = input
                .prompt
                .as_ref()
                .map(|it| env.new_string(it))
                .transpose()?
                .map_or_else(|| JObject::null(), |s| s.into());
            let default = env.new_string(&input.default)?;
            let ok_label = env.new_string(input.ok_label.as_deref().unwrap_or(DEFAULT_OK_LABEL))?;
            let cancel_label = env.new_string(
                input
                    .cancel_label
                    .as_deref()
                    .unwrap_or(DEFAULT_CANCEL_LABEL),
            )?;
            let mode = env.new_string(input.mode.as_str())?;

            let result = env
                .call_static_method(
                    java_class,
                    jni_str!("showInput"),
                    SHOW_INPUT_SIG,
                    &[
                        JValue::Long(Box::into_raw(Box::new(callback)) as _),
                        (&title).into(),
                        (&prompt).into(),
                        (&default).into(),
                        (&ok_label).into(),
                        (&cancel_label).into(),
                        (&mode).into(),
                        input.auto_wrap.into(),
                        input.scroll_to_end.into(),
                    ],
                )?
                .l()?;
            if !result.is_null() {
                let result = JString::cast_local(env, result)?;
                Err(io::Error::new(io::ErrorKind::Other, result.to_string()).into())
            } else {
                Ok(())
            }
        })
    }
}

impl Backend for Android {
    fn execute_async(
        &self,
        input: &InputBox,
        callback: Box<dyn FnOnce(io::Result<Option<String>>) + Send>,
    ) -> io::Result<()> {
        self.show_dialog(input, callback).map_err(|err| err.0)
    }
}

#[unsafe(export_name = "Java_moe_mivik_inputbox_InputBox_inputCallback")]
extern "system" fn input_callback(
    mut env: EnvUnowned,
    _class: JClass,
    callback: jlong,
    text: JString,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let text: Option<String> = if text.is_null() {
            None
        } else {
            Some(text.try_to_string(env)?)
        };
        let callback = unsafe {
            Box::from_raw(callback as *mut Box<dyn FnOnce(io::Result<Option<String>>) + Send>)
        };
        callback(Ok(text));
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}
