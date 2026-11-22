
use super::MainScene;
use crate::tips::random_tip;
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

const MIN_SHOW_TIME: f32 = 0.8;

pub(crate) const ASSET_PACKAGES: &[(&str, &str)] = if cfg!(target_os = "android") {
    &[("Ontology_package.zip", "本体资源包")]
} else {
    &[("Ontology_package.pak", "本体资源包")]
};

pub(crate) fn level_package_available(assets_dir: &std::path::Path) -> bool {
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

// 本体包完整性检查只关心必需的资源组；包里若含已废弃的目录（例如 achievements_icon）
// 会被解压出来但不参与检查，多余目录不影响判定。
pub(crate) const ONTOLOGY_GROUPS: &[&str] = &["icon_old(home)", "mod", "rank", "respack", "sfx", "Tutorial"];

pub(crate) fn all_packages_complete(assets_dir: &std::path::Path) -> bool {
    level_package_available(assets_dir) && ONTOLOGY_GROUPS.iter().all(|group| assets_dir.join(group).exists())
}

pub(crate) fn extract_stamp(assets_dir: &std::path::Path) -> String {
    ASSET_PACKAGES
        .iter()
        .map(|(name, _)| match std::fs::metadata(assets_dir.join(name)) {
            Ok(meta) => {
                let secs = meta
                    .modified()
                    .ok()
                    .and_then(|it| it.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|it| it.as_secs())
                    .unwrap_or(0);
                format!("{name}:{}:{secs}", meta.len())
            }
            Err(_) => format!("{name}:missing"),
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub(crate) struct ExtractProgress {
    pub(crate) current: usize,
    pub(crate) total: usize,
    pub(crate) message: String,
    pub(crate) done: bool,
}

pub struct StartupLoadingScene {
    load_task: LocalTask<Result<MainScene>>,
    ready_scene: Option<Box<dyn Scene>>,
    finish_time: f32,
    enter_time: f32,
    tip: &'static str,
    error: Option<String>,

    extract_progress: Option<Arc<Mutex<ExtractProgress>>>,

    extract_handle: Option<std::thread::JoinHandle<()>>,

    fallback_font: Option<FontArc>,
}

impl StartupLoadingScene {
    pub fn new(fallback: FontArc) -> Self {
        let tip = random_tip();

        let assets_dir = super::assets_root();
        let marker = assets_dir.join(".extracted_builtin");
        let need_extract = match std::fs::read_to_string(&marker) {
            Ok(stamp) => stamp.trim() != extract_stamp(&assets_dir),
            Err(_) => !all_packages_complete(&assets_dir),
        };

        let (extract_progress, extract_handle) = if need_extract {
            let progress = Arc::new(Mutex::new(ExtractProgress {
                current: 0,
                total: ASSET_PACKAGES.len(),
                message: crate::tr!("准备解压...").to_string(),
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
                        p.message = crate::tr!("解压时发生错误").to_string();
                    }
                }
            });
            (Some(progress), Some(handle))
        } else {
            (None, None)
        };

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

pub(crate) fn extract_builtin_assets(progress: &Arc<Mutex<ExtractProgress>>) {
    let assets_dir = super::assets_root();
    let mut all_ok = true;

    for (i, (archive_name, desc)) in ASSET_PACKAGES.iter().enumerate() {
        let archive_path = assets_dir.join(archive_name);

        {
            let mut p = progress.lock().unwrap();
            p.current = i;
            p.message = format!("{}{}...", crate::tr!("正在解压"), crate::extra_l10n::tr(desc));
        }

        if !archive_path.exists() {
            eprintln!("Archive not found: {}", archive_path.display());
            all_ok = false;
            continue;
        }

        match extract_builtin_archive(&archive_path, &assets_dir) {
            Ok(_) => {
                eprintln!("Successfully extracted {}", archive_name);
            }
            Err(e) => {
                eprintln!("Failed to extract {}: {}", archive_name, e);
                all_ok = false;
            }
        }
    }

    if all_ok {
        let marker = assets_dir.join(".extracted_builtin");
        let _ = std::fs::write(&marker, extract_stamp(&assets_dir));
    }

    let mut p = progress.lock().unwrap();
    p.done = true;
    p.message = if all_ok { crate::tr!("解压完成").to_string() } else { crate::tr!("解压部分失败").to_string() };
}

pub(crate) fn extract_zip_skip_top_dir(
    archive_path: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(archive_path)?;
    let archive = zip::ZipArchive::new(file)?;
    extract_zip_inner(archive, output_dir)
}

/// 从内存字节解压（跳过顶层目录），用于解密后的本体资源包。
pub(crate) fn extract_zip_bytes(
    data: &[u8],
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(data))?;
    extract_zip_inner(archive, output_dir)
}

/// 解压本体资源包：桌面端为加密 PVX1 包，Android 端为明文 zip。
#[cfg(not(target_os = "android"))]
fn extract_builtin_archive(
    archive_path: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read(archive_path)?;
    let zip_bytes = crate::pack::decrypt_pack(&data)?;
    extract_zip_bytes(&zip_bytes, output_dir)
}

#[cfg(target_os = "android")]
fn extract_builtin_archive(
    archive_path: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    extract_zip_skip_top_dir(archive_path, output_dir)
}

fn extract_zip_inner<R: std::io::Read + std::io::Seek>(
    mut archive: zip::ZipArchive<R>,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut top_dir: Option<String> = None;
    for i in 0..archive.len() {
        let f = archive.by_index(i)?;
        let name = f.name().to_string();
        if name.is_empty() || name == "/" {
            continue;
        }

        let first_sep = name.find('/').or_else(|| name.find('\\'));
        let current_top = match first_sep {
            Some(idx) => &name[..idx],
            None => {

                top_dir = None;
                break;
            }
        };
        match &top_dir {
            None => top_dir = Some(current_top.to_string()),
            Some(existing) if existing != current_top => {

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

        if let Some(progress) = &self.extract_progress {
            let done = progress.lock().map(|p| p.done).unwrap_or(true);
            if done && self.load_task.is_none() {

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

        set_camera(&ui.bg_camera());
        let t = tm.now() as f32;
        let top = ui.top;
        let full = ui.screen_rect();

        ui.fill_rect(
            full,
            (
                Color::new(0.10, 0.12, 0.18, 1.),
                (full.x, full.y),
                Color::new(0.02, 0.03, 0.06, 1.),
                (full.x, full.bottom()),
            ),
        );

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

                let (msg, done, current, total) = if let Ok(p) = progress.lock() {
                    (p.message.clone(), p.done, p.current, p.total)
                } else {
                    (crate::tr!("解压错误").to_string(), true, 0, 1)
                };
                let msg = if done {
                    "Loading...".to_string()
                } else {
                    format!("{} ({}/{})", msg, current + 1, total)
                };

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

                if !done && total > 0 {
                    let progress_val = current as f32 / total as f32;

                    draw_parallelogram(r, None, Color::new(1., 1., 1., 0.15), false);

                    if progress_val > 0.01 {
                        let fill_w = r.w * progress_val;
                        let fill_r = Rect::new(r.x, r.y, fill_w, r.h);
                        draw_parallelogram(fill_r, None, WHITE, false);

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

                draw_loading_tip(ui, top, self.tip);
            } else {

                draw_loading_tip(ui, top, self.tip);

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

/// Phigros 式加载提示：左下角一个白色斜切「TIP」标签 + 一行提示文字
fn draw_loading_tip(ui: &mut Ui, top: f32, tip: &str) {
    let tag = Rect::new(-0.95, top - 0.115, 0.235, 0.062);
    // 斜切标签（跟 UI 变换/淡入一起走，别用裸 GL 画）
    let sk = tag.h * PARALLELOGRAM_SLOPE;
    let mut b = lyon::path::Path::builder();
    b.begin(lyon::math::point(tag.x + sk, tag.y));
    b.line_to(lyon::math::point(tag.right() + sk, tag.y));
    b.line_to(lyon::math::point(tag.right(), tag.bottom()));
    b.line_to(lyon::math::point(tag.x, tag.bottom()));
    b.end(true);
    ui.fill_path(&b.build(), WHITE);
    ui.text("TIP")
        .pos(tag.center().x, tag.center().y)
        .anchor(0.5, 0.5)
        .size(0.30)
        .color(BLACK)
        .draw();
    ui.text(tip)
        .pos(tag.right() + 0.03, tag.center().y)
        .anchor(0., 0.5)
        .max_width(1.28)
        .size(0.34)
        .color(semi_white(0.62))
        .draw();
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
