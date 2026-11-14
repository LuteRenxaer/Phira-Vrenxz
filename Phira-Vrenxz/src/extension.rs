
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::lanzou::{self, LanzouFile};

pub const EXTENSION_DIR_NAME: &str = "extensions";

pub const EXTENSION_CACHE_DIR: &str = "cache";

pub const EXTENSION_EXTRACTED_DIR: &str = "extracted";

pub const MANIFEST_FILE: &str = "manifest.json";

pub const STATE_FILE: &str = "extensions_state.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionType {

    Levels,
}

impl Default for ExtensionType {
    fn default() -> Self {
        ExtensionType::Levels
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {

    pub id: String,

    pub name: String,

    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub author: String,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub ext_type: ExtensionType,

    #[serde(default)]
    pub source_file: String,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionState {

    pub id: String,

    pub enabled: bool,

    pub installed_version: Option<String>,

    pub install_path: Option<String>,

    pub download_time: Option<i64>,

    pub file_size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExtensionInfo {

    pub manifest: ExtensionManifest,

    pub state: ExtensionState,

    pub is_local: bool,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {

    pub downloaded: u64,

    pub total: Option<u64>,

    pub phase: DownloadPhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadPhase {

    Listing,

    ResolvingUrl,

    Downloading,

    Extracting,

    Done,

    Failed(String),
}

pub struct ExtensionManager {

    root_dir: PathBuf,

    states: HashMap<String, ExtensionState>,

    download_tasks: HashMap<String, Arc<Mutex<DownloadProgress>>>,
}

impl ExtensionManager {

    pub fn new(data_dir: &Path) -> Result<Self> {
        let root_dir = data_dir.join(EXTENSION_DIR_NAME);
        std::fs::create_dir_all(&root_dir).context("创建扩展包目录失败")?;
        std::fs::create_dir_all(root_dir.join(EXTENSION_CACHE_DIR)).ok();
        std::fs::create_dir_all(root_dir.join(EXTENSION_EXTRACTED_DIR)).ok();

        let mut mgr = Self {
            root_dir,
            states: HashMap::new(),
            download_tasks: HashMap::new(),
        };
        mgr.load_state()?;
        mgr.scan_local_extensions()?;
        Ok(mgr)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn extracted_dir(&self) -> PathBuf {
        self.root_dir.join(EXTENSION_EXTRACTED_DIR)
    }

    fn state_file_path(&self) -> PathBuf {
        self.root_dir.join(STATE_FILE)
    }

    fn load_state(&mut self) -> Result<()> {
        let path = self.state_file_path();
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(&path).context("读取扩展包状态文件失败")?;
        let states: Vec<ExtensionState> =
            serde_json::from_str(&content).unwrap_or_default();
        for s in states {
            self.states.insert(s.id.clone(), s);
        }
        Ok(())
    }

    fn save_state(&self) -> Result<()> {
        let states: Vec<&ExtensionState> = self.states.values().collect();
        let content = serde_json::to_string_pretty(&states).context("序列化扩展包状态失败")?;
        std::fs::write(self.state_file_path(), content).context("写入扩展包状态文件失败")?;
        Ok(())
    }

    fn scan_local_extensions(&mut self) -> Result<()> {
        let extracted = self.extracted_dir();
        if !extracted.exists() {
            return Ok(());
        }

        if let Ok(entries) = std::fs::read_dir(&extracted) {
            for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join(MANIFEST_FILE);
            if manifest_path.exists() {

                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = serde_json::from_str::<ExtensionManifest>(&content) {
                        let id = manifest.id.clone();
                        if !self.states.contains_key(&id) {
                            self.states.insert(
                                id.clone(),
                                ExtensionState {
                                    id: id.clone(),
                                    enabled: true,
                                    installed_version: Some(manifest.version.clone()),
                                    install_path: Some(path.to_string_lossy().to_string()),
                                    download_time: None,
                                    file_size: None,
                                },
                            );
                        }
                    }
                }
            } else {

                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                let id = format!("local_{}", dir_name);
                if !self.states.contains_key(&id) {

                    let has_level = path.join("Level").exists()
                        || WalkDir::new(&path)
                            .into_iter()
                            .filter_map(|e| e.ok())
                            .any(|e| e.file_name().to_string_lossy() == "Level.json");
                    if !has_level {
                        continue;
                    }

                    let manifest = ExtensionManifest {
                        id: id.clone(),
                        name: dir_name,
                        version: "1.0.0".to_string(),
                        author: "本地".to_string(),
                        description: "本地安装的扩展包".to_string(),
                        ext_type: ExtensionType::Levels,
                        source_file: String::new(),
                    };

                    let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
                    std::fs::write(path.join(MANIFEST_FILE), manifest_json).ok();

                    self.states.insert(
                        id.clone(),
                        ExtensionState {
                            id: id.clone(),
                            enabled: true,
                            installed_version: Some("1.0.0".to_string()),
                            install_path: Some(path.to_string_lossy().to_string()),
                            download_time: None,
                            file_size: None,
                        },
                    );
                }
            }
            }
        }
        self.save_state()?;
        Ok(())
    }

    pub fn list_installed(&self) -> Vec<ExtensionInfo> {
        let mut result = Vec::new();
        for state in self.states.values() {
            if let Some(path) = &state.install_path {
                let manifest_path = Path::new(path).join(MANIFEST_FILE);
                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = serde_json::from_str::<ExtensionManifest>(&content) {
                        result.push(ExtensionInfo {
                            manifest,
                            state: state.clone(),
                            is_local: state.download_time.is_none(),
                        });
                    }
                }
            }
        }
        result.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        result
    }

    pub fn enabled_paths(&self) -> Vec<PathBuf> {
        self.states
            .values()
            .filter(|s| s.enabled)
            .filter_map(|s| s.install_path.as_ref().map(PathBuf::from))
            .collect()
    }

    pub fn get_info(&self, id: &str) -> Option<ExtensionInfo> {
        let state = self.states.get(id)?;
        let path = state.install_path.as_ref()?;
        let manifest_path = Path::new(path).join(MANIFEST_FILE);
        let content = std::fs::read_to_string(&manifest_path).ok()?;
        let manifest = serde_json::from_str::<ExtensionManifest>(&content).ok()?;
        Some(ExtensionInfo {
            manifest,
            state: state.clone(),
            is_local: state.download_time.is_none(),
        })
    }

    pub fn enable(&mut self, id: &str) -> Result<()> {
        if let Some(state) = self.states.get_mut(id) {
            state.enabled = true;
            self.save_state()?;
            info!("启用扩展包: {}", id);
        }
        Ok(())
    }

    pub fn disable(&mut self, id: &str) -> Result<()> {
        if let Some(state) = self.states.get_mut(id) {
            state.enabled = false;
            self.save_state()?;
            info!("禁用扩展包: {}", id);
        }
        Ok(())
    }

    pub fn toggle(&mut self, id: &str) -> Result<bool> {
        let enabled = if let Some(state) = self.states.get_mut(id) {
            state.enabled = !state.enabled;
            state.enabled
        } else {
            false
        };
        self.save_state()?;
        Ok(enabled)
    }

    pub fn uninstall(&mut self, id: &str) -> Result<()> {
        if let Some(state) = self.states.remove(id) {
            if let Some(path) = state.install_path {
                if Path::new(&path).exists() {
                    std::fs::remove_dir_all(&path).ok();
                }
            }

            let cache_file = self.root_dir.join(EXTENSION_CACHE_DIR).join(format!("{}.zip", id));
            if cache_file.exists() {
                std::fs::remove_file(cache_file).ok();
            }
            self.save_state()?;
            info!("卸载扩展包: {}", id);
        }
        Ok(())
    }

    pub async fn download_from_lanzou(
        &mut self,
        folder_url: &str,
        folder_password: Option<&str>,
        file_password: Option<&str>,
        name_filter: Option<&str>,
        task_id: Option<String>,
    ) -> Result<Vec<String>> {

        let progress = Arc::new(Mutex::new(DownloadProgress {
            downloaded: 0,
            total: None,
            phase: DownloadPhase::Listing,
        }));
        let task_id = task_id.unwrap_or_else(|| format!("task_{}", uuid::Uuid::new_v4().simple()));
        self.download_tasks.insert(task_id, progress.clone());

        let result = self
            .do_download_all(folder_url, folder_password, file_password, name_filter, &progress)
            .await;

        {
            let mut p = progress.lock().await;
            match &result {
                Ok(_) => p.phase = DownloadPhase::Done,
                Err(e) => p.phase = DownloadPhase::Failed(e.to_string()),
            }
        }

        result
    }

    async fn do_download_all(
        &mut self,
        folder_url: &str,
        folder_password: Option<&str>,
        file_password: Option<&str>,
        name_filter: Option<&str>,
        progress: &Arc<Mutex<DownloadProgress>>,
    ) -> Result<Vec<String>> {

        {
            let mut p = progress.lock().await;
            p.phase = DownloadPhase::Listing;
        }
        let folder = match lanzou::list_folder(folder_url, folder_password).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("获取蓝奏云文件列表失败: {:?}", e);
                return Err(anyhow!("获取蓝奏云文件列表失败: {}", e));
            }
        };

        let zip_files: Vec<_> = folder
            .files
            .iter()
            .filter(|f| {
                let lower = f.name.to_lowercase();
                (lower.ends_with(".zip") || lower.ends_with(".7z"))
                    && name_filter.as_ref().map_or(true, |filter| {
                        lower.contains(&filter.to_lowercase())
                    })
            })
            .collect();

        if zip_files.is_empty() {
            anyhow::bail!("文件夹中没有找到压缩文件（.zip/.7z）");
        }

        info!("找到 {} 个压缩文件", zip_files.len());

        let mut installed_ids = Vec::new();
        let cache_dir = self.root_dir.join(EXTENSION_CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).ok();

        for (idx, target) in zip_files.iter().enumerate() {
            info!("下载文件 {}/{}: {}", idx + 1, zip_files.len(), target.name);

            {
                let mut p = progress.lock().await;
                p.phase = DownloadPhase::ResolvingUrl;
                p.downloaded = 0;
                p.total = None;
            }
            let direct_url = lanzou::get_direct_url(&target.url, file_password)
                .await
                .context(format!("获取直链失败: {}", target.name))?;

            let id = format!("ext_{}", uuid::Uuid::new_v4().simple());
            let ext = if target.name.to_lowercase().ends_with(".7z") {
                "7z"
            } else {
                "zip"
            };
            let cache_file = cache_dir.join(format!("{}.{}", id, ext));

            {
                let mut p = progress.lock().await;
                p.phase = DownloadPhase::Downloading;
            }
            let progress_owned = (*progress).clone();
            lanzou::download_file(&direct_url, &cache_file, move |downloaded, total| {
                if let Ok(mut p) = progress_owned.try_lock() {
                    p.downloaded = downloaded;
                    p.total = total;
                }
            })
            .await
            .context(format!("下载文件失败: {}", target.name))?;

            let file_size = std::fs::metadata(&cache_file).map(|m| m.len()).ok();

            {
                let mut p = progress.lock().await;
                p.phase = DownloadPhase::Extracting;
            }
            let install_path = self.install_extension(&id, &cache_file, &target.name)?;

            let now = chrono::Utc::now().timestamp();
            self.states.insert(
                id.clone(),
                ExtensionState {
                    id: id.clone(),
                    enabled: true,
                    installed_version: Some("1.0.0".to_string()),
                    install_path: Some(install_path.to_string_lossy().to_string()),
                    download_time: Some(now),
                    file_size,
                },
            );
            self.save_state()?;

            info!("扩展包安装完成: {} ({}) -> {:?}", id, target.name, install_path);
            installed_ids.push(id);

            let _ = std::fs::remove_file(&cache_file);
        }

        Ok(installed_ids)
    }

    fn install_extension(&self, id: &str, cache_file: &Path, original_name: &str) -> Result<PathBuf> {
        let extracted_dir = self.extracted_dir();
        let install_path = extracted_dir.join(id);
        std::fs::create_dir_all(&install_path).context("创建扩展包目录失败")?;

        let lower_name = original_name.to_lowercase();
        if lower_name.ends_with(".zip") {

            self.extract_zip(cache_file, &install_path)?;
        } else if lower_name.ends_with(".7z") {

            self.extract_7z(cache_file, &install_path)?;
        } else {

            let dest = install_path.join(original_name);
            std::fs::copy(cache_file, &dest).context("复制文件失败")?;
        }

        let manifest_path = install_path.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            let name = original_name
                .rsplit('/')
                .next()
                .unwrap_or(original_name)
                .trim_end_matches(".zip")
                .trim_end_matches(".7z")
                .trim_end_matches(".ZIP")
                .trim_end_matches(".7Z")
                .to_string();

            let manifest = ExtensionManifest {
                id: id.to_string(),
                name,
                version: "1.0.0".to_string(),
                author: "蓝奏云".to_string(),
                description: "从蓝奏云下载的扩展包".to_string(),
                ext_type: ExtensionType::Levels,
                source_file: original_name.to_string(),
            };

            let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
            std::fs::write(&manifest_path, manifest_json).context("写入 manifest.json 失败")?;
        }

        Ok(install_path)
    }

    fn extract_zip(&self, cache_file: &Path, install_path: &Path) -> Result<()> {
        let file = std::fs::File::open(cache_file).context("打开 zip 文件失败")?;
        let mut archive = ZipArchive::new(file).context("读取 zip 归档失败")?;

        let has_single_root = {
            let mut roots = std::collections::HashSet::new();
            for i in 0..archive.len() {
                if let Ok(entry) = archive.by_index(i) {
                    if let Some(name) = entry.name().split('/').next() {
                        if !name.is_empty() {
                            roots.insert(name.to_string());
                        }
                    }
                }
            }
            roots.len() == 1
        };

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).context("读取 zip 条目失败")?;
            let entry_name = entry.name().to_string();

            let relative_path = if has_single_root {
                entry_name.splitn(2, '/').nth(1).unwrap_or(&entry_name).to_string()
            } else {
                entry_name.clone()
            };

            if relative_path.is_empty() {
                continue;
            }

            let out_path = install_path.join(&relative_path);

            if entry.is_dir() {
                std::fs::create_dir_all(&out_path).ok();
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let mut out_file = std::fs::File::create(&out_path).context("创建文件失败")?;
                std::io::copy(&mut entry, &mut out_file).context("写入文件失败")?;
            }
        }
        Ok(())
    }

    fn extract_7z(&self, cache_file: &Path, install_path: &Path) -> Result<()> {

        let temp_dir = install_path.parent().unwrap().join(format!("_temp_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).ok();

        sevenz_rust::decompress_file(cache_file, &temp_dir)
            .context("解压 7z 文件失败")?;

        let entries: Vec<_> = std::fs::read_dir(&temp_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .collect();

        if entries.len() == 1 && entries[0].path().is_dir() {

            let root_dir = &entries[0].path();
            for entry in std::fs::read_dir(root_dir).into_iter().flatten().flatten() {
                let dest = install_path.join(entry.file_name());
                if entry.path().is_dir() {
                    let _ = std::fs::remove_dir_all(&dest);
                    std::fs::rename(entry.path(), &dest).ok();
                } else {
                    let _ = std::fs::remove_file(&dest);
                    std::fs::rename(entry.path(), &dest).ok();
                }
            }
            let _ = std::fs::remove_dir_all(root_dir);
        } else {

            for entry in entries {
                let dest = install_path.join(entry.file_name());
                if entry.path().is_dir() {
                    let _ = std::fs::remove_dir_all(&dest);
                    std::fs::rename(entry.path(), &dest).ok();
                } else {
                    let _ = std::fs::remove_file(&dest);
                    std::fs::rename(entry.path(), &dest).ok();
                }
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    pub fn get_download_progress(&self, id: &str) -> Option<Arc<Mutex<DownloadProgress>>> {
        self.download_tasks.get(id).cloned()
    }

    pub fn cleanup_download_tasks(&mut self) {
        self.download_tasks.retain(|_, v| {
            if let Ok(p) = v.try_lock() {
                !matches!(p.phase, DownloadPhase::Done | DownloadPhase::Failed(_))
            } else {
                true
            }
        });
    }
}

pub fn get_extension_level_dirs(mgr: &ExtensionManager) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for path in mgr.enabled_paths() {
        let level_dir = path.join("Level");
        if level_dir.exists() {
            dirs.push(level_dir);
        }

        if path.join("Level.json").exists() {
            dirs.push(path.clone());
        }
    }
    dirs
}
