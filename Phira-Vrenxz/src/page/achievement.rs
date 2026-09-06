//! 成就页面
//!
//! UI 模仿谱面库（LibraryPage）：
//! 左侧为竖排分类 tab 栏（Tabs 组件），右侧为可滚动的成就卡片网格。

prpr_l10n::tl_file!("achievement");

use super::{Fader, NextPage, Page, SharedState};
use crate::{
    achievement::{AchievementCategory, AchievementDef, AchievementManager, ALL_ACHIEVEMENTS},
    tabs::{Tabs, TitleFn},
};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{semi_black, semi_white, RectExt, ScaleType},
    ui::{DRectButton, Scroll, Ui, PREFER_REDUCED_MOTION},
};
use std::borrow::Cow;
use std::sync::{
    atomic::Ordering,
    Arc, Mutex,
};

const CARD_PADDING: f32 = 0.016;
const CARD_CORNER_RADIUS: f32 = 0.025;
const ROW_NUM: u32 = 4;
const ROW_HEIGHT: f32 = 0.3;

/// 成就分类筛选
#[derive(Clone, Copy, PartialEq, Eq)]
enum AchievementFilter {
    All,
    Category(AchievementCategory),
}

struct AchievementCard {
    def: &'static AchievementDef,
    btn: DRectButton,
    current: u64,
    unlocked: bool,
    unlocked_at: Option<i64>,
}

/// 单个分类的成就列表（模仿谱面库 ChartsView）
struct AchievementList {
    cards: Vec<AchievementCard>,
    scroll: Scroll,
    fader: Fader,
}

