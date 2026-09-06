plugins {
    alias(libs.plugins.android.application)
}

android {
    namespace = "com.teamflos.phirlie"
    compileSdk {
        version = release(37)
    }

    defaultConfig {
        applicationId = "com.teamflos.PhiraVrenxz"
        minSdk = 23
        targetSdk = 37
        versionCode = 93
        versionName = "0.9.3-CBT2"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // 打包 arm64-v8a（真机）和 x86_64（模拟器）两个 ABI 的原生库。
        // 注意：已在 AndroidManifest 禁用 hardwareAccelerated，避免 MuMu 模拟器 HWUI 崩溃。
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            optimization {
                enable = false
            }
        }
    }

    // 让 .so 从 APK 中解压加载（等价于旧版 manifest 的 extractNativeLibs="true"），
    // 兼容 Android 6.0 并支持随时替换 jniLibs 里的 .so。
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
        // 排除 baseline profile（assets/dexopt/baseline.prof），
        // 避免往 APK 写入内容时与该 entry 冲突（"cannot overwrite"）
        resources {
            excludes += "/assets/dexopt/baseline.prof"
            excludes += "/assets/dexopt/baseline.profm"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

dependencies {
    // 移除 appcompat / material：外壳使用纯 Android Activity + android.app.AlertDialog，
    // 不依赖 AndroidX；这两个库自带的 baseline profile（assets/dexopt/baseline.prof）
    // 会导致打包时 entry 冲突（"cannot overwrite"）。
    // rustls-platform-verifier 的 Android 证书校验 Java 类（org.rustls.platformverifier.*）
    implementation(files("libs/rustls-platform-verifier-android.jar"))
    testImplementation(libs.junit)
    androidTestImplementation(libs.espresso.core)
    androidTestImplementation(libs.ext.junit)
}