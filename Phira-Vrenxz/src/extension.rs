//! 扩展包管理系统
//!
//! 支持从蓝奏云下载关卡扩展包，本地缓存、解压、启用/禁用管理。
//! 扩展包内容会合并到资源搜索路径中，对游戏透明。

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

// ========== 常量 ==========

/// 扩展包根目录（相对于用户数据目录）
pub const EXTENSION_DIR_NAME: &str = "extensions";

/// 扩展包缓存目录
pub const EXTENSION_CACHE_DIR: &str = "cache";

/// 扩展包已解压目录
pub const EXTENSION_EXTRACTED_DIR: &str = "extracted";

/// 扩展包元数据文件名
pub const MANIFEST_FILE: &str = "manifest.json";

/// 扩展包状态文件
pub const STATE_FILE: &str = "extensions_state.json";

// ========== 数据结构 ==========

/// 扩展包类型
/// 注：仅保留关卡类型用于新装的扩展包；`Models`/`Both` 仅为兼容旧数据保留。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionType {
    /// 仅包含关卡
    Levels,
    /// 仅包含 Spine 模型（旧数据兼容，不再产生）
    Models,
    /// 同时包含模型和关卡（旧数据兼容，不再产生）
    Both,
}

impl Default for ExtensionType {
    fn default() -> Self {
        ExtensionType::Both
    }
}

/// 扩展包元数据（manifest.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    /// 扩展包唯一 ID
    pub id: String,
    /// 显示名称
    pub name: String,
    /// 版本号
    #[serde(default = "default_version")]
    pub version: String,
    /// 作者
    #[serde(default)]
    pub author: String,
    /// 描述
    #[serde(default)]
    pub description: String,
    /// 扩展包类型
    #[serde(default)]
    pub ext_type: ExtensionType,
    /// 蓝奏云文件名（用于更新检测）
    #[serde(default)]
    pub source_file: String,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// 扩展包运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionState {
    /// 扩展包 ID
    pub id: String,
    /// 是否已启用
    pub enabled: bool,
    /// 已安装版本
    pub installed_version: Option<String>,
    /// 本地文件路径（解压后的目录）
    pub install_path: Option<String>,
    /// 下载时间（时间戳）
    pub download_time: Option<i64>,
    /// 文件大小（字节）
    pub file_size: Option<u64>,
}

/// 扩展包完整信息
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    /// 元数据
    pub manifest: ExtensionManifest,
    /// 运行时状态
    pub state: ExtensionState,
    /// 是否为本地安装（非蓝奏云下载）
    pub is_local: bool,
}

/// 下载进度
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// 已下载字节
    pub downloaded: u64,
    /// 总字节（可能未知）
    pub total: Option<u64>,
    /// 当前阶段
    pub phase: DownloadPhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadPhase {
    /// 解析文件列表
    Listing,
    /// 获取直链
    ResolvingUrl,
    /// 下载中
    Downloading,
    /// 解压中
    Extracting,
    /// 完成
    Done,
    /// 失败
    Failed(String),
}

// ========== 全局状态 ==========

/// 扩展包管理器
pub struct ExtensionManager {
    /// 扩展包根目录
    root_dir: PathBuf,
    /// 已安装扩展包状态
    states: HashMap<String, ExtensionState>,
    /// 下载任务状态
    download_tasks: HashMap<String, Arc<Mutex<DownloadProgress>>>,
}

impl ExtensionManager {
    /// 创建扩展包管理器
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

    /// 获取扩展包根目录
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// 获取已解压目录
    pub fn extracted_dir(&self) -> PathBuf {
        self.root_dir.join(EXTENSION_EXTRACTED_DIR)
    }

    // ----- 状态持久化 -----

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

    // ----- 扫描本地扩展包 -----