impl AchievementList {
    fn new(filter: AchievementFilter) -> Self {
        let manager = crate::achievement::manager();
        let mgr = manager.lock().unwrap();
        // 按定义的显示顺序排列（1-20，对应 achievements_icon/1..20.png，即用户指定顺序）
        let mut defs: Vec<&'static AchievementDef> = match filter {
            AchievementFilter::All => ALL_ACHIEVEMENTS.to_vec(),
            AchievementFilter::Category(cat) => {
                ALL_ACHIEVEMENTS.iter().filter(|a| a.category == cat).copied().collect()
            }
        };
        defs.sort_by_key(|d| d.order);
        let cards = defs
            .into_iter()
            .map(|def| {
                let prog = mgr.progress(def.id);
                AchievementCard {
                    def,
                    btn: DRectButton::new(),
                    current: prog.current,
                    unlocked: prog.unlocked,
                    unlocked_at: prog.unlocked_at,
                }
            })
            .collect::<Vec<_>>();
        drop(mgr);
        Self {
            cards,
            scroll: Scroll::new(),
            fader: Fader::new().with_distance(0.06),
        }
    }

    /// 触摸处理：返回 (是否消费, 点击的卡片索引)
    fn touch(&mut self, touch: &Touch, t: f32) -> (bool, Option<usize>) {
        // 滚动
        if self.scroll.touch(touch, t) {
            return (true, None);
        }
        // 卡片点击：使用按钮的"按下后松开"判定（Ended 触发），
        // 避免一次点击的 Started/Ended 各触发一次（连点），也避免滚动时误开弹窗
        for (i, card) in self.cards.iter_mut().enumerate() {
            if card.btn.touch(touch, t) {
                return (true, Some(i));
            }
        }
        (false, None)
    }

    fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) {
        let cw = r.w / ROW_NUM as f32;
        let ch = ROW_HEIGHT;
        let p = CARD_PADDING;
        let cell = Rect::new(p, p, cw - p * 2., ch - p * 2.);
        self.scroll.size((r.w, r.h));
        ui.scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            self.scroll.render(ui, |ui| {
                self.fader.reset();
                self.fader.for_sub(|f| {
                    ui.hgrids(r.w, ch, ROW_NUM, self.cards.len() as u32, |ui, id| {
                        f.render(ui, t, |ui| {
                            Self::render_card_item(ui, id, cell, t, &mut self.cards);
                        });
                    });
                });
                (r.w, ((self.cards.len() + ROW_NUM as usize - 1) / ROW_NUM as usize) as f32 * ch)
            });
        });
    }

    /// 渲染成就卡片（UI 模仿谱面库 ChartsView 的卡片：
    /// 稀有度渐变背景 + 底部压暗、右上角深色稀有度徽章、左下角加粗名称、底部进度条）
    fn render_card_item(ui: &mut Ui, id: u32, r: Rect, t: f32, cards: &mut [AchievementCard]) {
        let Some(card) = cards.get_mut(id as usize) else {
            return;
        };
        let def = card.def;
        let (r_c, g_c, b_c) = def.rarity.color();
        let card_r = r.feather(-0.005);
        let card_path = card_r.rounded(CARD_CORNER_RADIUS);

        card.btn.render_shadow(ui, r, t, |ui, _path| {
            // 背景：已解锁用稀有度色渐变（模仿谱面库插图背景），未解锁深灰
            if card.unlocked {
                ui.fill_path(
                    &card_path,
                    (
                        Color::new(r_c * 0.5, g_c * 0.5, b_c * 0.5, 0.95),
                        (0., 0.),
                        Color::new(r_c * 0.16, g_c * 0.16, b_c * 0.16, 0.98),
                        (0., card_r.h),
                    ),
                );
            } else {
                ui.fill_path(&card_path, semi_black(0.55));
            }

            // 底部压暗渐变（模仿谱面库）
            ui.fill_path(
                &card_path,
                (
                    semi_black(0.0),
                    (0., 0.),
                    semi_black(0.75),
                    (0., card_r.h * 0.65),
                ),
            );

            // 大号图标居中（自定义图标，按原图宽高比渲染不拉伸；未加载时回退 emoji）
            let icon_center_y = card_r.center().y - card_r.h * 0.03;
            if let Some(tex) = crate::achievement::achievement_icon(def) {
                let base_h = if card.unlocked { 0.1 } else { 0.08 };
                let aspect = tex.width() / tex.height();
                let max_w = card_r.w * 0.75;
                let mut render_w = base_h * aspect;
                let mut render_h = base_h;
                if render_w > max_w {
                    render_w = max_w;
                    render_h = max_w / aspect;
                }
                let ir = Rect::new(card_r.center().x - render_w / 2., icon_center_y - render_h / 2., render_w, render_h);
                ui.fill_rect(
                    ir,
                    if card.unlocked {
                        (*tex, ir, ScaleType::Fit, WHITE)
                    } else {
                        (*tex, ir, ScaleType::Fit, semi_white(0.45))
                    },
                );
            } else {
                ui.text(def.icon)
                    .pos(card_r.center().x, icon_center_y)
                    .anchor(0.5, 0.5)
                    .size(if card.unlocked { 0.72 } else { 0.55 })
                    .color(if card.unlocked { WHITE } else { semi_white(0.45) })
                    .draw();
            }

            // 左上角数字编号（按用户指定顺序 1-20）+ 未解锁锁标记
            let num_text = format!("#{}", def.order);
            let mut num_x = card_r.x + 0.012;
            if !card.unlocked {
                ui.text("🔒")
                    .pos(num_x, card_r.y + 0.01)
                    .anchor(0., 0.)
                    .size(0.3)
                    .color(semi_white(0.65))
                    .draw();
                num_x += 0.04;
            }
            ui.text(&num_text)
                .pos(num_x, card_r.y + 0.012)
                .anchor(0., 0.)
                .size(0.26)
                .color(if card.unlocked {
                    semi_white(0.7)
                } else {
                    semi_white(0.5)
                })
                .draw();

            // 稀有度徽章（右上角，深色底 + 稀有度色文字，模仿谱面库等级徽章）
            let rarity_label = tl!(def.rarity.label_key());
            let mut rarity_text = ui
                .text(rarity_label)
                .pos(card_r.right() - 0.012, card_r.y + 0.012)
                .max_width(card_r.w * 0.6)
                .anchor(1., 0.)
                .size(0.4)
                .color(if card.unlocked {
                    Color::new(r_c, g_c, b_c, 1.0)
                } else {
                    semi_white(0.5)
                });
            let ms = rarity_text.measure();
            rarity_text.ui.fill_path(
                &ms.feather(0.008).rounded(0.014),
                Color {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: if card.unlocked { 0.55 } else { 0.35 },
                },
            );
            rarity_text.draw();

            // 成就名（左下角，模仿谱面库名称）
            ui.text(tl!(def.name))
                .pos(card_r.x + 0.012, card_r.bottom() - 0.035)
                .max_width(card_r.w - 0.03)
                .anchor(0., 1.)
                .size(0.42)
                .color(if card.unlocked { WHITE } else { semi_white(0.55) })
                .draw_using(&prpr::core::BOLD_FONT);

            // 进度文字（右下角，进度条上方）
            let progress = if def.target > 0 {
                (card.current as f32 / def.target as f32).clamp(0., 1.)
            } else {
                0.
            };
            let progress_text = if card.unlocked {
                "✓".to_string()
            } else {
                format!("{}/{}", card.current, def.target)
            };
            ui.text(&progress_text)
                .pos(card_r.right() - 0.012, card_r.bottom() - 0.038)
                .anchor(1., 1.)
                .size(0.24)
                .color(if card.unlocked {
                    Color::new(r_c, g_c, b_c, 0.95)
                } else {
                    semi_white(0.6)
                })
                .draw();

            // 进度条（卡片底部）
            let bar_h = 0.01;
            let bar_y = card_r.bottom() - bar_h - 0.008;
            let bar_x = card_r.x + 0.014;
            let bar_w = card_r.w - 0.028;
            let bg_r = Rect::new(bar_x, bar_y, bar_w, bar_h);
            ui.fill_path(&bg_r.rounded(0.005), semi_black(0.45));
            if progress > 0. {
                let fill_r = Rect::new(bar_x, bar_y, bar_w * progress, bar_h);
                ui.fill_path(
                    &fill_r.rounded(0.005),
                    if card.unlocked {
                        Color::new(r_c, g_c, b_c, 0.95)
                    } else {
                        semi_white(0.55)
                    },
                );
            }
        });
    }
}

