prpr_l10n::tl_file!("data");

use super::{request_export_filtered, take_export, NextPage, Page, SharedState};
use crate::data_transfer::{
    self,
    lan::{self, RecvPhase, SendPhase},
    Category, Progress, CATEGORIES,
};
use anyhow::{anyhow, Result};
use inputbox::InputBox;
use macroquad::prelude::*;
use prpr::{
    core::BOLD_FONT,
    ext::{semi_black, semi_white, RectExt},
    scene::{request_file, request_input, return_file, return_input, show_error, show_message, take_file, take_input},
    ui::{Dialog, DRectButton, Scroll, Ui},
};
use std::{
    borrow::Cow,
    net::SocketAddr,
    sync::{mpsc, Arc},
};

const ROW_H: f32 = 0.135;
const PACKAGE_EXT: &str = "pfd";
const IMPORT_FILE_ID: &str = "_import_data";
const PAIR_INPUT_ID: &str = "data_pair_code";

fn category_title(category: Category) -> Cow<'static, str> {
    match category {
        Category::Account => tl!("category-account"),
        Category::Settings => tl!("category-settings"),
        Category::Charts => tl!("category-charts"),
        Category::ResPack => tl!("category-respack"),
    }
}

fn category_subtitle(category: Category) -> Cow<'static, str> {
    match category {
        Category::Account => tl!("category-account-sub"),
        Category::Settings => tl!("category-settings-sub"),
        Category::Charts => tl!("category-charts-sub"),
        Category::ResPack => tl!("category-respack-sub"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Main,
    Select,
    Exporting,
    Importing,
    Transfer,
    SendSelect,
    Waiting,
    Receiving,
    ReceiveProgress,
}

pub struct DataManagePage {
    step: Step,
    export_card: DRectButton,
    import_card: DRectButton,
    transfer_card: DRectButton,
    selected: [bool; CATEGORIES.len()],
    check_btns: [DRectButton; CATEGORIES.len()],
    start_btn: DRectButton,
    conflict_btn: DRectButton,
    scroll: Scroll,
    job: Option<mpsc::Receiver<Result<()>>>,
    progress: Arc<Progress>,
    pending_data: Option<Vec<u8>>,
    start_time: f32,
    finish_time: f32,
    send_card: DRectButton,
    receive_card: DRectButton,
    start_transfer_btn: DRectButton,
    device_btns: Vec<DRectButton>,
    send: Option<lan::SendSession>,
    receive: Option<lan::ReceiveSession>,
    pair_target: Option<SocketAddr>,
    restart_dialog_shown: bool,
    send_done: bool,
    overwrite: bool,
    next_page: Option<NextPage>,
}

impl DataManagePage {
    pub fn new() -> Self {
        Self {
            step: Step::Main,
            export_card: DRectButton::new(),
            import_card: DRectButton::new(),
            transfer_card: DRectButton::new(),
            selected: [true; CATEGORIES.len()],
            check_btns: [(); CATEGORIES.len()].map(|_| DRectButton::new()),
            start_btn: DRectButton::new(),
            conflict_btn: DRectButton::new(),
            scroll: Scroll::new(),
            job: None,
            progress: Arc::new(Progress::default()),
            pending_data: None,
            start_time: 0.,
            finish_time: f32::NAN,
            send_card: DRectButton::new(),
            receive_card: DRectButton::new(),
            start_transfer_btn: DRectButton::new(),
            device_btns: Vec::new(),
            send: None,
            receive: None,
            pair_target: None,
            restart_dialog_shown: false,
            send_done: false,
            overwrite: true,
            next_page: None,
        }
    }

    fn frame(ui: &mut Ui) -> Rect {
        let mut panel = ui.content_rect().feather(-0.04);
        panel.w += panel.x - (-0.92);
        panel.x = -0.92;
        ui.fill_path(&panel.rounded(0.01), semi_black(0.45));
        panel.feather(-0.035)
    }

    fn render_card(
        ui: &mut Ui,
        rect: Rect,
        title: Cow<'static, str>,
        desc: Cow<'static, str>,
        icon: &str,
        enabled: bool,
    ) {
        let center = rect.center();
        let icon_rect = Rect::new(center.x - 0.075, rect.y + rect.h * 0.16, 0.15, 0.15);
        ui.fill_path(
            &icon_rect.rounded(0.045),
            semi_white(if enabled { 0.14 } else { 0.06 }),
        );
        ui.text(icon)
            .pos(icon_rect.center().x, icon_rect.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.56)
            .color(if enabled { WHITE } else { semi_white(0.35) })
            .draw();
        ui.text(title)
            .pos(center.x, rect.y + rect.h * 0.5)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.5)
            .color(if enabled { WHITE } else { semi_white(0.45) })
            .draw_using(&BOLD_FONT);
        ui.text(desc)
            .pos(center.x, rect.y + rect.h * 0.6)
            .anchor(0.5, 0.)
            .multiline()
            .max_width(rect.w - 0.10)
            .h_center()
            .size(0.28)
            .color(semi_white(if enabled { 0.6 } else { 0.35 }))
            .draw();
    }

    fn render_main(&mut self, ui: &mut Ui, rect: Rect, t: f32) {
        let gap = 0.04;
        let card_w = (rect.w - gap) / 2.;
        let card_h = (rect.h * 0.62).min(card_w * 1.15);
        let card_y = rect.y + (rect.h - card_h) * 0.42;
        let left = Rect::new(rect.x, card_y, card_w, card_h);
        let right = Rect::new(rect.x + card_w + gap, card_y, card_w, card_h);
        self.export_card.render_shadow(ui, left, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.35));
            Self::render_card(ui, left, tl!("export-card-title"), tl!("export-card-desc"), "\u{21E9}", true);
        });
        self.import_card.render_shadow(ui, right, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.35));
            Self::render_card(ui, right, tl!("import-card-title"), tl!("import-card-desc"), "\u{21E7}", true);
        });

        // 跨设备传输入口：整行卡片
        let row = Rect::new(rect.x, left.bottom() + 0.05, rect.w, 0.15);
        self.transfer_card.render_shadow(ui, row, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.35));
            ui.text("\u{21C4}")
                .pos(row.x + 0.09, row.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.56)
                .color(WHITE)
                .draw();
            ui.text(tl!("transfer-card-title"))
                .pos(row.x + 0.19, row.center().y - 0.022)
                .anchor(0., 0.5)
                .no_baseline()
                .size(0.42)
                .color(WHITE)
                .draw_using(&BOLD_FONT);
            ui.text(tl!("transfer-card-desc"))
                .pos(row.x + 0.19, row.center().y + 0.036)
                .anchor(0., 0.5)
                .no_baseline()
                .size(0.26)
                .max_width(row.w - 0.25)
                .color(semi_white(0.6))
                .draw();
        });
    }

    fn render_select(&mut self, ui: &mut Ui, inner: Rect, t: f32, next_label: Cow<'static, str>, with_conflict: bool) {
        let list_h = (inner.h - if with_conflict { 0.26 } else { 0.14 }).max(0.1);
        let list_rect = Rect::new(inner.x, inner.y, inner.w, list_h);
        self.scroll.size((inner.w, list_h));
        ui.scissor(list_rect, |ui| {
            ui.scope(|ui| {
                ui.dx(inner.x);
                ui.dy(inner.y);
                let w = inner.w;
                self.scroll.render(ui, |ui| {
                    let mut y = 0.;
                    for (i, category) in CATEGORIES.iter().enumerate() {
                        let row = Rect::new(0., y, w, ROW_H);
                        let checked = self.selected[i];
                        self.check_btns[i].render_shadow(ui, row, t, |ui, path| {
                            ui.fill_path(&path, semi_black(if checked { 0.42 } else { 0.28 }));
                            let cb_size = 0.062;
                            let cb = Rect::new(
                                row.right() - 0.035 - cb_size,
                                row.center().y - cb_size / 2.,
                                cb_size,
                                cb_size,
                            );
                            if checked {
                                ui.fill_path(&cb.rounded(0.014), semi_white(0.85));
                                ui.text("\u{2713}")
                                    .pos(cb.center().x, cb.center().y)
                                    .anchor(0.5, 0.5)
                                    .no_baseline()
                                    .size(0.42)
                                    .color(Color::new(0.15, 0.15, 0.15, 1.))
                                    .draw();
                            } else {
                                ui.stroke_path(&cb.rounded(0.014), 0.0028, semi_white(0.35));
                            }
                            ui.text(category_title(*category))
                                .pos(row.x + 0.035, row.center().y - 0.024)
                                .anchor(0., 0.5)
                                .no_baseline()
                                .size(0.4)
                                .max_width(row.w - 0.16)
                                .color(if checked { WHITE } else { semi_white(0.7) })
                                .draw();
                            ui.text(category_subtitle(*category))
                                .pos(row.x + 0.035, row.center().y + 0.036)
                                .anchor(0., 0.5)
                                .no_baseline()
                                .size(0.24)
                                .max_width(row.w - 0.16)
                                .color(semi_white(if checked { 0.55 } else { 0.4 }))
                                .draw();
                        });
                        y += ROW_H;
                    }
                    (w, y.max(list_h))
                });
            });
        });

        let btn = Rect::new(inner.x + 0.06, inner.bottom() - 0.095, inner.w - 0.12, 0.08);
        let any = self.selected.iter().any(|it| *it);
        if with_conflict {
            let row = Rect::new(inner.x + 0.02, btn.y - 0.095, inner.w - 0.04, 0.075);
            let overwrite = self.overwrite;
            self.conflict_btn.render_shadow(ui, row, t, |ui, path| {
                ui.fill_path(&path, semi_black(0.35));
                ui.text(tl!("conflict-title"))
                    .pos(row.x + 0.035, row.center().y - 0.013)
                    .anchor(0., 0.5)
                    .no_baseline()
                    .size(0.32)
                    .color(WHITE)
                    .draw();
                ui.text(tl!("conflict-sub"))
                    .pos(row.x + 0.035, row.center().y + 0.025)
                    .anchor(0., 0.5)
                    .no_baseline()
                    .size(0.22)
                    .color(semi_white(0.5))
                    .draw();
                let cb_size = 0.05;
                let cb = Rect::new(row.right() - 0.035 - cb_size, row.center().y - cb_size / 2., cb_size, cb_size);
                if overwrite {
                    ui.fill_path(&cb.rounded(0.012), semi_white(0.85));
                    ui.text("\u{2713}")
                        .pos(cb.center().x, cb.center().y)
                        .anchor(0.5, 0.5)
                        .no_baseline()
                        .size(0.34)
                        .color(Color::new(0.15, 0.15, 0.15, 1.))
                        .draw();
                } else {
                    ui.stroke_path(&cb.rounded(0.012), 0.0025, semi_white(0.35));
                }
                ui.text(if overwrite {
                    tl!("conflict-overwrite")
                } else {
                    tl!("conflict-ignore")
                })
                .pos(cb.x - 0.015, row.center().y)
                .anchor(1., 0.5)
                .no_baseline()
                .size(0.3)
                .color(if overwrite { WHITE } else { semi_white(0.8) })
                .draw();
            });
        }
        self.start_btn.render_text(ui, btn, t, next_label, 0.46, any);
    }

    fn render_progress_value(&self, ui: &mut Ui, rect: Rect, t: f32, p: f32, status: Cow<'static, str>) {
        let center = rect.center();
        ui.text(status)
            .pos(center.x, rect.y + rect.h * 0.30)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.8)
            .color(WHITE)
            .draw_using(&BOLD_FONT);
        ui.text(tl!("keep-open"))
            .pos(center.x, rect.y + rect.h * 0.46)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.42)
            .color(semi_white(0.75))
            .draw();
        let bar_w = (rect.w * 0.62).min(1.2);
        let bar_h = 0.022;
        let bar = Rect::new(center.x - bar_w / 2., rect.y + rect.h * 0.68, bar_w, bar_h);
        ui.fill_path(&bar.rounded(bar_h / 2.), semi_white(0.15));
        if p > 0. {
            ui.fill_path(&Rect::new(bar.x, bar.y, (bar.w * p).max(bar_h), bar_h).rounded(bar_h / 2.), semi_white(0.85));
        }
        ui.text(format!("{:.0}%", p * 100.))
            .pos(center.x, bar.bottom() + 0.085)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.42)
            .color(WHITE)
            .draw();
        let eta = if p >= 1. {
            Some(0.)
        } else if p > 0.005 {
            Some((t - self.start_time).max(0.) * (1. - p) / p)
        } else {
            None
        };
        let eta_text = match eta {
            Some(eta) => tl!("export-eta", "time" => format_duration(eta)),
            None => tl!("export-eta-calc").into_owned(),
        };
        ui.text(eta_text)
            .pos(center.x, bar.bottom() + 0.155)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.28)
            .color(semi_white(0.55))
            .draw();
    }

    fn render_progress(&self, ui: &mut Ui, rect: Rect, t: f32, status: Cow<'static, str>) {
        self.render_progress_value(ui, rect, t, self.progress.fraction(), status);
    }

    fn render_transfer(&mut self, ui: &mut Ui, rect: Rect, t: f32) {
        let gap = 0.04;
        let card_w = (rect.w - gap) / 2.;
        let card_h = (rect.h * 0.62).min(card_w * 1.15);
        let card_y = rect.y + (rect.h - card_h) * 0.42;
        let left = Rect::new(rect.x, card_y, card_w, card_h);
        let right = Rect::new(rect.x + card_w + gap, card_y, card_w, card_h);
        self.send_card.render_shadow(ui, left, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.35));
            Self::render_card(ui, left, tl!("transfer-send"), tl!("transfer-send-desc"), "\u{2191}", true);
        });
        self.receive_card.render_shadow(ui, right, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.35));
            Self::render_card(ui, right, tl!("transfer-receive"), tl!("transfer-receive-desc"), "\u{2193}", true);
        });
    }

    fn render_waiting(&mut self, ui: &mut Ui, rect: Rect, t: f32) {
        let Some(send) = &self.send else {
            return;
        };
        let phase = send.phase;
        let frac = send.progress.fraction();
        let code = send.code.clone();
        let peer = send.peer.clone();
        let can_begin = send.can_begin();

        if phase == SendPhase::Preparing {
            self.render_progress_value(ui, rect, t, frac, tl!("waiting-preparing"));
            return;
        }
        let center = rect.center();
        if phase == SendPhase::Transferring || phase == SendPhase::Finished {
            let status = if phase == SendPhase::Finished {
                tl!("transfer-done")
            } else {
                tl!("transferring")
            };
            self.render_progress_value(ui, rect, t, frac, status);
            return;
        }

        ui.text(tl!("waiting-code"))
            .pos(center.x, rect.y + rect.h * 0.16)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.36)
            .color(semi_white(0.7))
            .draw();
        ui.text(code.unwrap_or_default())
            .pos(center.x, rect.y + rect.h * 0.30)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(1.4)
            .color(WHITE)
            .draw_using(&BOLD_FONT);
        ui.text(tl!("waiting-hint"))
            .pos(center.x, rect.y + rect.h * 0.42)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.3)
            .color(semi_white(0.55))
            .draw();

        let connected = Rect::new(rect.x + 0.05, rect.y + rect.h * 0.52, rect.w - 0.1, 0.16);
        ui.fill_path(&connected.rounded(0.02), semi_black(0.25));
        ui.text(tl!("waiting-connected"))
            .pos(connected.x + 0.03, connected.center().y - 0.025)
            .anchor(0., 0.5)
            .no_baseline()
            .size(0.3)
            .color(semi_white(0.6))
            .draw();
        match &peer {
            Some(peer) => {
                ui.text(peer.as_str())
                    .pos(connected.x + 0.03, connected.center().y + 0.035)
                    .anchor(0., 0.5)
                    .no_baseline()
                    .size(0.4)
                    .max_width(connected.w - 0.06)
                    .color(WHITE)
                    .draw();
            }
            None => {
                ui.text(tl!("waiting-none"))
                    .pos(connected.x + 0.03, connected.center().y + 0.035)
                    .anchor(0., 0.5)
                    .no_baseline()
                    .size(0.34)
                    .color(semi_white(0.45))
                    .draw();
            }
        }

        if peer.is_some() {
            let btn = Rect::new(center.x - 0.3, rect.bottom() - 0.12, 0.6, 0.085);
            self.start_transfer_btn
                .render_text(ui, btn, t, tl!("transfer-start"), 0.46, can_begin);
        }
    }

    fn render_receiving(&mut self, ui: &mut Ui, inner: Rect, t: f32) {
        let devices = self.receive.as_ref().map(|it| it.devices.clone()).unwrap_or_default();
        if devices.is_empty() {
            ui.text(tl!("receiving-searching"))
                .pos(inner.center().x, inner.y + 0.25)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.36)
                .color(semi_white(0.6))
                .draw();
            return;
        }
        while self.device_btns.len() < devices.len() {
            self.device_btns.push(DRectButton::new());
        }
        self.scroll.size((inner.w, inner.h));
        ui.scissor(inner, |ui| {
            ui.scope(|ui| {
                ui.dx(inner.x);
                ui.dy(inner.y);
                let w = inner.w;
                self.scroll.render(ui, |ui| {
                    let mut y = 0.;
                    for (device, btn) in devices.iter().zip(self.device_btns.iter_mut()) {
                        let row = Rect::new(0., y, w, ROW_H);
                        btn.render_shadow(ui, row, t, |ui, path| {
                            ui.fill_path(&path, semi_black(0.4));
                            ui.text(device.name.as_str())
                                .pos(row.x + 0.035, row.center().y - 0.024)
                                .anchor(0., 0.5)
                                .no_baseline()
                                .size(0.4)
                                .max_width(row.w - 0.1)
                                .color(WHITE)
                                .draw();
                            ui.text(device.ip().to_string())
                                .pos(row.x + 0.035, row.center().y + 0.036)
                                .anchor(0., 0.5)
                                .no_baseline()
                                .size(0.26)
                                .color(semi_white(0.55))
                                .draw();
                            ui.text("\u{203A}")
                                .pos(row.right() - 0.05, row.center().y)
                                .anchor(1., 0.5)
                                .no_baseline()
                                .size(0.46)
                                .color(WHITE)
                                .draw();
                        });
                        y += ROW_H;
                    }
                    (w, y.max(inner.h))
                });
            });
        });
    }

    fn render_receive_progress(&mut self, ui: &mut Ui, rect: Rect, t: f32) {
        let Some(receive) = &self.receive else {
            return;
        };
        let phase = receive.phase;
        let frac = receive.progress.fraction();
        let status = match phase {
            RecvPhase::WaitingStart => tl!("waiting-sender-start"),
            RecvPhase::Downloading => tl!("downloading"),
            RecvPhase::Importing => tl!("importing"),
            RecvPhase::Done => tl!("import-done-title"),
            RecvPhase::Discovering => tl!("receiving-searching"),
        };
        if phase == RecvPhase::WaitingStart {
            ui.text(status)
                .pos(rect.center().x, rect.y + rect.h * 0.35)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.56)
                .color(WHITE)
                .draw_using(&BOLD_FONT);
            return;
        }
        self.render_progress_value(ui, rect, t, frac, status);
    }

    fn begin_send(&mut self) -> Result<()> {
        let conflict = if self.overwrite {
            data_transfer::Conflict::Overwrite
        } else {
            data_transfer::Conflict::Ignore
        };
        self.send = Some(lan::SendSession::new(&self.selected, conflict)?);
        self.send_done = false;
        self.step = Step::Waiting;
        Ok(())
    }

    fn begin_receive(&mut self) -> Result<()> {
        self.receive = Some(lan::ReceiveSession::new()?);
        self.device_btns.clear();
        self.restart_dialog_shown = false;
        self.step = Step::Receiving;
        Ok(())
    }

    fn show_restart_dialog(&mut self) {
        Dialog::plain(tl!("import-done-title"), tl!("import-done"))
            .buttons(vec![tl!("import-done-ok").into_owned()])
            .listener(|_, _| {
                // 立即重启：先落盘再拉起新进程，避免导入的内存数据丢失
                let _ = crate::save_data();
                match std::env::current_exe().and_then(|exe| std::process::Command::new(exe).spawn()) {
                    Ok(_) => std::process::exit(0),
                    Err(err) => {
                        tracing::warn!(?err, "failed to restart the game");
                        show_message(tl!("import-restart-failed")).error();
                        false
                    }
                }
            })
            .show();
    }

    fn begin_export(&mut self, config: crate::page::ExportConfig, t: f32) -> Result<()> {
        let entries = match data_transfer::collect_entries(&self.selected, data_transfer::Conflict::Overwrite) {
            Ok(entries) => entries,
            Err(err) => {
                if let Err(err) = (config.deleter)() {
                    tracing::warn!(?err, "failed to delete export file");
                }
                return Err(err);
            }
        };
        let total = entries.iter().map(|it| it.size()).sum::<u64>();
        let progress = Arc::new(Progress::new(total));
        let thread_progress = Arc::clone(&progress);
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let res = data_transfer::write_zip(entries, config.file, &thread_progress);
            if res.is_err() {
                if let Err(err) = (config.deleter)() {
                    tracing::warn!(?err, "failed to delete export file");
                }
            }
            let _ = tx.send(res);
        });
        self.progress = progress;
        self.job = Some(rx);
        self.step = Step::Exporting;
        self.start_time = t;
        self.finish_time = f32::NAN;
        Ok(())
    }

    fn finish_export(&mut self) {
        if self.next_page.is_some() {
            return;
        }
        self.finish_time = f32::NAN;
        show_message(tl!("export-success")).ok();
        self.next_page = Some(NextPage::Pop);
    }

    fn begin_import(&mut self, file: String, t: f32) -> Result<()> {
        let plan = data_transfer::prepare_import(&file)?;
        let progress = Arc::new(Progress::new(plan.total));
        let thread_progress = Arc::clone(&progress);
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending_data = plan.data_json.clone();
        std::thread::spawn(move || {
            let res = data_transfer::run_import(file, plan, &thread_progress);
            let _ = tx.send(res);
        });
        self.progress = progress;
        self.job = Some(rx);
        self.step = Step::Importing;
        self.start_time = t;
        self.finish_time = f32::NAN;
        Ok(())
    }

    fn finish_import(&mut self) {
        self.finish_time = f32::NAN;
        self.step = Step::Main;
        // 同步到内存：避免之后任何一次 save_data() 用旧数据覆盖刚导入的 data.json。
        if let Some(bytes) = self.pending_data.take() {
            if let Err(err) = data_transfer::apply_imported_data(&bytes) {
                tracing::warn!(?err, "failed to apply imported data in memory");
            }
        }
        self.show_restart_dialog();
    }
}