    /// 扫描已解压目录中的所有扩展包
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
                // 标准扩展包
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
                // 非标准扩展包（直接放的 Level 目录），自动生成 manifest
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                let id = format!("local_{}", dir_name);
                if !self.states.contains_key(&id) {
                    // 检测内容类型（仅关卡内容有效）
                    let has_level = path.join("Level").exists()
                        || WalkDir::new(&path)
                            .into_iter()
                            .filter_map(|e| e.ok())
                            .any(|e| e.file_name().to_string_lossy() == "Level.json");
                    if !has_level {
                        continue; // 不含关卡内容，忽略（角色模型扩展已移除）
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

                    // 写入 manifest
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

    // ----- 查询 -----

    /// 获取所有已安装扩展包
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

    /// 获取所有已启用扩展包的安装路径
    pub fn enabled_paths(&self) -> Vec<PathBuf> {
        self.states
            .values()
            .filter(|s| s.enabled)
            .filter_map(|s| s.install_path.as_ref().map(PathBuf::from))
            .collect()
    }

    /// 获取扩展包信息
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

    // ----- 启用/禁用 -----

    /// 启用扩展包
    pub fn enable(&mut self, id: &str) -> Result<()> {
        if let Some(state) = self.states.get_mut(id) {
            state.enabled = true;
            self.save_state()?;
            info!("启用扩展包: {}", id);
        }
        Ok(())
    }

    /// 禁用扩展包
    pub fn disable(&mut self, id: &str) -> Result<()> {
        if let Some(state) = self.states.get_mut(id) {
            state.enabled = false;
            self.save_state()?;
            info!("禁用扩展包: {}", id);
        }
        Ok(())
    }

    /// 切换启用状态
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

    // ----- 卸载 -----

    /// 卸载扩展包
    pub fn uninstall(&mut self, id: &str) -> Result<()> {
        if let Some(state) = self.states.remove(id) {
            if let Some(path) = state.install_path {
                if Path::new(&path).exists() {
                    std::fs::remove_dir_all(&path).ok();
                }
            }
            // 同时删除缓存文件
            let cache_file = self.root_dir.join(EXTENSION_CACHE_DIR).join(format!("{}.zip", id));
            if cache_file.exists() {
                std::fs::remove_file(cache_file).ok();
            }
            self.save_state()?;
            info!("卸载扩展包: {}", id);
        }
        Ok(())
    }

    // ----- 下载 -----

    /// 从蓝奏云下载并安装扩展包
    /// 从蓝奏云文件夹下载扩展包（zip/7z 文件）
    ///
    /// # Arguments
    /// * `folder_url` - 蓝奏云文件夹链接
    /// * `folder_password` - 文件夹密码
    /// * `file_password` - 文件密码（可选）
    /// * `name_filter` - 文件名过滤（可选，只下载包含该字符串的文件）
    /// * `task_id` - 下载任务 ID（可选，用于外部进度跟踪）
    pub async fn download_from_lanzou(
        &mut self,
        folder_url: &str,
        folder_password: Option<&str>,
        file_password: Option<&str>,
        name_filter: Option<&str>,
        task_id: Option<String>,
    ) -> Result<Vec<String>> {
        // 创建下载进度跟踪
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

        // 更新最终状态
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
        // 1. 列出文件夹文件
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

        // 2. 过滤出压缩文件（zip/7z），可选按名称过滤
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

            // 3. 获取直链
            {
                let mut p = progress.lock().await;
                p.phase = DownloadPhase::ResolvingUrl;
                p.downloaded = 0;
                p.total = None;
            }
            let direct_url = lanzou::get_direct_url(&target.url, file_password)
                .await
                .context(format!("获取直链失败: {}", target.name))?;

            // 4. 下载文件
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

            // 5. 解压/安装
            {
                let mut p = progress.lock().await;
                p.phase = DownloadPhase::Extracting;
            }
            let install_path = self.install_extension(&id, &cache_file, &target.name)?;

            // 6. 更新状态
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

            // 清理缓存文件
            let _ = std::fs::remove_file(&cache_file);
        }

        Ok(installed_ids)
    }

    /// 安装扩展包（解压 zip/7z 或直接复制文件）
    fn install_extension(&self, id: &str, cache_file: &Path, original_name: &str) -> Result<PathBuf> {
        let extracted_dir = self.extracted_dir();
        let install_path = extracted_dir.join(id);
        std::fs::create_dir_all(&install_path).context("创建扩展包目录失败")?;

        let lower_name = original_name.to_lowercase();
        if lower_name.ends_with(".zip") {
            // 解压 zip
            self.extract_zip(cache_file, &install_path)?;
        } else if lower_name.ends_with(".7z") {
            // 解压 7z
            self.extract_7z(cache_file, &install_path)?;
        } else {
            // 非压缩文件，直接复制
            let dest = install_path.join(original_name);
            std::fs::copy(cache_file, &dest).context("复制文件失败")?;
        }

        // 如果没有 manifest.json，自动生成（扩展包一律按关卡包处理）
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

    /// 解压 zip 文件到目标目录（自动去掉单层根目录）
    fn extract_zip(&self, cache_file: &Path, install_path: &Path) -> Result<()> {
        let file = std::fs::File::open(cache_file).context("打开 zip 文件失败")?;
        let mut archive = ZipArchive::new(file).context("读取 zip 归档失败")?;

        // 检测是否有单层根目录
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

    /// 解压 7z 文件到目标目录
    fn extract_7z(&self, cache_file: &Path, install_path: &Path) -> Result<()> {
        // 先解压到临时目录
        let temp_dir = install_path.parent().unwrap().join(format!("_temp_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).ok();

        sevenz_rust::decompress_file(cache_file, &temp_dir)
            .context("解压 7z 文件失败")?;

        // 检测是否有单层根目录
        let entries: Vec<_> = std::fs::read_dir(&temp_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .collect();

        if entries.len() == 1 && entries[0].path().is_dir() {
            // 单层根目录，把内容移到 install_path
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
            // 直接把所有内容移到 install_path
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

    // ----- 下载进度查询 -----

    /// 获取下载进度
    pub fn get_download_progress(&self, id: &str) -> Option<Arc<Mutex<DownloadProgress>>> {
        self.download_tasks.get(id).cloned()
    }

    /// 清理已完成的下载任务记录
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

// ========== 资源路径合并 ==========

/// 获取所有已启用扩展包中的 Level 目录路径
pub fn get_extension_level_dirs(mgr: &ExtensionManager) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for path in mgr.enabled_paths() {
        let level_dir = path.join("Level");
        if level_dir.exists() {
            dirs.push(level_dir);
        }
        // 检查直接放在根目录的 Level.json
        if path.join("Level.json").exists() {
            dirs.push(path.clone());
        }
    }
    dirs
}