pub struct AchievementPage {
    manager: Arc<Mutex<AchievementManager>>,

    /// 左侧分类 tab 栏（模仿谱面库）
    tabs: Tabs<AchievementList>,

    /// 选中的成就（当前分类列表中的索引，用于详情弹窗）
    selected: Option<usize>,
    close_btn: DRectButton,
    /// 详情弹窗面板位置（用于点击弹窗外关闭）
    detail_rect: Rect,
    /// 详情弹窗入场动画开始时间（每次打开时重置）
    popup_enter_time: f32,

    enter_time: f32,
}

impl AchievementPage {
    pub fn new() -> Result<Self> {
        let manager = crate::achievement::manager();
        // 打开页面时同步收藏进度并重新判定一次成就，保证数据新鲜、不漏判
        crate::achievement::sync_favorites();
        crate::achievement::recheck_unlocks();

        let tabs = Tabs::new([
            (AchievementList::new(AchievementFilter::All), (|| tl!("category-all").into()) as TitleFn),
            (
                AchievementList::new(AchievementFilter::Category(AchievementCategory::Play)),
                (|| tl!("category-play").into()) as TitleFn,
            ),
            (
                AchievementList::new(AchievementFilter::Category(AchievementCategory::Score)),
                (|| tl!("category-score").into()) as TitleFn,
            ),
            (
                AchievementList::new(AchievementFilter::Category(AchievementCategory::Difficulty)),
                (|| tl!("category-difficulty").into()) as TitleFn,
            ),
            (
                AchievementList::new(AchievementFilter::Category(AchievementCategory::Collection)),
                (|| tl!("category-collection").into()) as TitleFn,
            ),
        ]);
        Ok(Self {
            manager,
            tabs,
            selected: None,
            close_btn: DRectButton::new(),
            detail_rect: Rect::default(),
            popup_enter_time: f32::NAN,
            enter_time: 0.0,
        })
    }

    fn enter(&mut self, s: &mut SharedState) -> Result<()> {
        // 进入页面时让卡片网格带渐变淡入
        for list in self.tabs.iter_mut() {
            list.fader.sub(s.t);
        }
        Ok(())
    }

    /// 渲染详情弹窗（带入场动画：遮罩淡入 + 面板从中心放大，与游戏 Dialog 风格一致）
    fn render_detail(&mut self, ui: &mut Ui, t: f32) {
        let Some(idx) = self.selected else {
            return;
        };
        // 入场动画计时（每次打开弹窗时重置）
        if self.popup_enter_time.is_nan() {
            self.popup_enter_time = t;
        }
        let p = if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
            1.
        } else {
            ((t - self.popup_enter_time) / 0.22).clamp(0., 1.)
        };
        let ease = 1. - (1. - p).powi(3);
        let scale = 0.94 + 0.06 * ease;
        // 先取出卡片数据，避免长借用 self.tabs
        let (def, current, unlocked, unlocked_at) = {
            let Some(card) = self.tabs.selected_mut().cards.get(idx) else {
                return;
            };
            (card.def, card.current, card.unlocked, card.unlocked_at)
        };
        let (r_c, g_c, b_c) = def.rarity.color();

