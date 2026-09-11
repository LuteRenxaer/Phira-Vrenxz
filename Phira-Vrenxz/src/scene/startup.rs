//! 加载主页的加载页。仿造 prpr 的 LoadingScene,只保留背景、Tip 和右下角的加载图标。
//! 首次启动时会解压内置资源（本体资源包）。
//! 内置谱面包（`assets/Level.zip` / Android 的 `Expansion_package.zip`）不再解压，
//! 由 `crate::scene::open_builtin_level_zip` 直接从 zip 读取。

use super::MainScene;
use crate::blue_archive_tips::random_tip;
prpr_l10n::tl_file!("login");
use prpr::{
    ext::{draw_parallelogram, poll_future, semi_white, LocalTask, PARALLELOGRAM_SLOPE},
    scene::{show_error, NextScene, Scene},
    time::TimeManager,
    ui::{FontArc, Ui},
};
use anyhow::Result;
use futures_util::Future;
use macroquad::prelude::*;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const FADE_IN_TIME: f32 = 0.5;
/// 主页加载完成后,加载页至少再显示这么久,避免一闪而过
const MIN_SHOW_TIME: f32 = 0.8;

/// 需要解压的内置资源包
/// (压缩包名, 描述)
/// 内置谱面包现在直接读 zip，不在此列。
const ASSET_PACKAGES: &[(&str, &str)] = &[("Ontology_package.zip", "本体资源包")];

/// 检查内置谱面包是否可用（存在且能作为 zip 打开，不解压）。
fn level_package_available(assets_dir: &std::path::Path) -> bool {
    let Some(path) = super::builtin_level_zip_in(assets_dir) else {
        return false;
    };
    match prpr::fs::ZipFileSystem::open(&path) {
        Ok(_) => true,
        Err(err) => {
            eprintln!("内置谱面包无法打开 {}: {err:#}", path.display());
            false
        }
    }
}

/// 检查所有资源包是否已就绪
fn all_packages_complete(assets_dir: &std::path::Path) -> bool {
    level_package_available(assets_dir) && assets_dir.join("achievements_icon").exists()
}

/// 解压进度共享状态
struct ExtractProgress {
    current: usize,
    total: usize,
    message: String,
    done: bool,
}

pub struct StartupLoadingScene {
    load_task: LocalTask<Result<MainScene>>,
    ready_scene: Option<Box<dyn Scene>>,
    finish_time: f32,
    enter_time: f32,
    tip: String,
    error: Option<String>,
    /// 解压进度（None 表示不需要解压）
    extract_progress: Option<Arc<Mutex<ExtractProgress>>>,
    /// 解压任务句柄
    extract_handle: Option<std::thread::JoinHandle<()>>,
    /// 缓存的 fallback font（解压完成后用于创建 MainScene）
    fallback_font: Option<FontArc>,
}

impl StartupLoadingScene {
    pub fn new(fallback: FontArc) -> Self {
        let tip = random_tip();

        // 检查是否需要解压（标记文件不存在且本体资源不完整）
        let assets_dir = super::assets_root();
        let marker = assets_dir.join(".extracted_builtin");
        let need_extract = !marker.exists() && !all_packages_complete(&assets_dir);

        let (extract_progress, extract_handle) = if need_extract {
            let progress = Arc::new(Mutex::new(ExtractProgress {
                current: 0,
                total: ASSET_PACKAGES.len(),
                message: "准备解压...".to_string(),
                done: false,
            }));
            let progress_clone = progress.clone();
            let handle = std::thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    extract_builtin_assets(&progress_clone);
                }));
                if result.is_err() {
                    if let Ok(mut p) = progress_clone.lock() {
                        p.done = true;
                        p.message = "解压时发生错误".to_string();
                    }
                }
            });
            (Some(progress), Some(handle))
        } else {
            (None, None)
        };

        // 如果不需要解压，直接开始加载 MainScene
        let (load_task, fallback_font) = if need_extract {
            (None, Some(fallback))
        } else {
            let task: Pin<Box<dyn Future<Output = Result<MainScene>>>> = Box::pin(async move { MainScene::new(fallback).await });
            (Some(task), None)
        };

        Self {
            load_task,
            ready_scene: None,
            finish_time: f32::INFINITY,
            enter_time: f32::NAN,
            tip,
            error: None,
            extract_progress,
            extract_handle,
            fallback_font,
        }
    }
}

