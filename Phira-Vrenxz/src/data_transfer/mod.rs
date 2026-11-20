//! 数据包（`.pfd`，ZIP 容器）导入导出与跨设备传输的公共逻辑（移植自 Phira-Firefly）。
//!
//! 包内结构：
//! - `export.json`：导出清单（`categories` / `exportedAt` / `version`）
//! - `data.json`：按分类筛选后的存档数据
//! - `charts/…`、`respack/…`、`fonts/…`：对应分类的文件
pub mod lan;

use crate::{data::Data, dir, get_data, get_data_mut, sync_data};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{Map, Value};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use walkdir::WalkDir;

/// 导出包内 data.json 的生成：只保留所选分类对应的字段。
pub fn build_data_json(selected: &[bool; CATEGORIES.len()]) -> Result<Vec<u8>> {
    let source = serde_json::to_value(get_data())?;
    let mut out = Map::new();
    for (i, category) in CATEGORIES.iter().enumerate() {
        if !selected[i] {
            continue;
        }
        for key in category.data_keys() {
            if let Some(value) = source.get(*key) {
                out.insert((*key).to_owned(), value.clone());
            }
        }
    }
    Ok(serde_json::to_vec_pretty(&Value::Object(out))?)
}

pub enum EntrySource {
    Memory(Vec<u8>),
    File(PathBuf),
}

pub struct Entry {
    pub name: String,
    pub source: EntrySource,
}

impl Entry {
    pub fn size(&self) -> u64 {
        match &self.source {
            EntrySource::Memory(data) => data.len() as u64,
            EntrySource::File(path) => std::fs::metadata(path).map(|it| it.len()).unwrap_or(0),
        }
    }
}

fn push_dir(entries: &mut Vec<Entry>, root: &Path, prefix: &str) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for item in WalkDir::new(root).sort_by_file_name() {
        let item = item?;
        if !item.file_type().is_file() {
            continue;
        }
        let rel = item.path().strip_prefix(root)?;
        entries.push(Entry {
            name: format!("{prefix}/{}", rel.to_string_lossy().replace('\\', "/")),
            source: EntrySource::File(item.path().to_owned()),
        });
    }
    Ok(())
}

/// 收集所选分类的导出条目（data.json / export.json / 谱面 / 资源包 / 字体）。
pub fn collect_entries(selected: &[bool; CATEGORIES.len()], conflict: Conflict) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();

    entries.push(Entry {
        name: "data.json".to_owned(),
        source: EntrySource::Memory(build_data_json(selected)?),
    });

    let categories = CATEGORIES
        .iter()
        .enumerate()
        .filter(|(i, _)| selected[*i])
        .map(|(_, category)| category.key())
        .collect::<Vec<_>>();
    let meta = serde_json::json!({
        "exportedAt": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
        "categories": categories,
        "conflict": conflict.key(),
    });
    entries.push(Entry {
        name: "export.json".to_owned(),
        source: EntrySource::Memory(serde_json::to_vec_pretty(&meta)?),
    });

    if selected[Category::Charts.index()] {
        let root = dir::charts()?;
        let mut seen = HashSet::new();
        for chart in &get_data().charts {
            let local_path = chart.local_path.trim_matches('/');
            if local_path.is_empty()
                || local_path.starts_with(':')
                || local_path.contains("..")
                || Path::new(local_path).is_absolute()
            {
                continue;
            }
            if !seen.insert(local_path.to_owned()) {
                continue;
            }
            let src = Path::new(&root).join(local_path);
            if !src.is_dir() {
                tracing::warn!("chart directory not found, skipping: {}", src.display());
                continue;
            }
            push_dir(&mut entries, &src, &format!("charts/{local_path}"))?;
        }
    }
    if selected[Category::ResPack.index()] {
        push_dir(&mut entries, Path::new(&dir::respacks()?), "respack")?;
    }
    if selected[Category::Settings.index()] {
        push_dir(&mut entries, Path::new(&dir::fonts()?), "fonts")?;
    }
    Ok(entries)
}

/// 把条目写入 ZIP（.pfd）文件，按字节更新进度。
pub fn write_zip(entries: Vec<Entry>, file: File, progress: &Progress) -> Result<()> {
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);

    let mut buf = vec![0u8; 64 * 1024];
    for entry in entries {
        match entry.source {
            EntrySource::Memory(data) => {
                zip.start_file(&entry.name, deflated)?;
                zip.write_all(&data)?;
                progress.add(data.len() as u64);
            }
            EntrySource::File(path) => {
                let mut src =
                    BufReader::new(File::open(&path).with_context(|| format!("failed to open {}", path.display()))?);
                let compressible = matches!(
                    path.extension().and_then(|it| it.to_str()).map(|it| it.to_ascii_lowercase()).as_deref(),
                    Some("json" | "yml" | "yaml" | "txt" | "pec" | "csv" | "lrc" | "ttml")
                );
                zip.start_file(&entry.name, if compressible { deflated } else { stored })?;
                loop {
                    let n = src.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    zip.write_all(&buf[..n])?;
                    progress.add(n as u64);
                }
            }
        }
    }
    zip.finish()?;
    progress.finish();
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Account,
    Settings,
    Charts,
    ResPack,
}

