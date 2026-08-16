# 🔥 phirLie

**phirLie** 是一款社区驱动的节奏游戏，是 [Phira](https://github.com/TeamFlos/phira) 的分支，玩法受 Phigros启发。

基于 Rust 开发，继承 Phira 核心功能并新增以下特性：

- **全屏判定** – 可开关，触摸判定不再受区域限制
- **自定义 Combo 文本** – 自由修改连击显示文字
- **中文数字** – 支持用中文数字显示分数
- **自定义 Autoplay 标签** – 修改自动演示模式文本
- **自定义水印** – 在游戏画面添加自定义文字
- **全新 UI** – 现代卡片式视觉风格

phirLie 是 Phira 的分支，玩法受 Phigros（Pigeon Games）启发。感谢两个项目的灵感与贡献。

---

## 📥 下载

> 即将发布。预编译版本将在 [Releases](https://github.com/LuteRenxaer/phirLie/releases) 提供。

---

## 🛠 从源码构建

### 环境要求
- Rust（推荐 nightly 版本）
- Android SDK 和 NDK（如需构建 Android 版本）

### 构建命令
```bash
# 桌面端（Windows/Linux）
cargo run -p PhirLie-main

# Android APK
cargo apk build -p PhirLie