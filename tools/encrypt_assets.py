#!/usr/bin/env python3
"""对 assets 里的音频做简单 XOR 混淆，生成 .enc 文件。

用法: python tools/encrypt_assets.py
Rust 端用同样的 KEY 逐字节异或还原。
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets_src"   # 明文源文件（不打包进 APK）
DST = ROOT / "assets"       # 加密后输出到这里（会被 cargo-apk 打包进 APK）

# 与 Rust 端 XOR_KEY 保持一致
KEY = b"phirLie-firefly-2026"

# 需要加密的文件（相对 assets_src/），输出到 assets/ 同路径 + .enc
TARGETS = [
    "bgm/home.ogg",
    "bgm/gameerror.mp3",
]


def xor(data: bytes, key: bytes) -> bytes:
    klen = len(key)
    return bytes(b ^ key[i % klen] for i, b in enumerate(data))


def main() -> None:
    for rel in TARGETS:
        src = SRC / rel
        if not src.exists():
            print(f"[skip] {rel} not found in {SRC}")
            continue
        dst = DST / (rel + ".enc")
        dst.parent.mkdir(parents=True, exist_ok=True)
        raw = src.read_bytes()
        enc = xor(raw, KEY)
        dst.write_bytes(enc)
        print(f"[ok]   {rel} -> {dst.relative_to(ROOT)}  ({len(raw)} bytes)")


if __name__ == "__main__":
    main()
