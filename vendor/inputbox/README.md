# inputbox

[![crates.io](https://img.shields.io/crates/v/inputbox)](https://crates.io/crates/inputbox)
[![docs.rs](https://img.shields.io/docsrs/inputbox)](https://docs.rs/inputbox)
[![CI](https://github.com/Mivik/inputbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Mivik/inputbox/actions)

> A cross-platform, native GUI input box for Rust. Yes, finally.

## The Story

Picture this: you're writing a Rust CLI tool and you just want to pop up a little dialog that says _"hey, what's your name?"_ and take whatever the user types. Simple, right?

So you look at the ecosystem.

- **`rfd`**: Rusty File Dialogs! Cross-platform! Async! Beautiful! ...opens files. Not text. Files.
- **`native-dialog`**: Again, no input box, just message boxes and file pickers.
- **`tinyfiledialogs`**: `input_box` looks promising! ...but why does my input box turn into a password input when `default` is empty (`Some("")`)? No multiline, no custom labels, no control over backends... Oh and it's a C binding.
- **`dialog`**: Finally, an input box! ...but not for Windows or macOS. It fully depends on tools like `zenity`, `kdialog` or `dialog`.

You stare into the void. The void stares back. You write the dialog in HTML/JS because at least Electron works on all platforms.

_Not anymore._

## What `inputbox` Does

`inputbox` is a minimal, cross-platform Rust library that shows a native GUI input dialog and returns what the user typed. It uses whatever is available on the system. Should work™ most of the time.

## Quick Start

```toml
[dependencies]
inputbox = "0.1.4"
```

```rust
use inputbox::InputBox;

fn main() {
    let result = InputBox::new()
        .title("Greetings")
        .prompt("What's your name?")
        .show()
        .unwrap();

    match result {
        Some(name) => println!("Hello, {name}!"),
        None => println!("Fine, be that way."),
    }
}
```

## Examples

### Password Input

```rust
use inputbox::{InputBox, InputMode};

let result = InputBox::new()
    .title("Login")
    .prompt("Enter your password:")
    .mode(InputMode::Password)
    .show()
    .unwrap();
```

### Custom Buttons and Dimensions

```rust
use inputbox::InputBox;

let result = InputBox::new()
    .title("Confirm")
    .prompt("Are you sure?")
    .ok_label("Yes, do it")
    .cancel_label("No, abort")
    .width(400)
    .height(200)
    .show()
    .unwrap();
```

### Asynchronous Usage

```rust
use inputbox::InputBox;

InputBox::new()
    .title("Async Input")
    .prompt("Enter something:")
    .show_async(|result| {
        println!("Result: {:?}", result);
    });
```

> **Note:** On iOS, you must use async methods. See [the
> documentation](https://docs.rs/inputbox/latest/inputbox/struct.InputBox.html#sync-vs-async)
> for details.

## Features

- **Multiple input modes** — text, password, or multiline
- **Highly customizable** — title, prompt, button labels, dimensions, and more
- **Works on most platforms** — Windows, macOS, Linux, Android, iOS and OpenHarmony
- **Pluggable backends** — use a specific backend or let the library pick
- **Synchronous and asynchronous** — safe sync on most platforms, async required on iOS

## Backends

| Backend     | Platform    | How it works                                   | Extra setup                         |
| ----------- | ----------- | ---------------------------------------------- | ----------------------------------- |
| `PSScript`  | Windows     | PowerShell + WinForms, no extra install needed | None                                |
| `JXAScript` | macOS       | `osascript` JXA, built into the OS             | None                                |
| `Android`   | Android     | AAR + JNI to show an Android AlertDialog       | See [Android Setup](#android-setup) |
| `IOS`       | iOS         | UIKit alert                                    | None                                |
| `OHOS`      | OpenHarmony | NAPI + ArkTS dialog                            | See [OHOS Setup](#ohos-setup)       |
| `Yad`       | Linux       | [`yad`](https://github.com/v1cont/yad)         | Install `yad`                       |
| `Zenity`    | Linux       | `zenity` — fallback on GNOME systems           | Install `zenity`                    |

### Android Setup

You need to include AAR in your Android project to use the Android backend. The AAR is bundled with the crate, but you need to configure your Gradle build to find it.

#### Gradle Setup

Inside of your project's `settings.gradle` file, add the following code and Maven repository definition.

`$PATH_TO_DEPENDENT_CRATE` is the relative path to the Cargo manifest (`Cargo.toml`) of any crate in your workspace that depends on `inputbox` from the location of your `settings.gradle` file:

```groovy
import groovy.json.JsonSlurper

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // ...Other repositories...
        maven {
            url = findCrateBundledProject("inputbox-android")
            metadataSources.artifact()
        }
    }
}

String findCrateBundledProject(packageName) {
    def dependencyText = providers.exec {
        commandLine("cargo", "metadata", "--format-version", "1", "--filter-platform", "aarch64-linux-android", "--manifest-path", "$PATH_TO_DEPENDENT_CRATE/Cargo.toml")
    }.standardOutput.asText.get()

    def dependencyJson = new JsonSlurper().parseText(dependencyText)
    def manifestPath = file(dependencyJson.packages.find { it.name == packageName }.manifest_path)
    return new File(manifestPath.parentFile, "maven").path
}
```

Then, wherever you declare your dependencies, add the following:

```groovy
implementation "moe.mivik:inputbox:latest.release"
```

#### Crate initialization

The `jni` crate must be initialized before the crate can interact with the JVM. Add the following code to your native library's initialization function:

```rust,ignore
// `jni` crate v0.22 or higher
#[export_name = "Java_com_orgname_android_rust_init"]
extern "system" fn java_init(
    env: EnvUnowned,
    _class: JClass,
) {
    let result = env.with_env(|env| -> jni::errors::Result<()> {
        inputbox::backend::Android::initialize(env)?;
        // ...
    });
    // ...
}

// Earlier versions of `jni` or other JNI bindings
#[export_name = "Java_com_orgname_android_rust_init"]
extern "system" fn java_init(
    env: JNIEnv,
    _class: JClass,
) {
    unsafe {
        inputbox::backend::Android::initialize_raw(env.as_raw() as *mut _)?;
    }
    // ...
}
```

### OHOS Setup

Currently you need to implement the ArkTS dialog yourself and use `registerInputboxCallback` to connect it to `inputbox`. See the [OHOS example](https://github.com/Mivik/inputbox/blob/9f5664f2b720cc9f002b0798d0cb43a0bb466858/src/backend/ohos.rs#L47) for setup details.

## License

MIT