impl Page for DataManagePage {
    fn label(&self) -> Cow<'static, str> {
        match self.step {
            Step::Main => tl!("page-title"),
            Step::Select => tl!("export-page-title"),
            Step::Exporting => tl!("export-title"),
            Step::Importing => tl!("import-title"),
            Step::Transfer => tl!("transfer-title"),
            Step::SendSelect => tl!("export-page-title"),
            Step::Waiting => tl!("waiting-page-title"),
            Step::Receiving => tl!("receiving-title"),
            Step::ReceiveProgress => tl!("transfer-title"),
        }
    }

    fn exit(&mut self) -> Result<()> {
        // 离开页面时结束后台传输线程
        self.send = None;
        self.receive = None;
        Ok(())
    }

    fn on_back_pressed(&mut self, _s: &mut SharedState) -> bool {
        match self.step {
            Step::Main => false,
            Step::Select | Step::Transfer => {
                self.step = Step::Main;
                true
            }
            Step::SendSelect | Step::Waiting | Step::Receiving | Step::ReceiveProgress => {
                self.send = None;
                self.receive = None;
                self.step = Step::Transfer;
                true
            }
            Step::Exporting | Step::Importing => true,
        }
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        match self.step {
            Step::Main => {
                if self.export_card.touch(touch, t) {
                    self.step = Step::Select;
                    return Ok(true);
                }
                if self.import_card.touch(touch, t) {
                    request_file(IMPORT_FILE_ID);
                    return Ok(true);
                }
                if self.transfer_card.touch(touch, t) {
                    self.step = Step::Transfer;
                    return Ok(true);
                }
            }
            Step::Transfer => {
                if self.send_card.touch(touch, t) {
                    self.step = Step::SendSelect;
                    return Ok(true);
                }
                if self.receive_card.touch(touch, t) {
                    if let Err(err) = self.begin_receive() {
                        show_error(err.context(tl!("receive-failed")));
                    }
                    return Ok(true);
                }
            }
            Step::Select | Step::SendSelect => {
                if self.scroll.touch(touch, t) {
                    return Ok(true);
                }
                for i in 0..CATEGORIES.len() {
                    if self.check_btns[i].touch(touch, t) {
                        self.selected[i] ^= true;
                        return Ok(true);
                    }
                }
                if self.start_btn.touch(touch, t) {
                    if !self.selected.iter().any(|it| *it) {
                        show_message(tl!("export-none-selected")).warn();
                        return Ok(true);
                    }
                    if self.step == Step::Select {
                        // 数据包只允许 .pfd：保存对话框的「保存类型」只列 pfd，
                        // 否则会沿用谱面导出的 pez 过滤器把文件名写成 data.pfd.pez
                        request_export_filtered(
                            format!("data.{}", PACKAGE_EXT),
                            vec![(tl!("filter-pfd").to_string(), vec![PACKAGE_EXT.to_string()])],
                        );
                    } else if let Err(err) = self.begin_send() {
                        show_error(err.context(tl!("send-failed")));
                    }
                    return Ok(true);
                }
                if self.step == Step::SendSelect && self.conflict_btn.touch(touch, t) {
                    self.overwrite ^= true;
                    return Ok(true);
                }
            }
            Step::Waiting => {
                if self.start_transfer_btn.touch(touch, t) {
                    if let Some(send) = &mut self.send {
                        send.begin();
                    }
                    return Ok(true);
                }
            }
            Step::Receiving => {
                if self.scroll.touch(touch, t) {
                    return Ok(true);
                }
                let devices = self.receive.as_ref().map(|it| it.devices.clone()).unwrap_or_default();
                for (i, btn) in self.device_btns.iter_mut().enumerate() {
                    if btn.touch(touch, t) {
                        if let Some(device) = devices.get(i) {
                            self.pair_target = Some(device.addr);
                            request_input(
                                PAIR_INPUT_ID,
                                InputBox::new().title(tl!("pair-code-title")).prompt(tl!("pair-code-prompt")),
                            );
                        }
                        return Ok(true);
                    }
                }
            }
            Step::Exporting | Step::Importing | Step::ReceiveProgress => return Ok(true),
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        if let Some((id, file)) = take_file() {
            if id == IMPORT_FILE_ID {
                if let Err(err) = self.begin_import(file, t) {
                    show_error(err);
                }
            } else {
                return_file(id, file);
            }
        }
        if let Some((id, text)) = take_input() {
            if id == PAIR_INPUT_ID {
                let addr = self.pair_target.take();
                if let Some(addr) = addr {
                    let mut paired = false;
                    if let Some(receive) = &mut self.receive {
                        match receive.pair(addr, text) {
                            Ok(()) => paired = true,
                            Err(err) => show_error(err.context(tl!("pair-failed"))),
                        }
                    }
                    if paired {
                        self.step = Step::ReceiveProgress;
                    }
                }
            } else {
                return_input(id, text);
            }
        }

        if self.step == Step::Select || self.step == Step::SendSelect || self.step == Step::Receiving {
            self.scroll.update(t);
        }

        if self.step == Step::Select {
            if let Some(config) = take_export() {
                match config {
                    Ok(config) => self.begin_export(config, t)?,
                    Err(err) => show_error(err.into()),
                }
            }
        }

        if self.step == Step::Waiting {
            let mut finished = false;
            if let Some(send) = &mut self.send {
                send.update();
                if let Some(err) = send.error.take() {
                    show_message(err).error();
                }
                finished = send.phase == SendPhase::Finished;
            }
            if finished && !self.send_done {
                self.send_done = true;
                show_message(tl!("transfer-done")).ok();
                self.next_page = Some(NextPage::Pop);
            }
        }

        let mut receive_done = false;
        if matches!(self.step, Step::Receiving | Step::ReceiveProgress) {
            if let Some(receive) = &mut self.receive {
                receive.update();
                if let Some(err) = receive.take_error() {
                    show_message(err).error();
                }
                receive_done = receive.phase == RecvPhase::Done;
            }
        }
        if receive_done && !self.restart_dialog_shown {
            self.restart_dialog_shown = true;
            self.step = Step::Main;
            self.send = None;
            self.receive = None;
            self.show_restart_dialog();
        }

        if matches!(self.step, Step::Exporting | Step::Importing) {
            let result = self.job.as_mut().and_then(|job| match job.try_recv() {
                Ok(res) => Some(res),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(anyhow!("Data transfer thread panicked"))),
            });
            if let Some(res) = result {
                let was_export = self.step == Step::Exporting;
                self.job = None;
                match res {
                    Ok(()) => self.finish_time = t,
                    Err(err) => {
                        show_error(err);
                        self.finish_time = f32::NAN;
                        self.step = if was_export { Step::Select } else { Step::Main };
                    }
                }
            }
            if !self.finish_time.is_nan() && t - self.finish_time > 0.25 {
                if self.step == Step::Exporting {
                    self.finish_export();
                } else {
                    self.finish_import();
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        s.render_fader(ui, |ui| {
            let inner = Self::frame(ui);
            match self.step {
                Step::Main => self.render_main(ui, inner, t),
                Step::Select => self.render_select(ui, inner, t, tl!("export-start"), false),
                Step::SendSelect => self.render_select(ui, inner, t, tl!("transfer-next"), true),
                Step::Exporting => self.render_progress(ui, inner, t, tl!("exporting")),
                Step::Importing => self.render_progress(ui, inner, t, tl!("importing")),
                Step::Transfer => self.render_transfer(ui, inner, t),
                Step::Waiting => self.render_waiting(ui, inner, t),
                Step::Receiving => self.render_receiving(ui, inner, t),
                Step::ReceiveProgress => self.render_receive_progress(ui, inner, t),
            }
        });
        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        self.next_page.take().unwrap_or_default()
    }
}

fn format_duration(secs: f32) -> String {
    let secs = secs.max(0.).round() as u64;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}
