# 原生库（.so）目录

把编译好的 Phira-Vrenxz 原生库放入对应 ABI 文件夹：

```
app/src/main/jniLibs/
├── arm64-v8a/        libphira_vrenxz.so   （绝大多数现代手机）
├── armeabi-v7a/      libphira_vrenxz.so   （32 位老设备，如需支持请加该 ABI）
└── x86_64/           libphira_vrenxz.so   （模拟器）
```

## 编译命令

```powershell
# 在仓库根目录执行
cargo ndk -t arm64-v8a -t x86_64 -o Android/app/src/main/jniLibs build --release -p Phira-Vrenxz
```

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
