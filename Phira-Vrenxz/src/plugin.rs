//! 插件加载模块（新框架格式）
//!
//! 扫描 exe 旁 `plugins\` 目录，每个子目录视为一个插件，目录结构约定：
//! - `info.json`：插件元信息（name / version / author / description / supported_version_list，
//!   可选 `main_folder_name`（默认 "main"）、`assets_folder_name`（默认 "assets"））
//! - `main\`：入口代码目录（含 .rs / .json 等文本入口文件）
//! - `assets\`：资源目录（图片 / json 等）
//!
//! 损坏 / 缺 info.json 的目录会被跳过。

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// info.json 元信息（新框架格式）
#[derive(Debug, Clone, Deserialize)]
pub struct PluginInfo {
    /// 插件名称
    pub name: String,
    /// 版本号
    pub version: String,
    /// 作者
    pub author: String,
    /// 简介
    pub description: String,
    /// 插件类型（缺省 general）
    #[serde(alias = "type", default)]
    pub plugin_type: Option<String>,
    /// 支持的版本列表
    #[serde(default)]
    pub supported_version_list: Vec<String>,
    /// 主文件夹名，默认 "main"（模板字段拼写为 `mian_folder_name`，二者均兼容）
    #[serde(alias = "mian_folder_name", default)]
    pub main_folder_name: Option<String>,
    /// 资源文件夹名，默认 "assets"
    #[serde(alias = "assets_folder_name", default)]
    pub assets_folder_name: Option<String>,
}

impl PluginInfo {
    /// 插件类型（缺省 "general"）
    pub fn plugin_type(&self) -> &str {
        self.plugin_type.as_deref().unwrap_or("general")
    }

    /// 主文件夹名（缺省 "main"）
    pub fn main_folder_name(&self) -> &str {
        self.main_folder_name.as_deref().unwrap_or("main")
    }

    /// 资源文件夹名（缺省 "assets"）
    pub fn assets_folder_name(&self) -> &str {
        self.assets_folder_name.as_deref().unwrap_or("assets")
    }
}

/// 单个已加载插件
#[derive(Debug, Clone)]
pub struct Plugin {
    /// 插件元信息
    pub info: PluginInfo,
    /// 插件目录（plugins\<插件名>）
    pub dir: PathBuf,
    /// 主文件夹绝对路径
    pub main_dir: PathBuf,
    /// 资源文件夹绝对路径
    pub assets_dir: PathBuf,
    /// 主文件夹下的入口文件（.rs / .json 等，存在且可读取文本）
    pub entry_files: Vec<PathBuf>,
    /// 入口是否有效（存在且可读取文本）
    pub entry_ok: bool,
    /// 入口异常说明（entry_ok 为 false 时）
    pub entry_error: Option<String>,
    /// 若来自 Phira Pro 插件运行时，对应 manifest.id
    pub pro_id: Option<String>,
    pub pro_enabled: bool,
}

impl Plugin {
    /// 插件当前是否启用（插件管理页开关控制）。
    pub fn enabled(&self) -> bool {
        if self.pro_id.is_some() {
            return self.pro_enabled;
        }
        let data = crate::get_data();
        !data.disabled_plugins.iter().any(|n| n == &self.info.name)
    }
}

/// 视为入口文件的常见文本扩展名
const ENTRY_EXTS: &[&str] = &["rs", "json", "py", "js", "ts", "txt", "yml", "yaml", "toml"];

/// 扫描 exe 所在目录下的 `plugins\` 目录并加载全部插件。
/// plugins 目录不存在或不可读时返回空列表。
pub fn load_plugins() -> Vec<Plugin> {
    let mut out = plugins_dir().map(|dir| load_plugins_from(&dir)).unwrap_or_default();
    if let Ok(list) = crate::plugins::installed() {
        for p in list {
            if out.iter().any(|it| it.pro_id.as_deref() == Some(&p.manifest.id)) {
                continue;
            }
            let dir = plugins_dir().unwrap_or_default();
            out.push(Plugin {
                info: PluginInfo {
                    name: p.manifest.name.clone(),
                    version: p.manifest.version.clone(),
                    author: p.manifest.author.clone(),
                    description: p.manifest.description.clone(),
                    plugin_type: Some("general".into()),
                    supported_version_list: vec![],
                    main_folder_name: None,
                    assets_folder_name: None,
                },
                dir: dir.clone(),
                main_dir: dir.clone(),
                assets_dir: dir,
                entry_files: vec![],
                entry_ok: true,
                entry_error: None,
                pro_id: Some(p.manifest.id),
                pro_enabled: p.enabled,
            });
        }
    }
    out
}

