//! 本地谱面分享：打包 / 解压 / 上传 / 下载

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::dir;

/// 把本地谱面 `local_path`（相对 `dir::charts()` 的子路径）复制到 `download/{uuid}`
pub fn stage_local_chart(local_path: &str, uuid: &str) -> Result<()> {
    let src = format!("{}/{}", dir::charts()?, local_path);
    let src_path = std::path::Path::new(&src);
    if !src_path.is_dir() {
        anyhow::bail!("local chart directory not found: {}", src_path.display());
    }
    let dst = format!("{}/download/{uuid}", dir::charts()?);
    let dst_path = std::path::Path::new(&dst);
    if dst_path.exists() {
        if dst_path.is_file() {
            std::fs::remove_file(dst_path)?;
        } else {
            std::fs::remove_dir_all(dst_path)?;
        }
    }
    copy_dir(src_path, dst_path)?;
    Ok(())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src).with_context(|| format!("read dir {}", src.display()))? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// 把 `charts/download/{chart_id}` 目录打包成 zip（内存中）
pub fn pack_chart_dir(chart_id: &str) -> Result<Vec<u8>> {
    let root = format!("{}/download/{chart_id}", dir::charts()?);
    let root = std::path::Path::new(&root);
    if !root.is_dir() {
        anyhow::bail!("local chart directory not found: {}", root.display());
    }

    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let options =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        fn visit(
            zip: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
            options: zip::write::SimpleFileOptions,
            base: &std::path::Path,
            path: &std::path::Path,
        ) -> Result<()> {
            for entry in std::fs::read_dir(path).with_context(|| format!("read dir {}", path.display()))? {
                let entry = entry?;
                let p = entry.path();
                let rel = p.strip_prefix(base)?;
                if p.is_dir() {
                    let name = format!("{}/", rel.to_string_lossy());
                    zip.add_directory(name, options)?;
                    visit(zip, options, base, &p)?;
                } else {
                    zip.start_file(rel.to_string_lossy().to_string(), options)?;
                    let mut f = std::fs::File::open(&p)?;
                    std::io::copy(&mut f, zip)?;
                }
            }
            Ok(())
        }

        visit(&mut zip, options, root, root)?;
        zip.finish()?;
    }
    Ok(out)
}

/// 玩家从房主下载谱面时的共享状态
pub struct ChartSyncing {
    pub done: AtomicBool,
    pub error: Mutex<Option<String>>,
    pub started: AtomicBool,
}

impl ChartSyncing {
    pub fn new() -> Self {
        Self {
            done: AtomicBool::new(false),
            error: Mutex::new(None),
            started: AtomicBool::new(false),
        }
    }

    pub fn mark_started(&self) {
        self.started.store(true, Ordering::SeqCst);
    }

    pub fn mark_done(&self) {
        self.done.store(true, Ordering::SeqCst);
    }

    pub fn set_error(&self, e: impl Into<String>) {
        *self.error.lock().unwrap() = Some(e.into());
        self.done.store(true, Ordering::SeqCst);
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
}

/// 房主把本地谱面包上传到服务端（经 game 连接）
pub async fn upload_chart(client: &phira_mp_client::Client, chart_id: &str) -> Result<()> {
    let zip = pack_chart_dir(chart_id)?;
    client.upload_chart(chart_id.to_string(), zip).await?;
    Ok(())
}

/// 玩家从服务端下载谱面包，解压到 `download/{chart_id}`
pub async fn download_chart(
    client: &phira_mp_client::Client,
    chart_id: &str,
    syncing: Arc<ChartSyncing>,
) -> Result<()> {
    syncing.mark_started();
    let bytes = client.download_chart(chart_id.to_string()).await?;

    let tmp = format!("{}/download/sync_{chart_id}", dir::charts()?);
    let tmp_path = std::path::Path::new(&tmp);
    if tmp_path.exists() {
        if tmp_path.is_file() {
            std::fs::remove_file(tmp_path)?;
        } else {
            std::fs::remove_dir_all(tmp_path)?;
        }
    }
    std::fs::create_dir_all(tmp_path)?;
    {
        let chart_dir = prpr::dir::Dir::new(tmp_path)?;
        prpr::ext::unzip_into(std::io::Cursor::new(bytes), &chart_dir, false)?;
    }

    let to = format!("{}/download/{chart_id}", dir::charts()?);
    let to_path = std::path::Path::new(&to);
    if to_path.exists() {
        if to_path.is_file() {
            std::fs::remove_file(to_path)?;
        } else {
            std::fs::remove_dir_all(to_path)?;
        }
    }
    if let Some(parent) = to_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(tmp_path, to_path)?;

    syncing.mark_done();
    Ok(())
}