/// 在后台线程解压内置资源
fn extract_builtin_assets(progress: &Arc<Mutex<ExtractProgress>>) {
    let assets_dir = super::assets_root();
    let mut all_ok = true;

    for (i, (archive_name, desc)) in ASSET_PACKAGES.iter().enumerate() {
        let archive_path = assets_dir.join(archive_name);

        {
            let mut p = progress.lock().unwrap();
            p.current = i;
            p.message = format!("正在解压{}...", desc);
        }

        if !archive_path.exists() {
            eprintln!("Archive not found: {}", archive_path.display());
            all_ok = false;
            continue;
        }

        // 解压到 assets 目录（自动跳过顶层目录）
        match extract_zip_skip_top_dir(&archive_path, &assets_dir) {
            Ok(_) => {
                eprintln!("Successfully extracted {}", archive_name);
            }
            Err(e) => {
                eprintln!("Failed to extract {}: {}", archive_name, e);
                all_ok = false;
            }
        }
    }

    // 只有所有包都解压成功才写标记文件
    if all_ok {
        let marker = assets_dir.join(".extracted_builtin");
        let _ = std::fs::write(&marker, "extracted");
    }

    let mut p = progress.lock().unwrap();
    p.done = true;
    p.message = if all_ok { "解压完成".to_string() } else { "解压部分失败".to_string() };
}