pub fn delete_plugin(plugin: &Plugin) -> Result<()> {
    if let Some(id) = &plugin.pro_id {
        crate::plugins::change(id, None, None, true)?;
        return Ok(());
    }
    if plugin.dir.exists() {
        fs::remove_dir_all(&plugin.dir)?;
    }
    Ok(())
}

/// exe 所在目录下的 plugins 目录路径（不存在时返回 None）
pub fn plugins_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    Some(parent.join("plugins"))
}

/// 从指定目录加载全部插件子目录（损坏 / 缺 info.json 的目录跳过）
pub fn load_plugins_from(dir: &Path) -> Vec<Plugin> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(p) = load_plugin(&path) {
            out.push(p);
        }
    }
    out
}

/// 加载单个插件目录；返回 None 表示该目录不是有效插件（损坏 / 缺 info.json）
fn load_plugin(dir: &Path) -> Option<Plugin> {
    let info_path = dir.join("info.json");
    let (info, main_name, assets_name) = if info_path.exists() {
        let raw = fs::read_to_string(&info_path).ok()?;
        let raw = raw.trim_start_matches('\u{feff}');
        let info: PluginInfo = serde_json::from_str(raw).ok()?;
        let mn = info.main_folder_name().to_string(); let an = info.assets_folder_name().to_string(); (info, mn, an)
    } else {
        // PVN 格式：About.json + code/main.json + assets/
        let about = dir.join("About.json");
        let raw = fs::read_to_string(&about).ok()?;
        let v: serde_json::Value = serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()?;
        let info = PluginInfo {
            name: v.get("name").and_then(|x| x.as_str()).unwrap_or("PVN Plugin").to_string(),
            version: v.get("version").and_then(|x| x.as_str()).unwrap_or("0").to_string(),
            author: v.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            description: v.get("description").and_then(|x| x.as_str()).unwrap_or("PVN 插件").to_string(),
            plugin_type: Some("pvn".into()),
            supported_version_list: vec![],
            main_folder_name: None,
            assets_folder_name: None,
        };
        (info, "code".to_string(), "assets".to_string())
    };

    let main_dir = dir.join(&main_name);
    let assets_dir = dir.join(&assets_name);

    // 收集主文件夹下入口文件：存在且可读取文本才算有效
    let mut entry_files = Vec::new();
    if let Ok(entries) = fs::read_dir(&main_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ENTRY_EXTS.contains(&ext.as_str()) && fs::read_to_string(&p).is_ok() {
                entry_files.push(p);
            }
        }
    }
    entry_files.sort();

    let entry_ok = !entry_files.is_empty();
    let entry_error = if entry_ok {
        None
    } else {
        Some(format!(
            "入口缺失：{} 目录下无 .rs/.json 等文本文件",
            info.main_folder_name()
        ))
    };

    Some(Plugin {
        info,
        dir: dir.to_path_buf(),
        main_dir,
        assets_dir,
        entry_files,
        entry_ok,
        entry_error,
        pro_id: None,
        pro_enabled: false,
    })
}

/// 设置插件启用状态并持久化到 data.json（插件管理页开关调用）。
pub fn set_plugin_enabled(info: &PluginInfo, enabled: bool) -> Result<()> {
    if let Ok(list) = crate::plugins::installed() {
        if let Some(p) = list.into_iter().find(|p| p.manifest.name == info.name) {
            let r = crate::plugins::change(&p.manifest.id, Some(enabled), None, false);
            crate::plugins::reload();
            return r;
        }
    }
    {
        let data = crate::get_data_mut();
        if enabled {
            data.disabled_plugins.retain(|n| n != &info.name);
        } else if !data.disabled_plugins.iter().any(|n| n == &info.name) {
            data.disabled_plugins.push(info.name.clone());
        }
    }
    crate::save_data()
}

