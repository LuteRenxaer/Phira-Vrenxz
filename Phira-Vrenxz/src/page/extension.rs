use super::{Page, SharedState};
use crate::{
    dir,
    extension::{DownloadPhase, ExtensionManager, ExtensionInfo, ExtensionType},
};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{poll_future, semi_black, semi_white, LocalTask, RectExt, SafeTexture},
    scene::{show_error, show_message},
    ui::{button_hit, DRectButton, Scroll, Ui},
};
use std::{
    borrow::Cow,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const DEFAULT_FOLDER_URL: &str = "https://wwamt.lanzout.com/b01bjn9e9i/";
const DEFAULT_FOLDER_PASSWORD: &str = "hlib";

pub struct ExtensionPage {
    manager: Arc<Mutex<ExtensionManager>>,

    // 列表
    items: Vec<ExtensionItem>,
    item_rects: Vec<Rect>,
    list_scroll: Scroll,
    selected: usize,

    // 下载
    download_level_btn: DRectButton,
    refresh_btn: DRectButton,
    download_task: LocalTask<Result<String>>,
    download_progress: Option<Arc<tokio::sync::Mutex<crate::extension::DownloadProgress>>>,
    current_download_id: Option<String>,
    current_download_type: Option<String>,

    // 操作按钮
    toggle_btn: DRectButton,
    delete_btn: DRectButton,

    // 动画
    enter_time: f32,
}

struct ExtensionItem {
    info: ExtensionInfo,
}

impl ExtensionPage {
    pub fn new() -> Result<Self> {
        let data_dir = PathBuf::from(dir::root()?);
        let manager = ExtensionManager::new(&data_dir)?;
        let manager = Arc::new(Mutex::new(manager));

        let mut page = Self {
            manager,
            items: Vec::new(),
            item_rects: Vec::new(),
            list_scroll: Scroll::new(),
            selected: 0,
            download_level_btn: DRectButton::new(),
            refresh_btn: DRectButton::new(),
            download_task: None,
            download_progress: None,
            current_download_id: None,
            current_download_type: None,
            toggle_btn: DRectButton::new(),
            delete_btn: DRectButton::new(),
            enter_time: 0.0,
        };
        page.refresh_items();
        Ok(page)
    }

    fn refresh_items(&mut self) {
        let manager = self.manager.lock().unwrap();
        let infos = manager.list_installed();
        self.items = infos
            .into_iter()
            .map(|info| ExtensionItem { info })
            .collect();
        if self.selected >= self.items.len() {
            self.selected = 0;
        }
    }

    fn start_download(&mut self, download_type: &str) {
        if self.download_task.is_some() {
            show_message("已有下载任务进行中").warn();
            return;
        }

        // 检测是否已有对应资源
        let asset_dir = std::env::current_dir().unwrap_or_default().join("assets");
        let target_dir = asset_dir.join("Level");
        if target_dir.exists() && target_dir.read_dir().map(|d| d.count() > 0).unwrap_or(false) {
            show_message("你的游戏是完整版，已有 Level 资源，无需下载").warn();
            return;
        }

        let manager = self.manager.clone();
        let task_id = format!("ui_{}", uuid::Uuid::new_v4().simple());
        self.current_download_id = Some(task_id.clone());
        self.current_download_type = Some(download_type.to_string());
        let filter = download_type.to_string();
        self.download_task = Some(Box::pin(async move {
            let mut mgr = manager.lock().unwrap();
            mgr.download_from_lanzou(
                DEFAULT_FOLDER_URL,
                Some(DEFAULT_FOLDER_PASSWORD),
                None,
                Some(&filter),
                Some(task_id),
            )
            .await
            .map(|ids| format!("{}", ids.len()))
        }));
        show_message(&format!("开始下载 {}...", download_type)).ok();
    }

    fn poll_download(&mut self) {
        if let Some(task) = &mut self.download_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Ok(id) => {
                        self.current_download_id = Some(id);
                        self.download_task = None;
                        self.download_progress = None;
                        show_message("扩展包下载完成").ok();
                        self.refresh_items();
                    }
                    Err(e) => {
                        show_error(anyhow::anyhow!("下载失败: {}", e));
                        self.download_task = None;
                        self.download_progress = None;
                        self.current_download_id = None;
                    }
                }
            } else if let Some(id) = &self.current_download_id {
                if let Ok(manager) = self.manager.try_lock() {
                    self.download_progress = manager.get_download_progress(id);
                }
            }
        }
    }

    fn type_label(t: ExtensionType) -> &'static str {
        match t {
            ExtensionType::Models => "模型",
            ExtensionType::Levels => "关卡",
            ExtensionType::Both => "模型+关卡",
        }
    }

    fn format_size(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl Page for ExtensionPage {
    fn label(&self) -> Cow<'static, str> {
        "扩展包".into()
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        self.enter_time = (self.enter_time + get_frame_time()).min(1.0);
        self.list_scroll.update(t);
        self.poll_download();
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if self.list_scroll.touch(touch, t) {
            return Ok(true);
        }
        for (i, rect) in self.item_rects.iter().enumerate() {
            if rect.contains(touch.position) {
                button_hit();
                self.selected = i;
                self.list_scroll.y_scroller.halt();
                return Ok(true);
            }
        }
        if self.download_level_btn.touch(touch, t) {
            button_hit();
            self.start_download("Level");
            return Ok(true);
        }
        if self.refresh_btn.touch(touch, t) {
            button_hit();
            self.refresh_items();
            show_message("已刷新").ok();
            return Ok(true);
        }
        if !self.items.is_empty() {
            if self.toggle_btn.touch(touch, t) {
                button_hit();
                let id = self.items[self.selected].info.state.id.clone();
                let mut manager = self.manager.lock().unwrap();
                match manager.toggle(&id) {
                    Ok(enabled) => {
                        show_message(if enabled { "已启用" } else { "已禁用" }).ok();
                    }
                    Err(e) => show_error(anyhow::anyhow!("操作失败: {}", e)),
                }
                drop(manager);
                self.refresh_items();
                return Ok(true);
            }
            if self.delete_btn.touch(touch, t) {
                button_hit();
                let id = self.items[self.selected].info.state.id.clone();
                let name = self.items[self.selected].info.manifest.name.clone();
                let mut manager = self.manager.lock().unwrap();
                match manager.uninstall(&id) {
                    Ok(_) => {
                        show_message(&format!("已卸载: {}", name)).ok();
                    }
                    Err(e) => show_error(anyhow::anyhow!("卸载失败: {}", e)),
                }
                drop(manager);
                self.refresh_items();
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let cr = ui.content_rect();

        // 左侧列表
        let list_w = cr.w * 0.38;
        let list_r = Rect::new(cr.x, cr.y, list_w, cr.h);
        s.render_fader(ui, |ui| {
            ui.fill_path(&list_r.rounded(0.012), semi_black(0.3));

            // 标题
            ui.text("已安装")
                .pos(list_r.x + 0.04, list_r.y + 0.03)
                .anchor(0., 0.)
                .size(0.42)
                .color(semi_white(0.9))
                .draw();

            // 数量
            ui.text(&format!("{} 个", self.items.len()))
                .pos(list_r.right() - 0.04, list_r.y + 0.035)
                .anchor(1., 0.)
                .size(0.28)
                .color(semi_white(0.5))
                .draw();

            let pad = 0.025;
            let top = list_r.y + 0.09;
            self.list_scroll.size((list_r.w, list_r.h - 0.11));
            ui.dx(list_r.x);
            ui.dy(top);
            self.item_rects.clear();
            self.list_scroll.render(ui, |ui| {
                let w = list_r.w - pad * 2.;
                let mut h = 0.;
                let item_h = 0.13;
                let gap = 0.012;

                if self.items.is_empty() {
                    ui.text("暂无扩展包")
                        .pos(list_r.w / 2., 0.1)
                        .anchor(0.5, 0.)
                        .size(0.32)
                        .color(semi_white(0.4))
                        .draw();
                    h = 0.3;
                }

                for (i, item) in self.items.iter_mut().enumerate() {
                    let y = i as f32 * (item_h + gap);
                    let is_selected = i == self.selected;
                    let item_rect = Rect::new(pad, y, w, item_h);
                    let card_r = item_rect.feather(-0.004);

                    // 背景
                    ui.fill_path(
                        &card_r.rounded(0.01),
                        if is_selected {
                            Color::new(0.15, 0.35, 0.6, 0.5)
                        } else {
                            semi_black(0.15)
                        },
                    );

                    // 选中指示条
                    if is_selected {
                        ui.fill_rect(
                            Rect::new(card_r.x, card_r.y, 0.006, card_r.h),
                            Color::new(0.3, 0.6, 1.0, 0.9),
                        );
                    }

                    // 名称
                    ui.text(&item.info.manifest.name)
                        .pos(card_r.x + 0.02, card_r.y + 0.018)
                        .anchor(0., 0.)
                        .size(0.34)
                        .max_width(card_r.w - 0.04)
                        .color(WHITE)
                        .draw();

                    // 类型标签
                    let type_text = Self::type_label(item.info.manifest.ext_type);
                    let type_color = match item.info.manifest.ext_type {
                        ExtensionType::Models => Color::new(0.3, 0.7, 1.0, 0.8),
                        ExtensionType::Levels => Color::new(1.0, 0.7, 0.3, 0.8),
                        ExtensionType::Both => Color::new(0.5, 1.0, 0.5, 0.8),
                    };
                    ui.text(type_text)
                        .pos(card_r.x + 0.02, card_r.bottom() - 0.018)
                        .anchor(0., 1.)
                        .size(0.24)
                        .color(type_color)
                        .draw();

                    // 版本
                    ui.text(&format!("v{}", item.info.manifest.version))
                        .pos(card_r.right() - 0.02, card_r.bottom() - 0.018)
                        .anchor(1., 1.)
                        .size(0.22)
                        .color(semi_white(0.5))
                        .draw();

                    // 启用状态点
                    let dot_x = card_r.right() - 0.025;
                    let dot_y = card_r.y + 0.025;
                    ui.fill_circle(
                        dot_x,
                        dot_y,
                        0.008,
                        if item.info.state.enabled {
                            Color::new(0.3, 0.9, 0.4, 1.0)
                        } else {
                            Color::new(0.5, 0.5, 0.5, 0.6)
                        },
                    );

                    // 记录全局位置用于触摸检测
                    self.item_rects.push(Rect::new(
                        list_r.x + pad,
                        top + y,
                        w,
                        item_h,
                    ));

                    h = y + item_h + gap;
                }
                (w, h)
            });
        });

        // 右侧详情
        let detail_x = list_r.right() + 0.02;
        let detail_r = Rect::new(detail_x, cr.y, cr.right() - detail_x, cr.h);
        s.render_fader(ui, |ui| {
            ui.fill_path(&detail_r.rounded(0.012), semi_black(0.3));

            if self.items.is_empty() {
                // 空状态 - 显示下载引导
                let ct = detail_r.center();
                ui.text("扩展包下载")
                    .pos(ct.x, ct.y - 0.18)
                    .anchor(0.5, 0.5)
                    .size(0.55)
                    .color(WHITE)
                    .draw();
                ui.text("从蓝奏云下载缺失的游戏资源")
                    .pos(ct.x, ct.y - 0.1)
                    .anchor(0.5, 0.5)
                    .size(0.3)
                    .color(semi_white(0.6))
                    .draw();

                // 检测完整版
                let asset_dir = std::env::current_dir().unwrap_or_default().join("assets");
                let has_level = asset_dir.join("Level").exists();

                if has_level {
                    ui.text("你的游戏是完整版，无需下载")
                        .pos(ct.x, ct.y - 0.02)
                        .anchor(0.5, 0.5)
                        .size(0.32)
                        .color(Color::new(0.4, 0.9, 0.5, 1.0))
                        .draw();
                }

                // Level 下载按钮
                let btn_w = 0.22;
                let btn_h = 0.09;
                let btn_y = ct.y + 0.02;

                let level_r = Rect::new(ct.x - btn_w / 2., btn_y, btn_w, btn_h);
                let downloading_level = self.download_task.is_some()
                    && self.current_download_type.as_deref() == Some("Level");
                let level_disabled = has_level;
                self.download_level_btn.render_shadow(ui, level_r, t, move |ui, _path| {
                    ui.fill_path(
                        &level_r.rounded(0.012),
                        if level_disabled {
                            Color::new(0.2, 0.4, 0.2, 0.6)
                        } else if downloading_level {
                            Color::new(0.2, 0.2, 0.2, 0.8)
                        } else {
                            Color::new(0.7, 0.4, 0.1, 0.85)
                        },
                    );
                    let label = if level_disabled {
                        "Level ✓"
                    } else if downloading_level {
                        "下载中..."
                    } else {
                        "下载 Level"
                    };
                    ui.text(label)
                        .pos(level_r.center().x, level_r.center().y)
                        .anchor(0.5, 0.5)
                        .size(0.32)
                        .color(if level_disabled { semi_white(0.6) } else { WHITE })
                        .draw();
                });

                // 下载进度
                if let Some(progress) = &self.download_progress {
                    if let Ok(p) = progress.try_lock() {
                        let progress_y = btn_y + btn_h + 0.04;
                        let phase_text = match &p.phase {
                            DownloadPhase::Listing => "获取文件列表...",
                            DownloadPhase::ResolvingUrl => "解析直链...",
                            DownloadPhase::Downloading => "下载中...",
                            DownloadPhase::Extracting => "解压中...",
                            DownloadPhase::Done => "完成",
                            DownloadPhase::Failed(e) => &format!("失败: {}", e),
                        };
                        ui.text(phase_text)
                            .pos(ct.x, progress_y)
                            .anchor(0.5, 0.)
                            .size(0.26)
                            .color(semi_white(0.7))
                            .draw();

                        if let Some(total) = p.total {
                            let percent = p.downloaded as f32 / total as f32;
                            let bar_r = Rect::new(ct.x - 0.2, progress_y + 0.045, 0.4, 0.022);
                            ui.fill_path(&bar_r.rounded(0.004), semi_black(0.4));
                            let fill_r = Rect::new(bar_r.x, bar_r.y, bar_r.w * percent, bar_r.h);
                            ui.fill_path(&fill_r.rounded(0.004), Color::new(0.3, 0.65, 1.0, 0.9));
                            ui.text(&format!("{:.0}%", percent * 100.0))
                                .pos(bar_r.center().x, bar_r.center().y)
                                .anchor(0.5, 0.5)
                                .size(0.2)
                                .color(WHITE)
                                .draw();
                        }
                    }
                }
            } else {
                let item = &self.items[self.selected];
                let info = &item.info;
                let pad = 0.05;

                // 标题
                ui.text(&info.manifest.name)
                    .pos(detail_r.x + pad, detail_r.y + 0.04)
                    .anchor(0., 0.)
                    .size(0.52)
                    .max_width(detail_r.w - pad * 2.)
                    .color(WHITE)
                    .draw();

                // 类型标签
                let type_text = Self::type_label(info.manifest.ext_type);
                let type_color = match info.manifest.ext_type {
                    ExtensionType::Models => Color::new(0.3, 0.7, 1.0, 0.9),
                    ExtensionType::Levels => Color::new(1.0, 0.7, 0.3, 0.9),
                    ExtensionType::Both => Color::new(0.5, 1.0, 0.5, 0.9),
                };
                let type_w = 0.12;
                let type_r = Rect::new(detail_r.x + pad, detail_r.y + 0.11, type_w, 0.04);
                ui.fill_path(&type_r.rounded(0.008), type_color);
                ui.text(type_text)
                    .pos(type_r.center().x, type_r.center().y)
                    .anchor(0.5, 0.5)
                    .size(0.24)
                    .color(BLACK)
                    .draw();

                // 版本
                ui.text(&format!("版本 v{}", info.manifest.version))
                    .pos(type_r.right() + 0.02, type_r.center().y)
                    .anchor(0., 0.5)
                    .size(0.26)
                    .color(semi_white(0.6))
                    .draw();

                // 分隔线
                let line_y = detail_r.y + 0.18;
                ui.fill_rect(
                    Rect::new(detail_r.x + pad, line_y, detail_r.w - pad * 2., 0.003),
                    semi_white(0.15),
                );

                // 详情信息
                let info_start = line_y + 0.03;
                let row_h = 0.06;
                let mut row = 0;

                let draw_row = |ui: &mut Ui, label: &str, value: &str, row: usize| {
                    let y = info_start + row as f32 * row_h;
                    ui.text(label)
                        .pos(detail_r.x + pad, y)
                        .anchor(0., 0.)
                        .size(0.26)
                        .color(semi_white(0.5))
                        .draw();
                    ui.text(value)
                        .pos(detail_r.right() - pad, y)
                        .anchor(1., 0.)
                        .size(0.28)
                        .color(WHITE)
                        .draw();
                };

                if !info.manifest.author.is_empty() {
                    draw_row(ui, "作者", &info.manifest.author, row);
                    row += 1;
                }
                if !info.manifest.description.is_empty() {
                    draw_row(ui, "描述", &info.manifest.description, row);
                    row += 1;
                }
                draw_row(
                    ui,
                    "来源",
                    if info.is_local { "本地" } else { "蓝奏云" },
                    row,
                );
                row += 1;
                if let Some(size) = info.state.file_size {
                    draw_row(ui, "大小", &Self::format_size(size), row);
                    row += 1;
                }
                draw_row(
                    ui,
                    "状态",
                    if info.state.enabled { "已启用" } else { "已禁用" },
                    row,
                );

                // 操作按钮区域
                let btn_y = detail_r.bottom() - 0.1;
                let btn_h = 0.07;
                let btn_w = (detail_r.w - pad * 2. - 0.02) / 2.;

                // 启用/禁用按钮
                let toggle_r = Rect::new(detail_r.x + pad, btn_y, btn_w, btn_h);
                let enabled = info.state.enabled;
                self.toggle_btn.render_shadow(ui, toggle_r, t, move |ui, _path| {
                    ui.fill_path(
                        &toggle_r.rounded(0.012),
                        if enabled {
                            Color::new(0.6, 0.25, 0.15, 0.8)
                        } else {
                            Color::new(0.15, 0.5, 0.25, 0.8)
                        },
                    );
                    ui.text(if enabled { "禁用" } else { "启用" })
                        .pos(toggle_r.center().x, toggle_r.center().y)
                        .anchor(0.5, 0.5)
                        .size(0.32)
                        .color(WHITE)
                        .draw();
                });

                // 卸载按钮
                let delete_r = Rect::new(toggle_r.right() + 0.02, btn_y, btn_w, btn_h);
                self.delete_btn.render_shadow(ui, delete_r, t, |ui, _path| {
                    ui.fill_path(
                        &delete_r.rounded(0.012),
                        Color::new(0.55, 0.15, 0.15, 0.8),
                    );
                    ui.text("卸载")
                        .pos(delete_r.center().x, delete_r.center().y)
                        .anchor(0.5, 0.5)
                        .size(0.32)
                        .color(WHITE)
                        .draw();
                });

                // 刷新按钮（右上角小按钮）
                let refresh_size = 0.05;
                let refresh_r = Rect::new(
                    detail_r.right() - pad - refresh_size,
                    detail_r.y + 0.04,
                    refresh_size,
                    refresh_size,
                );
                self.refresh_btn.render_shadow(ui, refresh_r, t, |ui, _path| {
                    ui.fill_path(
                        &refresh_r.rounded(0.008),
                        semi_black(0.2),
                    );
                    ui.text("↻")
                        .pos(refresh_r.center().x, refresh_r.center().y)
                        .anchor(0.5, 0.5)
                        .size(0.36)
                        .color(semi_white(0.8))
                        .draw();
                });
            }
        });

        Ok(())
    }
}