        // 半透明背景遮罩（淡入）
        let screen = ui.screen_rect();
        ui.fill_rect(screen, semi_black(0.7 * ease));

        // 弹窗面板（从中心放大展开）
        let panel_w = screen.w * 0.7 * scale;
        let panel_h = screen.h * 0.55 * scale;
        let panel_r = Rect::new(
            screen.center().x - panel_w / 2.,
            screen.center().y - panel_h / 2.,
            panel_w,
            panel_h,
        );
        // 记录面板位置（用于点击弹窗外关闭）
        self.detail_rect = panel_r;
        let panel_path = panel_r.rounded(0.03);
        ui.fill_path(
            &panel_path,
            (
                Color::new(0.08, 0.08, 0.12, 0.98 * ease),
                (0., 0.),
                Color::new(0.04, 0.04, 0.06, 0.98 * ease),
                (0., panel_h),
            ),
        );

        // 顶部色条
        let top_bar = Rect::new(panel_r.x, panel_r.y, panel_r.w, 0.012);
        ui.fill_path(&top_bar.rounded(0.03), Color::new(r_c, g_c, b_c, 1.0));

        // 关闭按钮
        let close_size = 0.06;
        let close_r = Rect::new(
            panel_r.right() - close_size - 0.02,
            panel_r.y + 0.02,
            close_size,
            close_size,
        );
        self.close_btn.render_shadow(ui, close_r, t, |ui, _| {
            ui.fill_path(&close_r.rounded(0.015), semi_black(0.4));
            ui.text("✕")
                .pos(close_r.center().x, close_r.center().y)
                .anchor(0.5, 0.5)
                .size(0.4)
                .color(WHITE)
                .draw();
        });

        // 大图标（自定义图标，按原图宽高比渲染不拉伸；未加载时回退 emoji）
        if let Some(tex) = crate::achievement::achievement_icon(def) {
            let base_h = if unlocked { 0.2 } else { 0.16 };
            let aspect = tex.width() / tex.height();
            let max_w = panel_r.w * 0.7;
            let mut render_w = base_h * aspect;
            let mut render_h = base_h;
            if render_w > max_w {
                render_w = max_w;
                render_h = max_w / aspect;
            }
            let ir = Rect::new(panel_r.center().x - render_w / 2., panel_r.y + 0.09, render_w, render_h);
            ui.fill_rect(
                ir,
                if unlocked {
                    (*tex, ir, ScaleType::Fit, WHITE)
                } else {
                    (*tex, ir, ScaleType::Fit, semi_white(0.4))
                },
            );
        } else {
            ui.text(def.icon)
                .pos(panel_r.center().x, panel_r.y + 0.09)
                .anchor(0.5, 0.)
                .size(if unlocked { 1.3 } else { 1.0 })
                .color(if unlocked { WHITE } else { semi_white(0.35) })
                .draw();
        }

        // 名称
        ui.text(tl!(def.name))
            .pos(panel_r.center().x, panel_r.y + 0.27)
            .anchor(0.5, 0.)
            .size(0.6)
            .color(WHITE)
            .draw_using(&prpr::core::BOLD_FONT);

        // 稀有度 + 分类
        let meta_text = format!("{} · {}", tl!(def.rarity.label_key()), tl!(def.category.label_key()));
        ui.text(&meta_text)
            .pos(panel_r.center().x, panel_r.y + 0.35)
            .anchor(0.5, 0.)
            .size(0.3)
            .color(Color::new(r_c, g_c, b_c, 0.9))
            .draw();

        // 描述
        ui.text(tl!(def.description))
            .pos(panel_r.center().x, panel_r.y + 0.42)
            .anchor(0.5, 0.)
            .max_width(panel_r.w - 0.1)
            .multiline()
            .h_center()
            .size(0.32)
            .color(semi_white(0.75))
            .draw();

        // 进度
        let progress = if def.target > 0 {
            (current as f32 / def.target as f32).clamp(0., 1.)
        } else {
            0.
        };
        let bar_y = panel_r.bottom() - 0.08;
        let bar_w = panel_r.w * 0.6;
        let bar_x = panel_r.center().x - bar_w / 2.;
        let bar_h = 0.025;
        let bg_r = Rect::new(bar_x, bar_y, bar_w, bar_h);
        ui.fill_path(&bg_r.rounded(0.012), semi_black(0.5));
        if progress > 0. {
            let fill_r = Rect::new(bar_x, bar_y, bar_w * progress, bar_h);
            ui.fill_path(
                &fill_r.rounded(0.012),
                if unlocked {
                    Color::new(r_c, g_c, b_c, 0.95)
                } else {
                    semi_white(0.6)
                },
            );
        }

