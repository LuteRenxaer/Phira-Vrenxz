# Phira-Vrenxz

**Phira-Vrenxz** is a community-driven rhythm game, forked from [Phira](https://github.com/teamflos/phira) and inspired by Phigros (Pigeon Games).

Built with Rust, it inherits Phira's core while adding new features:

- **Full Screen Judge** – toggle judgment across the entire screen
- **Custom Combo Text** – personalize the combo display
- **Chinese Numerals** – display scores in Chinese characters
- **Custom Autoplay Label** – change the autoplay mode text
- **Custom Watermark** – add your own text overlay
- **Redesigned UI** – modern card-style visual improvements

---

## Download

> Pre-built Android APKs are provided in this project's releases and dev builds.

---

## 🛠 Build from Source

### Prerequisites
- Rust (nightly recommended)
- Android SDK & NDK (for Android build)

### Build Commands
```bash
# Desktop (Windows/Linux)
cargo run -p Phira-Vrenxz-main

# Android APK
cargo ndk -t arm64-v8a -o Android/app/src/main/jniLibs build --release -p Phira-Vrenxz
cd Android && ./gradlew :app:assembleDebug
```
