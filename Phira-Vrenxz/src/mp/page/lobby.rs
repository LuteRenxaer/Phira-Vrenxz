//! 主页（已连接、未进房）。
//!
//! 版面就是"房间大厅"：**中间是公共房间列表**（点任意一行直接进房，行内「观战」按钮以
//! 观战者身份旁观），**右下角是一小簇缩小后的功能按钮**（创建房间 / 加入房间 / 刷新 /
//! 断开连接）。页头只有返回 + 标题 + 当前服务器地址，不再用整条底部操作条去占版面。
//!
//! ```text
//! ‹  多人游戏   ·   服务器 127.0.0.1:31205
//! ┌─────────────────────────────────────────┐
//! │ #31205 · 等待开始 · 3 名玩家      [观战] │  ← 中央：房间列表
//! │ #31206 · 游戏进行中 · 1 名玩家    [观战] │
//! └─────────────────────────────────────────┘
//!                          [创建房间] [加入房间]  ← 右下角：缩小后的按钮
//!                          [刷新]     [断开连接]
//! ```

use macroquad::prelude::*;
use prpr::ui::{DRectButton, Scroll, Ui};

use super::super::{
    state::PublicRoom,
    theme::{self, *},
};
use crate::mp::L10N_LOCAL;

/// 主页可执行的动作。
pub enum Action {
    Back,
    CreateRoom,
    JoinRoom,
    /// 重新拉取公共房间列表
    Refresh,
    Disconnect,
    /// 点房间行直接进房（对局中的房间由会话改判为观战）
    Join(String),
    /// 行内「观战」：以观战者身份（monitor）旁观
    Spectate(String),
}

/// 主页需要的只读状态。
pub struct View<'a> {
    /// 服务器地址
    pub address: &'a str,
    /// `None` 表示还没拿到过列表（首次加载中）
    pub rooms: Option<&'a [PublicRoom]>,
    /// 拉取请求在途
    pub loading: bool,
    /// 自己所在房间（用于把该行标成「观战中」）
    pub joined: Option<&'a str>,
    /// 自己是否处于观战状态
    pub spectating: bool,
    /// 是否有会话级任务在跑（建房 / 拉取房间列表…）
    pub busy: bool,
}

/// 右下角小按钮的尺寸。
const SMALL_BTN_H: f32 = 0.115 * SCALE;

#[derive(Default)]
pub struct LobbyPage {
    back: DRectButton,
    create: DRectButton,
    join: DRectButton,
    refresh: DRectButton,
    disconnect: DRectButton,
    scroll: Scroll,
    /// 行按钮（索引与 `ids` 对齐）
    rows: Vec<DRectButton>,
    /// 「观战」按钮（索引与 `ids` 对齐）
    watches: Vec<DRectButton>,
    /// 渲染时记录的房间 id：触摸只按这份数据索引，与渲染同一来源
    ids: Vec<String>,
}