        // 进度文字
        let progress_text = if unlocked {
            let args = prpr_l10n::fluent_args!["current" => current, "target" => def.target];
            tl!("achievement-unlocked", &args).into_owned()
        } else {
            let args = prpr_l10n::fluent_args!["current" => current, "target" => def.target, "percent" => (progress * 100.) as u32];
            tl!("achievement-progress-text", &args).into_owned()
        };
        ui.text(&progress_text)
            .pos(panel_r.center().x, bar_y - 0.01)
            .anchor(0.5, 1.)
            .size(0.28)
            .color(semi_white(0.7))
            .draw();

        // 解锁时间
        if let Some(ts) = unlocked_at {
            let date = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default();
            let args = prpr_l10n::fluent_args!["date" => date.as_str()];
            ui.text(tl!("achievement-unlocked-at", &args))
                .pos(panel_r.center().x, bar_y - 0.045)
                .anchor(0.5, 1.)
                .size(0.24)
                .color(semi_white(0.5))
                .draw();
        }
    }
}

impl Page for AchievementPage {
    fn label(&self) -> Cow<'static, str> {
        tl!("achievement-title").into()
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        self.enter_time = (self.enter_time + get_frame_time()).min(1.0);
        self.tabs.selected_mut().scroll.update(t);
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;

        // 详情弹窗优先处理
        if self.selected.is_some() {
            if self.close_btn.touch(touch, t) {
                self.selected = None;
                return Ok(true);
            }
            // 仅在新触摸按下（Started）且落在面板外时关闭弹窗。
            // 不能对所有触摸事件生效，否则"打开弹窗的那次点击"的抬起事件
            // 会被当成面板外点击，弹窗闪一下立刻关闭（连点问题）。
            if touch.phase == TouchPhase::Started && !self.detail_rect.contains(touch.position) {
                self.selected = None;
                return Ok(true);
            }
            return Ok(true);
        }

        // 左侧分类 tab 栏（含切换过渡动画）
        if self.tabs.touch(touch, s.rt) {
            if self.tabs.changed() {
                self.selected = None;
                // 切换分类时让卡片网格渐变淡入
                let t = s.t;
                for list in self.tabs.iter_mut() {
                    list.fader.sub(t);
                }
            }
            return Ok(true);
        }

        // 卡片点击 / 滚动
        let (consumed, card) = self.tabs.selected_mut().touch(touch, t);
        if let Some(i) = card {
            self.selected = Some(i);
            // 重新播放弹窗入场动画
            self.popup_enter_time = f32::NAN;
        }
        Ok(consumed)
    }

    fn render_top(&mut self, ui: &mut Ui, _s: &mut SharedState) -> Result<()> {
        // 在标题行右侧（屏幕右上角）渲染"已解锁 X/Y"统计
        let mgr = self.manager.lock().unwrap();
        let unlocked = mgr.unlocked_count();
        let total = AchievementManager::total_count();
        drop(mgr);

        let back_rect = ui.back_rect();
        let right_x = ui.content_rect().right() - 0.04;
        let percent = if total > 0 { unlocked as f32 / total as f32 * 100. } else { 0. };
        let args = prpr_l10n::fluent_args!["unlocked" => unlocked, "total" => total, "percent" => percent as u32];
        let stats_text = tl!("achievement-stats", &args).into_owned();
        ui.text(&stats_text)
            .pos(right_x, back_rect.center().y)
            .anchor(1., 0.5)
            .size(0.42)
            .color(semi_white(0.85))
            .draw_using(&prpr::core::BOLD_FONT);

        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let rt = s.rt;
        let r = ui.content_rect();

        s.render_fader(ui, |ui| {
            let main_r = r.feather(-0.01);
            ui.fill_path(&main_r.rounded(0.01), semi_black(0.15));
            self.tabs.render(ui, rt, r, |ui, list| {
                list.render(ui, r.feather(-0.015), t);
                Ok(())
            })
        })?;

        // 详情弹窗（最后渲染，在最上层）
        if self.selected.is_some() {
            self.render_detail(ui, t);
        }

        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        NextPage::None
    }
}