/// 解压 zip 文件，自动跳过顶层目录
fn extract_zip_skip_top_dir(
    archive_path: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // 先确定顶层目录名（如果所有条目都在同一个目录下）
    let mut top_dir: Option<String> = None;
    for i in 0..archive.len() {
        let f = archive.by_index(i)?;
        let name = f.name().to_string();
        if name.is_empty() || name == "/" {
            continue;
        }
        // 找第一个 / 之前的部分
        let first_sep = name.find('/').or_else(|| name.find('\\'));
        let current_top = match first_sep {
            Some(idx) => &name[..idx],
            None => {
                // 没有顶层目录，直接解压
                top_dir = None;
                break;
            }
        };
        match &top_dir {
            None => top_dir = Some(current_top.to_string()),
            Some(existing) if existing != current_top => {
                // 有多个不同的顶层目录，不跳过
                top_dir = None;
                break;
            }
            _ => {}
        }
    }

    let top_dir_prefix = top_dir.map(|d| format!("{}/", d));

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let enclosed = match file.enclosed_name() {
            Some(path) => path,
            None => continue,
        };

        let path_str = enclosed.to_string_lossy().replace('\\', "/");

        // 跳过顶层目录前缀
        let relative_path = if let Some(prefix) = &top_dir_prefix {
            if let Some(rest) = path_str.strip_prefix(prefix) {
                rest
            } else {
                &path_str
            }
        } else {
            &path_str
        };

        if relative_path.is_empty() {
            continue;
        }

        // 角色功能已移除：不再解压 skel / voice_Line 内容
        let first_seg = relative_path.split(['/', '\\']).next().unwrap_or("");
        if first_seg == "skel" || first_seg == "voice_Line" {
            continue;
        }

        let outpath = output_dir.join(relative_path);

        if file.is_dir() || relative_path.ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

impl Scene for StartupLoadingScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.enter_time.is_nan() {
            self.enter_time = tm.now() as f32;
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        // 检查解压是否完成
        if let Some(progress) = &self.extract_progress {
            let done = progress.lock().map(|p| p.done).unwrap_or(true);
            if done && self.load_task.is_none() {
                // 解压完成，开始加载 MainScene
                if let Some(fallback) = self.fallback_font.take() {
                    let task: Pin<Box<dyn Future<Output = Result<MainScene>>>> = Box::pin(async move { MainScene::new(fallback).await });
                    self.load_task = Some(task);
                }
            }
        }

        if let Some(future) = self.load_task.as_mut() {
            if let Some(res) = poll_future(future.as_mut()) {
                self.load_task = None;
                match res {
                    Ok(scene) => {
                        self.ready_scene = Some(Box::new(scene));
                        self.finish_time = tm.now() as f32 + MIN_SHOW_TIME;
                    }
                    Err(err) => {
                        self.error = Some(format!("{err:#}"));
                        show_error(err.context(tl!("startup-init-failed").to_string()));
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        // 背景使用原始比例，不随 UI 比例缩放
        set_camera(&ui.bg_camera());
        let t = tm.now() as f32;
        let top = ui.top;
        let full = ui.screen_rect();

        // 深色渐变背景
        ui.fill_rect(
            full,
            (
                Color::new(0.10, 0.12, 0.18, 1.),
                (full.x, full.y),
                Color::new(0.02, 0.03, 0.06, 1.),
                (full.x, full.bottom()),
            ),
        );

        // UI 使用带比例的 camera
        set_camera(&ui.camera());

        let alpha = ((t - self.enter_time) / FADE_IN_TIME).clamp(0., 1.);
        ui.alpha(alpha, |ui| {
            if let Some(err) = &self.error {
                ui.text(tl!("startup-load-failed"))
                    .pos(0., 0.)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.5)
                    .color(RED)
                    .draw();
                ui.text(err)
                    .pos(0., 0.08)
                    .anchor(0.5, 0.)
                    .max_width(1.4)
                    .size(0.3)
                    .color(semi_white(0.7))
                    .draw();
            } else if let Some(progress) = &self.extract_progress {
                // 显示解压进度
                let (msg, done, current, total) = if let Ok(p) = progress.lock() {
                    (p.message.clone(), p.done, p.current, p.total)
                } else {
                    ("解压错误".to_string(), true, 0, 1)
                };
                let msg = if done {
                    "Loading...".to_string()
                } else {
                    format!("{} ({}/{})", msg, current + 1, total)
                };

                // 右下角进度文字（仿 loading.rs 风格）
                let load_text = msg.as_str();
                let txt = ui.text(load_text)
                    .pos(0.93, top * 0.92)
                    .anchor(1., 1.)
                    .size(0.42)
                    .color(WHITE)
                    .draw();
                let we = 0.2;
                let he = 0.5;
                let r = Rect::new(txt.x - txt.w * we, txt.y - txt.h * he, txt.w * (1. + we * 2.), txt.h * (1. + he * 2.));

                // 进度条（平行四边形，白色）
                if !done && total > 0 {
                    let progress_val = current as f32 / total as f32;
                    // 背景（半透明）
                    draw_parallelogram(r, None, Color::new(1., 1., 1., 0.15), false);
                    // 进度填充（白色平行四边形）
                    if progress_val > 0.01 {
                        let fill_w = r.w * progress_val;
                        let fill_r = Rect::new(r.x, r.y, fill_w, r.h);
                        draw_parallelogram(fill_r, None, WHITE, false);
                        // scissor 显示黑色文字在进度条上
                        ui.scissor(fill_r, |ui| {
                            ui.text(load_text)
                                .pos(0.93, top * 0.92)
                                .anchor(1., 1.)
                                .size(0.42)
                                .color(BLACK)
                                .draw();
                        });
                    }
                } else {
                    // 加载中（解压完成后），用扫描动画
                    let pp = 0.6;
                    let s = 0.2;
                    let t_val = ((t - 0.3).max(0.) % (pp * 2. + s)) / pp;
                    let st = (t_val - 1.).clamp(0., 1.).powi(3);
                    let en = 1. - (1. - t_val.min(1.)).powi(3);
                    let progress_r = Rect::new(r.x + r.w * st, r.y, r.w * (en - st), r.h);
                    if progress_r.w > 0.001 {
                        ui.fill_rect(progress_r, WHITE);
                        ui.scissor(progress_r, |ui| {
                            ui.text(load_text)
                                .pos(0.93, top * 0.92)
                                .anchor(1., 1.)
                                .size(0.42)
                                .color(BLACK)
                                .draw();
                        });
                    }
                }

                // Tip(左下角)
                ui.text(tl!("startup-tip", "tip" => &self.tip))
                    .pos(-0.95, top - 0.05)
                    .anchor(0., 1.)
                    .max_width(1.6)
                    .size(0.4)
                    .color(semi_white(0.6))
                    .draw();
            } else {
                // Tip(左下角)
                ui.text(tl!("startup-tip", "tip" => &self.tip))
                    .pos(-0.95, top - 0.05)
                    .anchor(0., 1.)
                    .max_width(1.6)
                    .size(0.4)
                    .color(semi_white(0.6))
                    .draw();

                // 右下角 Loading... 扫描动画(仿 prpr LoadingScene)
                draw_loading_animation(ui, t, top);
            }
        });

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.ready_scene.is_some() && tm.now() as f32 > self.finish_time {
            let scene = self.ready_scene.take().unwrap();
            return NextScene::Replace(scene);
        }
        NextScene::None
    }
}

fn draw_loading_animation(ui: &mut Ui, now: f32, top: f32) {
    let load_text = "Loading...";
    let t = ui
        .text(load_text)
        .pos(0.93, top * 0.92)
        .anchor(1., 1.)
        .size(0.42)
        .color(WHITE)
        .draw();
    let we = 0.2;
    let he = 0.5;
    let r = Rect::new(t.x - t.w * we, t.y - t.h * he, t.w * (1. + we * 2.), t.h * (1. + he * 2.));

    let p = 0.6;
    let s = 0.2;
    let t_val = ((now - 0.3).max(0.) % (p * 2. + s)) / p;
    let st = (t_val - 1.).clamp(0., 1.).powi(3);
    let en = 1. - (1. - t_val.min(1.)).powi(3);

    let progress_r = Rect::new(r.x + r.w * st, r.y, r.w * (en - st), r.h);
    ui.fill_rect(progress_r, WHITE);
    ui.scissor(progress_r, |ui| {
        ui.text(load_text)
            .pos(0.93, top * 0.92)
            .anchor(1., 1.)
            .size(0.42)
            .color(BLACK)
            .draw();
    });
}