impl LobbyPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.create.invalidate();
        self.join.invalidate();
        self.refresh.invalidate();
        self.disconnect.invalidate();
        for b in self.rows.iter_mut() {
            b.invalidate();
        }
        for b in self.watches.iter_mut() {
            b.invalidate();
        }
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        let wide = theme::is_wide(ui);
        let top = ui.top;
        let pad = theme::page_pad(ui);

        // —— 顶部居中大标题「房间列表」——
        let title = mtl!("room-list-title");
        let strip_h = 0.14 * SCALE;
        ui.text(title.as_ref())
            .pos(0., -top + HEADER_TOP + strip_h * 0.6)
            .anchor(0.5, 0.5)
            .size(FS_PAGE_TITLE)
            .color(text())
            .draw();

        // 标题两侧小工具：刷新 / 加入房间（输 ID）
        let icon_sz = strip_h * 0.9;
        let tx = 0.86;
        let ic_refresh = theme::tool_icon(theme::ToolIcon::Refresh);
        let ic_join = theme::tool_icon(theme::ToolIcon::Join);
        theme::tool_button(ui, &mut self.refresh, t, Rect::new(tx - icon_sz, -top + HEADER_TOP, icon_sz, icon_sz), ic_refresh.as_ref(), "", false, accent);
        theme::tool_button(ui, &mut self.join, t, Rect::new(tx - icon_sz * 2. - 0.03, -top + HEADER_TOP, icon_sz, icon_sz), ic_join.as_ref(), "", false, accent);

        // —— 底部两个大圆角按钮：左下「退出」 右下「创建房间」——
        let bottom_h = (0.16 * SCALE).clamp(0.13, 0.2);
        let bottom_w = (0.62 * SCALE).clamp(0.5, 0.85);
        let bottom_y = top - pad - bottom_h;
        let back_r = Rect::new(-1. + pad, bottom_y, bottom_w, bottom_h);
        let create_r = Rect::new(1. - pad - bottom_w, bottom_y, bottom_w, bottom_h);
        theme::button(ui, &mut self.back, t, back_r, mtl!("leave-room"), (bottom_h * 3.0).clamp(0.24, FS_BUTTON), secondary(), text());
        theme::button(ui, &mut self.create, t, create_r, mtl!("create-room"), (bottom_h * 3.0).clamp(0.24, FS_BUTTON), primary(accent), WHITE);

        // —— 中央：房间卡片网格（列表底部留出按钮高度）——
        let list_top = -top + HEADER_TOP + strip_h + BODY_GAP;
        let list_bottom = bottom_y - BAR_GAP;
        let list = Rect::new(-1. + pad, list_top, 2. - pad * 2., (list_bottom - list_top).max(0.2));
        let rooms = v.rooms.unwrap_or(&[]);

        if rooms.is_empty() {
            let msg = if v.loading { mtl!("room-list-loading") } else { mtl!("room-list-empty") };
            ui.text(msg.as_ref())
                .pos(list.center().x, list.center().y)
                .anchor(0.5, 0.5)
                .size(FS_SECTION)
                .color(text_muted())
                .draw();
            self.ids.clear();
            self.rows.clear();
            self.watches.clear();
            return;
        }

        // 列数：横屏 3 列，竖屏 1 列
        let cols = if wide { 3 } else { 1 };
        let gap = 0.025 * SCALE;
        let card_h = (if wide { 0.30 } else { 0.26 }) * SCALE;
        let step = card_h + gap;
        let col_w = (list.w - gap * (cols - 1) as f32) / cols as f32;
        let rows_n = (rooms.len() as f32 / cols as f32).ceil() as usize;
        let view_h = rows_n as f32 * step;

        self.ids.clear();
        self.rows.resize_with(rooms.len(), DRectButton::new);
        self.watches.resize_with(rooms.len(), DRectButton::new);

        ui.scope(|ui| {
            ui.dx(list.x);
            ui.dy(list.y);
            self.scroll.size((list.w, list.h));
            self.scroll.render(ui, |ui| {
                for (i, room) in rooms.iter().enumerate() {
                    let col = i % cols;
                    let row = i / cols;
                    let cx = col as f32 * (col_w + gap);
                    let cy = row as f32 * step;
                    let card = Rect::new(cx, cy, col_w, card_h);
                    let playing = room.state != "waiting" && room.state != "准备中";
                    let locked = room.locked;

                    let watching_here = v.spectating && v.joined == Some(room.id.as_str());
                    // 整张卡片可点（row_button 自带半透明底 + 命中区）
                    theme::row_button(ui, &mut self.rows[i], t, card, watching_here, accent, |ui, card| {
                        // 左上：对勾 / 锁图标 + 房名
                        let icon_x = card.x + CARD_PAD;
                        let icon_y = card.y + card_h * 0.26;
                        if locked {
                            ui.text("🔒").pos(icon_x, icon_y).size(FS_BODY).color(text_dim()).draw();
                        } else {
                            ui.text("✔").pos(icon_x, icon_y).size(FS_BODY).color(WHITE).draw();
                        }
                        let name_w = card.w - CARD_PAD * 2. - 0.3;
                        theme::text_left_bold(ui, icon_x + 0.05 * SCALE, icon_y, FS_BODY, text(), &format!("#{}", room.id), name_w);
                        // 右上 ID
                        theme::text_right(ui, card.right() - CARD_PAD, icon_y, FS_SMALL, text_dim(), &format!("ID {}", room.id), 0.3);
                        // 第二行：房间状态/描述
                        theme::text_left(ui, card.x + CARD_PAD, card.y + card_h * 0.5, FS_SMALL, text_dim(), &room.state, card.w - CARD_PAD * 2.);
                        // 底部：左玩家数
                        let players = mtl!("mp-n-players", "n" => room.player_count as u64);
                        theme::text_left(ui, card.x + CARD_PAD, card.bottom() - card_h * 0.22, FS_SMALL, text(), &players, card.w * 0.55);
                    });
                    // 右下「加入 / 游戏中」按钮（叠在卡片上，点它优先于卡片）
                    let btn_w = (0.22 * SCALE).min(card.w * 0.28).max(0.13 * SCALE);
                    let btn_r = Rect::new(card.right() - CARD_PAD - btn_w, card.bottom() - card_h * 0.30, btn_w, card_h * 0.30);
                    if playing {
                        theme::button(ui, &mut self.watches[i], t, btn_r, mtl!("spectate"), FS_SMALL, Color::new(0.4, 0.4, 0.45, 0.6), text());
                    } else {
                        theme::button(ui, &mut self.watches[i], t, btn_r, mtl!("join-room"), FS_SMALL, if watching_here { primary(accent) } else { secondary() }, text());
                    }
                    self.ids.push(room.id.clone());
                }
                (list.w, view_h)
            });
        });

        if v.busy {
            theme::progress_bar(ui, Rect::new(list.x, list.bottom() - 0.012, list.w, 0.012), None, t, accent);
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.create.touch(touch, t) {
            return Some(Action::CreateRoom);
        }
        if self.join.touch(touch, t) {
            return Some(Action::JoinRoom);
        }
        if self.refresh.touch(touch, t) {
            return Some(Action::Refresh);
        }
        if self.disconnect.touch(touch, t) {
            return Some(Action::Disconnect);
        }
        if self.scroll.contains(touch) && self.scroll.touch(touch, t) {
            for b in self.rows.iter_mut() {
                b.inner.cancel();
            }
            return None;
        }
        // 先判定「观战」（行主体矩形已排除该区域，顺序只为更稳）
        for (i, b) in self.watches.iter_mut().enumerate() {
            if b.touch(touch, t) {
                return self.ids.get(i).cloned().map(Action::Spectate);
            }
        }
        for (i, b) in self.rows.iter_mut().enumerate() {
            if b.touch(touch, t) {
                return self.ids.get(i).cloned().map(Action::Join);
            }
        }
        None
    }
}
