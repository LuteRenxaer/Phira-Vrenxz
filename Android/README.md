# Phira-Vrenxz Android 外壳

这是一个纯 Java 编写的 Android 外壳工程，用于启动
Phira-Vrenxz 编译出的原生库（`libphira_vrenxz.so`，miniquad/macroquad 引擎）。

**最小兼容 Android 6.0（API 23）**，`.so` 可自由替换、无需改动任何 Java 代码。

## 目录结构

```
app/src/main/
├── java/
│   ├── quad_native/QuadNative.java      # 原生 JNI 方法声明（类名不可改）
│   ├── com/teamflos/phirlie/
│   │   ├── MainActivity.java            # 主界面：生命周期 + 文件/SAF 桥接
│   │   ├── QuadSurface.java             # SurfaceView：触摸/按键转发
│   │   └── ResizingLayout.java          # insets 处理布局
│   └── moe/mivik/inputbox/InputBox.java # 输入框（inputbox 后端）
├── jniLibs/<abi>/libphira_vrenxz.so     # 原生库（自行放入）
├── assets/                              # 游戏资源（fonts/、zip 包等，自行放入）
├── res/                                 # 资源
└── AndroidManifest.xml
```

## 一、编译 Phira-Vrenxz 原生库（.so）

Phira-Vrenxz 的 `Phira-Vrenxz` crate（lib 名 `phira_vrenxz`，cdylib）会生成
`libphira_vrenxz.so`。需要安装 **Android NDK**，然后用 `cargo ndk` 交叉编译：

```bash
# 安装 cargo-ndk（一次即可）
cargo install cargo-ndk

# 在仓库根目录下为各 ABI 编译 release 库
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
    -o Android/app/src/main/jniLibs \
    build --release -p Phira-Vrenxz
```

> crate 名是 `Phira-Vrenxz`，`[lib] name = "phira_vrenxz"`，
> 生成的库名为 `libphira_vrenxz.so`（对应 `MainActivity.LIBRARY_NAME`）。
> 若需要 `hykb`（好游快爆）等 feature：`--features Phira-Vrenxz/hykb`。
> 依赖的 git 依赖（macroquad/miniquad 等）由 Cargo.lock 锁定，可正常编译。

## 二、放置资源（assets）

miniquad 通过 `AAssetManager` 从 APK 的 `assets/` 读取资源：

```bash
# Windows PowerShell
Copy-Item -Path assets/* -Destination Android/app/src/main/assets/ -Recurse -Force
```

## 三、构建 / 安装 APK

用 Android Studio 打开 `Android/` 目录，或命令行：

```bash
cd Android
./gradlew :app:assembleDebug     # 或 assembleRelease
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## 四、自由替换 .so

1. 把新的 `libphira_vrenxz.so` 覆盖到 `app/src/main/jniLibs/<abi>/`；
2. 重新 `assemble` 打包即可，**不需要改动任何 Java 代码**。

若你的库名不是 `libphira_vrenxz.so`，把 `MainActivity.LIBRARY_NAME`
（`"phira_vrenxz"`）改成对应的库名即可。

> 已在 `AndroidManifest.xml` 设置 `android:hardwareAccelerated="false"`，
> 避免部分模拟器（如 MuMu）HWUI 崩溃；同时以 `useLegacyPackaging` 方式
> 让 .so 从 APK 中解压加载，兼容 Android 6.0 及各种替换场景。

## 五、JNI 契约（Java ⇄ .so）

### Java 声明为 native、由 .so 实现（调用原生）

| Java 方法（`quad_native.QuadNative`，全部 static） | 作用 |
| --- | --- |
| `initializeContext(Activity)` | 初始化 ndk_context，须最先调用 |
| `releaseContext()` | 释放 JNI 上下文 |
| `activityOnCreate(Activity)` | 触发 Rust 侧 `quad_main()`，启动渲染线程 |
| `activityOnResume/Pause/Destroy()` | 渲染线程生命周期 |
| `prprActivityOnPause/Resume/Destroy()` | 游戏暂停/恢复钩子 |
| `surfaceOnSurfaceCreated/Destroyed/Changed(...)` | Surface 生命周期 |
| `surfaceOnTouch(id, action, x, y, time)` | 触摸事件 |
| `surfaceOnKeyDown/Up(int)`, `surfaceOnCharacter(int)` | 键盘输入 |
| `initializeEnvironment()` | 初始化 inputbox 输入框后端 |
| `setDataPath(String)` / `setTempDir(String)` | 设置数据/缓存目录 |
| `setDpi(int)` | 设置 DPI |
| `setChosenFile(String)` / `markImport()` / `markImportRespack()` / `markAutoImport()` | 文件导入 |
| `setInputText(String)` | 输入框结果 |
| `setStartupArgs(join, create, server)` | 深链接（phira://）多人启动参数 |
| `processExportFd(Uri, int)` | SAF 导出 fd |

`moe.mivik.inputbox.InputBox`：`inputCallback(long, String)`。

> **注意：** 类 `quad_native.QuadNative` 与 `moe.mivik.inputbox.InputBox`
> 的**包名/类名/方法签名是硬绑定**的（对应 JNI 导出符号），请勿改动。
> 每个声明的 native 方法都必须有对应 .so 导出；删除某方法时请把声明与
> 调用处一起移除，否则会在运行时抛 `UnsatisfiedLinkError`。
> 业务类位于 `com.teamflos.phirlie` 包。

### Java 实现、由 .so 调用（在 MainActivity 上）

| 方法 | 作用 |
| --- | --- |
| `chooseFile()` | 打开文件选择器（导入谱面/资源包/头像等） |
| `chooseFolder()` | 打开文件夹选择器（导入自定义资源目录） |
| `showExportDialog(String)` | SAF 创建文档导出 |
| `deleteUri(Uri)` | 删除导出的临时文件 |
| `openUrl(String)` | 打开链接 |
| `antiAddiction(String, String)` | 防沉迷 |
| `copy(String)` | 剪贴板 |
| `setFullScreen(boolean)` / `showKeyboard(boolean)` | 全屏 / 软键盘 |

> 输入法（IME）文本由引擎按键事件驱动：`QuadSurface` 把 IME 的字符提交
> 转成 `surfaceOnCharacter`、把退格转成 `KEYCODE_DEL` 按键事件（引擎侧会
> 映射为 Backspace），因此不再需要额外的 `inputBackspace/inputSelectAll`
> 等专用 JNI。

## 说明

- 应用标识（applicationId）：`com.teamflos.PhiraVrenxz`
- 数据目录：`getFilesDir()/data/`（存放 `data.json`、`charts/`、`collections/`、`respack/`）。
- 缓存目录：`getCacheDir()/`（作为 `TMPDIR`）。
- 运行所需联网权限已在清单中声明。
- 深链接协议：`phira://room/join/<code>`、`phira://room/create/<id>`（可带 `?server=`）。