pub const CATEGORIES: [Category; 4] = [Category::Account, Category::Settings, Category::Charts, Category::ResPack];

impl Category {
    pub const fn index(self) -> usize {
        self as usize
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Settings => "settings",
            Self::Charts => "charts",
            Self::ResPack => "respack",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        CATEGORIES.iter().copied().find(|it| it.key() == key)
    }

    /// `data.json` 中归属该分类的字段。
    pub fn data_keys(self) -> &'static [&'static str] {
        match self {
            // 出于安全考虑不导出登录凭据（tokens）。
            Self::Account => &["me", "xcsim_account", "xcsim_api_url", "xcsim_download_url"],
            Self::Settings => &[
                "config",
                "language",
                "prefer_reduced_motion",
                "enable_anys",
                "anys_gateway",
                "read_tos_and_policy",
                "terms_modified",
                "ignored_version",
                "onboarding",
                "has_chosen_language",
                "initial_setup_done",
                "show_startup_screen",
                "message_check_time",
                "disabled_plugins",
                "fonts",
                "font_id",
            ],
            Self::Charts => &["charts", "local_records", "collections", "collection_uuids"],
            Self::ResPack => &["respacks", "respack_id"],
        }
    }
}

/// 导入/导出进度，按字节统计。
#[derive(Default)]
pub struct Progress {
    done: AtomicU64,
    total: AtomicU64,
}

impl Progress {
    pub fn new(total: u64) -> Self {
        Self {
            done: AtomicU64::new(0),
            total: AtomicU64::new(total),
        }
    }

    pub fn add(&self, bytes: u64) {
        self.done.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn set(&self, done: u64, total: u64) {
        self.total.store(total, Ordering::Relaxed);
        self.done.store(done, Ordering::Relaxed);
    }

    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// 收尾时对齐到 100%（目录条目等可能未计入字节数）。
    pub fn finish(&self) {
        self.done.store(self.total(), Ordering::Relaxed);
    }

    pub fn fraction(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            1.
        } else {
            (self.done() as f32 / total as f32).clamp(0., 1.)
        }
    }
}

/// 冲突数据处理策略：接收端已存在同名数据时的行为。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Conflict {
    /// 保留接收端已有数据，只导入缺失的部分
    Ignore,
    /// 覆盖接收端已有数据（默认）
    #[default]
    Overwrite,
}

impl Conflict {
    pub fn key(self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::Overwrite => "overwrite",
        }
    }

    fn from_key(key: &str) -> Self {
        if key == "ignore" {
            Self::Ignore
        } else {
            Self::Overwrite
        }
    }
}

/// 导入计划：由数据包清单决定。
pub struct ImportPlan {
    pub categories: Vec<Category>,
    pub data_json: Option<Vec<u8>>,
    pub total: u64,
    pub conflict: Conflict,
}

/// 读取数据包清单并生成导入计划。
pub fn prepare_import(file: &str) -> Result<ImportPlan> {
    let invalid = || anyhow!("invalid data package");
    let mut zip = zip::ZipArchive::new(BufReader::new(
        File::open(file).with_context(|| format!("failed to open {file}"))?,
    ))
    .map_err(|_| invalid())?;

    let (categories, conflict) = {
        let mut entry = zip.by_name("export.json").map_err(|_| invalid())?;
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        let value: Value = serde_json::from_str(&text).map_err(|_| invalid())?;
        let keys = value.get("categories").and_then(Value::as_array).ok_or_else(invalid)?;
        let mut categories = Vec::new();
        for key in keys.iter().filter_map(Value::as_str) {
            if let Some(category) = Category::from_key(key) {
                if !categories.contains(&category) {
                    categories.push(category);
                }
            }
        }
        let conflict = Conflict::from_key(value.get("conflict").and_then(Value::as_str).unwrap_or_default());
        (categories, conflict)
    };
    if categories.is_empty() {
        bail!("data package contains no categories");
    }

    let data_json = match zip.by_name("data.json") {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            merge_data_json(&bytes, &categories, conflict)?
        }
        Err(zip::result::ZipError::FileNotFound) => None,
        Err(err) => return Err(err.into()),
    };

    let mut total = data_json.as_ref().map(|it| it.len() as u64).unwrap_or(0);
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        if entry.name() == "data.json" || entry.name() == "export.json" {
            continue;
        }
        if import_target(entry.name(), &categories).is_some() {
            total += entry.size();
        }
    }

    Ok(ImportPlan {
        categories,
        data_json,
        total,
        conflict,
    })
}

