# 原生库（.so）目录

把编译好的 Phira-Vrenxz 原生库放入对应 ABI 文件夹：

```
app/src/main/jniLibs/
└── arm64-v8a/        libphira_vrenxz.so   （真机，当前唯一打包的 ABI）
```

> 没有 `armeabi-v7a` 配套的 `libc++_shared.so`，也没有 `x86_64` 预编译 FFmpeg，故只支持 arm64-v8a。

## 编译命令

```powershell
# 在仓库根目录执行（必须显式开启 video，否则谱面内嵌视频会被直接丢弃）
cargo ndk -t arm64-v8a -o Android/app/src/main/jniLibs build --release -p Phira-Vrenxz --features video
```

> 视频解码依赖 `prpr-avc/static-lib/<target>/` 里的 FFmpeg 静态库。
> Android 端临时视频文件写入应用 cache 目录（不是不可写的 `/data/local/tmp`）。

> crate 名是 `Phira-Vrenxz`，其 `[lib] name = "phira_vrenxz"`，
> 生成的库名为 `libphira_vrenxz.so`（对应 `MainActivity.LIBRARY_NAME = "phira_vrenxz"`）。

## 如何自由替换 .so

1. 用 `cargo ndk` 把 Phira-Vrenxz 编译成 Android 动态库 `libphira_vrenxz.so`；
2. 把新的 `libphira_vrenxz.so` 覆盖到对应 ABI 目录；
3. 重新构建 APK 即可 —— 无需改动任何 Java 代码。

> 若你的 .so 名称不是 `libphira_vrenxz.so`，
> 请同步修改 `MainActivity.LIBRARY_NAME` 常量（不含 `lib` 前缀与 `.so` 后缀）。

`app/build.gradle.kts` 中已设置 `useLegacyPackaging = true`，
保证从 APK 中提取解压原生库，兼容 Android 6.0 及任意替换场景。