/// 插件包支持的扩展名（三种都按 zip 容器解析）
pub const PLUGIN_ARCHIVE_EXTS: &[&str] = &["zip", "zips", "pvn"];

/// plugins 目录不存在时创建（首次导入插件时用）
pub fn ensure_plugins_dir() -> Result<PathBuf> {
    let dir = plugins_dir().ok_or_else(|| anyhow::anyhow!("无法定位程序目录"))?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// 目录名安全化：去掉 Windows 非法字符
fn sanitize_dir_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        "plugin".to_string()
    } else {
        s
    }
}

/// 从插件包导入插件，支持 `.zip` / `.zips` / `.pvn`（三者都是 zip 容器）。
///
/// 包内 `info.json` 可以在根目录，也可以套一层外壳目录（自动剥掉这一层）。
/// 导入到 `plugins/<插件名>/`，同名插件会被覆盖。返回导入后的插件。
pub fn import_plugin_archive(path: &Path) -> Result<Plugin> {
    let file = fs::File::open(path).with_context(|| format!("打开插件包失败：{}", path.display()))?;
    let mut archive = ZipArchive::new(file).context("插件包不是有效的 zip / pvn 容器")?;

    // PVN 框架：info.json + main/main.rs + assets。main.rs 作为 Rhai 脚本执行。
    {
        let mut info_raw = None;
        let mut script = None;
        for i in 0..archive.len() {
            let Ok(mut entry) = archive.by_index(i) else { continue };
            let name = entry.name().replace('\\', "/");
            if name == "info.json" || name.ends_with("/info.json") {
                let mut raw = String::new();
                use std::io::Read;
                if entry.read_to_string(&mut raw).is_ok() { info_raw = Some(raw); }
            } else if name == "main/main.rs" || name.ends_with("/main/main.rs") {
                let mut raw = String::new();
                use std::io::Read;
                if entry.read_to_string(&mut raw).is_ok() { script = Some(raw); }
            }
        }
        if let (Some(info_raw), Some(script)) = (info_raw, script) {
            let v: serde_json::Value = serde_json::from_str(info_raw.trim_start_matches('\u{feff}')).unwrap_or_default();
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("PVN Plugin").to_string();
            let version = v.get("version").and_then(|x| x.as_str()).unwrap_or("0").to_string();
            let author = v.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let description = v.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let slug = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
            let id = if slug.is_empty() { "pvn.keydisplay".to_string() } else { format!("pvn.{slug}") };
            let manifest = serde_json::json!({
                "api": 3, "id": id, "name": name, "version": version, "author": author,
                "description": description, "permissions": ["overlay.draw"],
                "settings": {"color": 5002495u64, "background": 0.62, "opacity": 1.0},
                "assets": {}, "assetFiles": {}
            });
            let tmp = std::env::temp_dir().join(format!("pvn-{}.zip", std::process::id()));
            {
                use std::io::Write;
                let mut z = zip::ZipWriter::new(fs::File::create(&tmp)?);
                let opt = zip::write::SimpleFileOptions::default();
                z.start_file("manifest.json", opt)?;
                z.write_all(serde_json::to_string(&manifest)?.as_bytes())?;
                z.start_file("main.rhai", opt)?;
                z.write_all(script.as_bytes())?;
                z.finish()?;
            }
            drop(archive);
            crate::plugins::import(&tmp)?;
            crate::plugins::change(&id, Some(true), None, false)?;
            let _ = fs::remove_file(&tmp);
            let dir = ensure_plugins_dir()?.join(sanitize_dir_name(&name));
            return Ok(Plugin { info: PluginInfo { name, version, author, description, plugin_type: Some("pvn".into()), supported_version_list: vec![], main_folder_name: None, assets_folder_name: None }, dir: dir.clone(), main_dir: dir.clone(), assets_dir: dir, entry_files: vec![], entry_ok: true, entry_error: None, pro_id: Some(id), pro_enabled: true });
        }
    }

    // Phira Pro 格式（manifest.json + main.rhai）：交给新插件运行时导入。
    let mut pro_manifest = None;
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else { continue };
        let name = entry.name().replace('\\', "/");
        if name == "manifest.json" || name.ends_with("/manifest.json") {
            let mut raw = String::new();
            use std::io::Read;
            if entry.read_to_string(&mut raw).is_ok() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if v.get("api").is_some() && v.get("id").is_some() {
                        pro_manifest = Some(v);
                        break;
                    }
                }
            }
        }
    }
    if let Some(v) = pro_manifest {
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("phira-pro.ppsr").to_string();
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("Phira Pro Plugin").to_string();
        let version = v.get("version").and_then(|x| x.as_str()).unwrap_or("0").to_string();
        let author = v.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let description = v.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string();
        drop(archive);
        let already = crate::plugins::installed().map(|l| l.iter().any(|p| p.manifest.id == id)).unwrap_or(false);
        if already {
            crate::plugins::change(&id, Some(true), None, false)?;
        } else {
            crate::plugins::import(path)?;
            crate::plugins::change(&id, Some(true), None, false)?;
        }
        let dir = ensure_plugins_dir()?.join(sanitize_dir_name(&name));
        return Ok(Plugin {
            info: PluginInfo {
                name,
                version,
                author,
                description,
                plugin_type: Some("general".into()),
                supported_version_list: vec![],
                main_folder_name: None,
                assets_folder_name: None,
            },
            dir: dir.clone(),
            main_dir: dir.clone(),
            assets_dir: dir.clone(),
            entry_files: vec![],
            entry_ok: true,
            entry_error: None,
            pro_id: Some(id.clone()),
            pro_enabled: true,
        });
    }

    // 所有条目都在同一个顶层目录下时，去掉这一层外壳
    let single_root = {
        let mut roots = std::collections::HashSet::new();
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                if let Some(root) = entry.name().split('/').next() {
                    if !root.is_empty() {
                        roots.insert(root.to_string());
                    }
                }
            }
        }
        if roots.len() == 1 {
            roots.into_iter().next()
        } else {
            None
        }
    };

    let root_dir = ensure_plugins_dir()?;
    // 清掉上次导入失败留下的临时目录
    if let Ok(entries) = fs::read_dir(&root_dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("_import_") {
                fs::remove_dir_all(e.path()).ok();
            }
        }
    }
    let tmp = root_dir.join(format!("_import_{}", std::process::id()));
    if tmp.exists() {
        fs::remove_dir_all(&tmp).ok();
    }
    fs::create_dir_all(&tmp)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("读取插件包条目失败")?;
        let name = entry.name().to_string();
        let rel = match &single_root {
            Some(root) if name.starts_with(&format!("{root}/")) => name[root.len() + 1..].to_string(),
            _ => name.clone(),
        };
        if rel.is_empty() {
            continue;
        }
        let out = tmp.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).ok();
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).ok();
            }
            let mut f = fs::File::create(&out).with_context(|| format!("写入失败：{}", out.display()))?;
            std::io::copy(&mut entry, &mut f).context("写入插件文件失败")?;
        }
    }

    // 校验：必须有可解析的 info.json
    let parsed = load_plugin(&tmp).ok_or_else(|| anyhow::anyhow!("插件包缺少有效的 info.json"))?;
    let target = root_dir.join(sanitize_dir_name(&parsed.info.name));
    if target.exists() {
        fs::remove_dir_all(&target).with_context(|| format!("删除旧插件失败：{}", target.display()))?;
    }
    fs::rename(&tmp, &target).or_else(|_| -> Result<()> {
        // 跨盘/占用时退化为逐文件复制
        let mut stack = vec![tmp.clone()];
        while let Some(dir) = stack.pop() {
            for e in fs::read_dir(&dir)?.flatten() {
                let p = e.path();
                let rel = p.strip_prefix(&tmp).unwrap_or(&p);
                let dst = target.join(rel);
                if p.is_dir() {
                    fs::create_dir_all(&dst)?;
                    stack.push(p);
                } else {
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&p, &dst)?;
                }
            }
        }
        fs::remove_dir_all(&tmp).ok();
        Ok(())
    })?;

    load_plugin(&target).ok_or_else(|| anyhow::anyhow!("导入后插件校验失败"))
}
