<<<<<<< HEAD
##PhireRe

**PhireRe** is a community-driven rhythm game, forked from Phira and inspired by Phigros.

Built with Rust, it inherits Phira's core while adding new features:

- **Full Screen Judge** – toggle judgment across the entire screen
- **Custom Combo Text** – personalize the combo display
- **Chinese Numerals** – display scores in Chinese characters
- **Custom Autoplay Label** – change the autoplay mode text
- **Custom Watermark** – add your own text overlay
- **Redesigned UI** – modern card-style visual improvements

PhireRe is a fork of Phira, inspired by Phigros (Pigeon Games). Special thanks to both projects.

---

## Download

> Coming soon. Pre-built binaries will be available in [Releases](https://github.com/LuteRenxaer/phire/releases).

---

## 🛠 Build from Source

### Prerequisites
- Rust (nightly recommended)
- Android SDK & NDK (for Android build)

### Build Commands
```bash
# Desktop (Windows/Linux)
cargo run -p phira-main

# Android APK
cargo apk build -p phirano