/// 把数据包里的 data.json 合并进当前数据，只处理清单里声明的分类。
pub fn merge_data_json(bytes: &[u8], categories: &[Category], conflict: Conflict) -> Result<Option<Vec<u8>>> {
    let imported: Value = serde_json::from_slice(bytes)?;
    let Some(imported) = imported.as_object() else {
        return Ok(None);
    };
    let mut current_value = serde_json::to_value(get_data())?;
    let Some(current) = current_value.as_object_mut() else {
        return Ok(None);
    };
    let mut changed = false;
    for category in categories {
        for key in category.data_keys() {
            let Some(value) = imported.get(*key) else {
                continue;
            };
            match (conflict, *key) {
                (Conflict::Overwrite, _) => {
                    current.insert((*key).to_owned(), value.clone());
                    changed = true;
                }
                // 忽略冲突时只补充本地没有的谱面条目
                (Conflict::Ignore, "charts") => {
                    let existing = current.get("charts").and_then(Value::as_array).cloned().unwrap_or_default();
                    let known = existing
                        .iter()
                        .filter_map(|it| it.get("local_path").and_then(Value::as_str).map(str::to_owned))
                        .collect::<HashSet<_>>();
                    let mut merged = existing;
                    for chart in value.as_array().into_iter().flatten() {
                        let path = chart.get("local_path").and_then(Value::as_str).unwrap_or_default();
                        if known.contains(path) {
                            continue;
                        }
                        merged.push(chart.clone());
                        changed = true;
                    }
                    current.insert("charts".to_owned(), Value::Array(merged));
                }
                (Conflict::Ignore, "local_records") => {
                    let entry = current.entry("local_records").or_insert_with(|| Value::Object(Default::default()));
                    if let (Some(dst), Some(src)) = (entry.as_object_mut(), value.as_object()) {
                        for (path, record) in src {
                            if !dst.contains_key(path) {
                                dst.insert(path.clone(), record.clone());
                                changed = true;
                            }
                        }
                    }
                }
                (Conflict::Ignore, _) => {}
            }
        }
    }
    Ok(changed.then(|| serde_json::to_vec(&current_value)).transpose()?)
}

/// 包内路径 → (分类, 相对路径)；分类不在清单中时返回 None。
pub fn import_target<'a>(name: &'a str, categories: &[Category]) -> Option<(Category, &'a str)> {
    let (category, prefix) = if name.starts_with("charts/") {
        (Category::Charts, "charts/")
    } else if name.starts_with("respack/") {
        (Category::ResPack, "respack/")
    } else if name.starts_with("fonts/") {
        (Category::Settings, "fonts/")
    } else {
        return None;
    };
    if !categories.contains(&category) {
        return None;
    }
    Some((category, &name[prefix.len()..]))
}

/// 解压数据包到对应目录，最后写入 data.json。
pub fn run_import(file: String, plan: ImportPlan, progress: &Progress) -> Result<()> {
    let ImportPlan {
        categories,
        data_json,
        conflict,
        ..
    } = plan;
    let roots = [dir::charts()?, dir::respacks()?, dir::fonts()?];
    let mut zip = zip::ZipArchive::new(BufReader::new(
        File::open(&file).with_context(|| format!("failed to open {file}"))?,
    ))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_owned();
        let Some((category, rest)) = import_target(&name, &categories) else {
            continue;
        };
        if rest.is_empty() || rest.ends_with('/') {
            continue;
        }
        let relative = Path::new(rest);
        if !relative.components().all(|it| matches!(it, std::path::Component::Normal(_))) {
            tracing::warn!("skip unsafe path in data package: {name}");
            continue;
        }
        let root = &roots[match category {
            Category::Charts => 0,
            Category::ResPack => 1,
            Category::Settings => 2,
            _ => continue,
        }];
        let dest = Path::new(root).join(relative);
        // 忽略冲突：已存在的文件保持不动
        if conflict == Conflict::Ignore && dest.exists() {
            progress.add(entry.size());
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = BufWriter::new(
            File::create(&dest).with_context(|| format!("failed to create {}", dest.display()))?,
        );
        std::io::copy(&mut entry, &mut out)?;
        out.flush()?;
        progress.add(entry.size());
    }
    if let Some(bytes) = data_json {
        let path = format!("{}/data.json", dir::root()?);
        fs::write(&path, &bytes).with_context(|| format!("failed to write {path}"))?;
        progress.add(bytes.len() as u64);
    }
    progress.finish();
    Ok(())
}

/// 把导入后的 data.json 同步到内存，避免后续 save_data() 用旧数据覆盖。
pub fn apply_imported_data(bytes: &[u8]) -> Result<()> {
    let data: Data = serde_json::from_slice(bytes)?;
    *get_data_mut() = data;
    sync_data();
    Ok(())
}
