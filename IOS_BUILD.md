# PhirLie iOS 构建指南

## 快速开始（推荐：GitHub Actions）

这是在 Windows 上获取 IPA 的唯一可靠方式——把代码推到 GitHub，CI 在 macOS 上自动构建。

### 1. 推送代码到 GitHub

```bash
git add .
git commit -m "feat: add iOS build pipeline"
git push origin main
```

### 2. 触发构建

- 推送到 `main` / `ios` 分支或打 `v*` tag 会自动触发
- 或在 GitHub → Actions → "iOS Build (IPA)" → Run workflow 手动触发

### 3. 下载 IPA

构建完成后，在 workflow run 页面的 **Artifacts** 区域下载 `PhirLie-ios-ipa`。

---

## 本地构建（需要 Mac）

如果你有一台 Mac，可以直接在本地构建：

```bash
# 安装依赖
brew install xcodegen
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

# 构建
chmod +x scripts/build_ios.sh
./scripts/build_ios.sh release
```

产物：项目根目录下的 `PhirLie-ios-release.ipa`

---

## 安装到设备

IPA 默认是未签名的，可以通过以下方式安装：

| 工具 | 说明 |
|------|------|
| **TrollStore** | 永久签名，无需 Apple ID，支持 iOS 14.0-16.6.1 |
| **Sideloadly** | 免费 Apple ID，7 天有效期，需每 7 天重签 |
| **AltStore** | 同上，免费 Apple ID 7 天 |
| **Xcode** | 有开发者账号可直接签名安装 |

### 代码签名（可选）

如果有 Apple Developer 账号：

1. 复制签名模板：
   ```bash
   cp PhirLie/xcode/LocalSigning.template.xcconfig PhirLie/xcode/LocalSigning.xcconfig
   ```
2. 编辑 `LocalSigning.xcconfig`，填入你的 Team ID
3. 重新构建，CI 会自动使用你的签名配置

---

## 工程结构

```
PhirLie/xcode/
├── project.yml              # xcodegen 工程描述（生成 .xcodeproj）
├── Shared.xcconfig          # 共享构建配置
├── LocalSigning.template.xcconfig  # 签名配置模板
├── Assets.xcassets/         # App 图标等资源
├── LaunchScreen.storyboard  # 启动页
└── PhirLie/                 # iOS 应用源码
    ├── AppDelegate.swift    # 应用入口
    └── ViewController.swift # 调用 Rust quad_main()
```

**构建流程：**
1. `xcodegen` 根据 `project.yml` 生成 `PhirLie.xcodeproj`
2. Xcode 预构建脚本调用 `cargo build --target aarch64-apple-ios` 编译 Rust 静态库（`libPhirLie.a`）
3. Xcode 编译 Swift 代码，链接 Rust 静态库和系统框架
4. 打包 `.app` → 压缩为 `.ipa`

---

## 常见问题

**Q: 为什么不能直接在 Windows 上编译 IPA？**
A: iOS SDK、Xcode 工具链、代码签名都只在 macOS 上可用。Rust 虽然支持交叉编译目标，但链接阶段需要 iOS SDK，这是 Windows 无法提供的。

**Q: 构建失败怎么办？**
A: 查看 CI 的 build log artifact，或本地运行 `scripts/build_ios.sh` 查看详细错误。常见原因：依赖缺失、Rust 编译错误、Xcode 版本不兼容。

**Q: 支持模拟器吗？**
A: 支持。project.yml 中配置了 `iphonesimulator` 平台，会同时编译 arm64 和 x86_64 模拟器架构并用 lipo 合并。
