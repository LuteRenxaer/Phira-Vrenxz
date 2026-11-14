//! 本体资源包（PVX1）加解密。
//!
//! 与 `scripts/encrypt_builtin_pack.py` 保持二进制兼容：
//! - 布局：magic(4B "PVX1") + version(1B) + salt(16B) + len(8B LE) + cipher(len B)
//! - 密钥流：K = SHA256(master || salt)，流 = SHA256(K || u64_LE(0)) ++ SHA256(K || u64_LE(1)) ++ ...
//! - 加密：cipher = plaintext XOR stream（不依赖资源内容格式，任何格式统一处理）

use anyhow::{bail, Result};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 4] = b"PVX1";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 4 + 1 + 16 + 8;

/// 主密钥以两段混淆常量存储，运行时异或恢复，避免源码/二进制里出现明文密钥。
fn master_key() -> [u8; 32] {
    const A: [u8; 32] = [
        23, 229, 50, 112, 226, 80, 192, 200, 2, 103, 100, 238, 123, 31, 226, 241, 52, 220, 36, 170, 86, 176, 85,
        191, 111, 250, 184, 175, 45, 54, 199, 147,
    ];
    const B: [u8; 32] = [
        71, 141, 91, 2, 131, 6, 178, 173, 108, 31, 30, 195, 57, 106, 139, 157, 64, 181, 74, 135, 6, 209, 54, 212,
        66, 172, 137, 130, 102, 115, 158, 178,
    ];
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = A[i] ^ B[i];
    }
    k
}

fn stream_block(key: &[u8; 32], block: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(key);
    h.update(block.to_le_bytes());
    h.finalize().into()
}

fn derive_key(salt: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(master_key());
    h.update(salt);
    h.finalize().into()
}

/// 解密 PVX1 包，返回明文（原始 zip 字节）。
pub fn decrypt_pack(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < HEADER_LEN {
        bail!("pack too small: {} bytes", data.len());
    }
    if &data[..4] != MAGIC {
        bail!("not a PVX1 pack (bad magic)");
    }
    if data[4] != VERSION {
        bail!("unsupported pack version {}", data[4]);
    }
    let n = u64::from_le_bytes(data[21..29].try_into().unwrap()) as usize;
    let cipher = &data[HEADER_LEN..];
    if cipher.len() != n {
        bail!("pack payload length mismatch: header {n}, actual {}", cipher.len());
    }
    let key = derive_key(&data[5..21]);
    let mut out = vec![0u8; n];
    for (i, chunk) in cipher.chunks(32).enumerate() {
        let sb = stream_block(&key, i as u64);
        let start = i * 32;
        for (j, b) in chunk.iter().enumerate() {
            out[start + j] = b ^ sb[j];
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encrypt_pack(data: &[u8], salt: &[u8; 16]) -> Vec<u8> {
        let key = derive_key(salt);
        let mut out = Vec::with_capacity(HEADER_LEN + data.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(salt);
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        for (i, chunk) in data.chunks(32).enumerate() {
            let sb = stream_block(&key, i as u64);
            for (j, b) in chunk.iter().enumerate() {
                out.push(b ^ sb[j]);
            }
        }
        out
    }

    #[test]
    fn roundtrip() {
        let salt = [7u8; 16];
        let plain = b"hello \xe4\xb8\xad\xe6\x96\x87 pack data \x00\x01\xff".to_vec();
        let pack = encrypt_pack(&plain, &salt);
        let back = decrypt_pack(&pack).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn rejects_bad_magic() {
        let salt = [1u8; 16];
        let mut pack = encrypt_pack(b"abc", &salt);
        pack[0] = b'X';
        assert!(decrypt_pack(&pack).is_err());
    }

    #[test]
    fn rejects_truncated() {
        assert!(decrypt_pack(b"PVX1").is_err());
        assert!(decrypt_pack(b"").is_err());
    }
}
