//! 局域网联机UI组件

use crate::get_data;
use crate::lan::{LanManager, LanState, ServerInfo};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    core::Tweenable,
    ext::{semi_black, semi_white, RectExt},
    ui::{DRectButton, Scroll, Ui},
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::info;

/// 面板宽度
const WIDTH: f32 = 1.6;
/// 进入动画时长
const ENTER_TRANSIT: f32 = 0.5;

/// 局域网联机面板
pub struct LanPanel {
    /// 管理器
    manager: Arc<Mutex<LanManager>>,
    /// 是否显示
    visible: bool,
    /// 进入时间
    enter_time: f32,
    /// 发现到的服务器列表
    servers: Vec<ServerInfo>,
    /// 选中的服务器索引
    selected_server: Option<usize>,
    /// 玩家名称
    player_name: String,
    /// 输入框焦点
    input_focused: bool,
    /// 滚动条
    scroll: Scroll,
    /// 服务器列表项按钮（用于点击选中）
    server_btns: Vec<DRectButton>,
    /// 招募按钮
    recruit_btn: DRectButton,
    /// 刷新按钮
    refresh_btn: DRectButton,
    /// 加入按钮
    join_btn: DRectButton,
    /// 离开按钮
    leave_btn: DRectButton,
    /// 准备按钮
    ready_btn: DRectButton,
    /// 取消准备按钮
    cancel_ready_btn: DRectButton,
    /// 开始游戏按钮
    start_game_btn: DRectButton,
    /// 等待其他玩家开关
    waiting_switch: bool,
    /// 发声设备列表
    audio_devices: Vec<String>,
    /// 当前选择的设备
    selected_device: usize,
    /// 自定义设备按钮
    custom_btn: DRectButton,
    /// 屏幕半高（用于触摸坐标换算）
    top: f32,
}

impl LanPanel {
    /// 创建新的面板
    pub fn new(manager: Arc<Mutex<LanManager>>) -> Self {
        Self {
            manager,
            visible: false,
            enter_time: 0.0,
            servers: Vec::new(),
            selected_server: None,
            player_name: "Player".to_string(),
            input_focused: false,
            scroll: Scroll::new(),
            server_btns: Vec::new(),
            recruit_btn: DRectButton::new(),
            refresh_btn: DRectButton::new(),
            join_btn: DRectButton::new(),
            leave_btn: DRectButton::new(),
            ready_btn: DRectButton::new(),
            cancel_ready_btn: DRectButton::new(),
            start_game_btn: DRectButton::new(),
            waiting_switch: false,
            audio_devices: Vec::new(),
            selected_device: 0,
            custom_btn: DRectButton::new(),
            top: 1.0,
        }
    }

    /// 显示面板
    pub fn show(&mut self, rt: f32) {
        self.visible = true;
        self.enter_time = rt;

        // 名称读取当前登录玩家的名称
        if let Some(name) = get_data().me.as_ref().map(|it| it.name.clone()) {
            if !name.is_empty() {
                self.player_name = name;
            }
        }
        // 不自动开始搜索，由用户点击“刷新”触发
    }

    /// 隐藏面板
    pub fn hide(&mut self) {
        self.visible = false;
        
        // 停止发现服务器
        if let Ok(mut manager) = self.manager.lock() {
            manager.stop_discovery();
        }
    }

    /// 面板是否可见
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// 更新
    pub fn update(&mut self, t: f32) -> Result<()> {
        if !self.visible {
            return Ok(());
        }

        // 检测房主离开/房间关闭导致的断开，自动退出房间
        if let Ok(mut manager) = self.manager.lock() {
            manager.check_disconnected();
        }
        // 更新服务器列表（仅在数量变化时打日志，避免刷屏）
        if let Ok(manager) = self.manager.lock() {
            let new_servers = manager.get_servers();
            if new_servers.len() != self.servers.len() {
                info!("LAN discovery: {} server(s) found", new_servers.len());
            }
            self.servers = new_servers;
        }

        // 更新滚动条
        self.scroll.update(t);

        Ok(())
    }

    /// 渲染面板（侧边滑入风格，坐标系统参照多人游戏面板）
    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        if !self.visible {
            return;
        }
        // 记录屏幕半高，供触摸坐标换算使用
        self.top = ui.top;

        // 面板在 enter_time 之后的滑入动画进度
        let p = ((t - self.enter_time) / ENTER_TRANSIT).clamp(0., 1.);
        let p = 1. - (1. - p).powi(3);

        // 半透明遮罩
        ui.fill_rect(ui.screen_rect(), semi_black(0.6 * p));

        // 侧边滑入面板（从屏幕左侧滑入，与多人游戏面板一致）
        let w = WIDTH;
        let pos = f32::tween(&-1., &(w - 1.), p);
        ui.scope(|ui| {
            ui.dx(pos - w);
            ui.dy(-ui.top);
            let h = ui.top * 2.;
            let panel = Rect::new(0., 0., w, h).feather(-0.02);
            ui.fill_path(&panel.rounded(0.02), ui.background());

            // 标题
            ui.text("局域网联机").pos(0.05, 0.05).size(0.7).color(WHITE).draw();
            let state = self.manager.lock().map(|m| m.get_state()).unwrap_or(LanState::Disconnected);
            let connected = matches!(state, LanState::Connected { .. });

            // 玩家名称输入框
            let input_rect = Rect::new(0.05, 0.16, w - 0.1, 0.07);
            ui.fill_rect(input_rect, semi_black(0.4));
            ui.text(&self.player_name)
                .pos(input_rect.x + 0.02, input_rect.center().y)
                .anchor(0.0, 0.5)
                .size(0.55)
                .color(WHITE)
                .draw();
            ui.text("名称").pos(input_rect.x, input_rect.y - 0.03).size(0.4).color(semi_white(0.6)).draw();

            // 刷新按钮（招募状态下禁用）
            let refresh_rect = Rect::new(0.05, 0.28, w - 0.1, 0.07);
            self.refresh_btn.render_text(ui, refresh_rect, t, "刷新", 0.55, !connected);

            // 服务器列表
            let list_rect = Rect::new(0.05, 0.4, w - 0.1, h - 0.78);
            ui.fill_rect(list_rect, semi_black(0.3));
            if connected {
                // 已连接：显示成员列表（黑框内）
                let members = self.manager.lock().map(|m| m.get_members()).unwrap_or_default();
                let mut y = 0.0;
                for m in &members {
                    let status = if m.is_host {
                        "房主"
                    } else if m.ready {
                        "✓ 已准备"
                    } else {
                        "未准备"
                    };
                    ui.text(format!("{}  {}", m.name, status))
                        .pos(list_rect.x + 0.03, list_rect.y + 0.03 + y)
                        .size(0.5)
                        .color(if m.ready || m.is_host { WHITE } else { semi_white(0.6) })
                        .draw();
                    y += 0.065;
                }
                if members.is_empty() {
                    ui.text("房间已创建，等待玩家加入...")
                        .pos(list_rect.center().x, list_rect.center().y)
                        .anchor(0.5, 0.5)
                        .size(0.5)
                        .color(semi_white(0.5))
                        .draw();
                }
            } else {
                // 未连接：显示服务器搜索列表
                if self.servers.is_empty() {
                    ui.text("正在搜索局域网房间...")
                        .pos(list_rect.center().x, list_rect.center().y)
                        .anchor(0.5, 0.5)
                        .size(0.5)
                        .color(semi_white(0.5))
                        .draw();
                }
                self.server_btns.resize(self.servers.len(), DRectButton::new());
                self.scroll.size((list_rect.w, list_rect.h));
                // 把滚动内容偏移到黑框位置，避免列表项画到窗口顶部
                ui.scope(|ui| {
                    ui.dx(list_rect.x);
                    ui.dy(list_rect.y);
                    self.scroll.render(ui, |ui| {
                        let mut y = 0.0;
                        for (i, server) in self.servers.iter().enumerate() {
                            let item_rect = Rect::new(0.02, y, list_rect.w - 0.04, 0.12);
                            if i < self.server_btns.len() {
                                self.server_btns[i].inner.set(ui, item_rect);
                            }
                            if self.selected_server == Some(i) {
                                ui.fill_rect(item_rect, semi_white(0.2));
                            }
                            ui.text(&server.name).pos(item_rect.x + 0.02, item_rect.y + 0.02).size(0.55).color(WHITE).draw();
                            ui.text(&server.room_name).pos(item_rect.x + 0.02, item_rect.y + 0.06).size(0.45).color(semi_white(0.8)).draw();
                            ui.text(format!("{} / {}", server.player_count, server.max_players))
                                .pos(item_rect.x + 0.02, item_rect.y + 0.1)
                                .size(0.4)
                                .color(semi_white(0.6))
                                .draw();
                            y += item_rect.h + 0.02;
                        }
                        (list_rect.w, y)
                    });
                });
            }

            // 底部按钮区
            let btn_h = 0.07;
            let bw = (w - 0.1 - 0.02) / 2.;

            match state {
                LanState::Connected { is_host, .. } => {
                    // 已连接：隐藏招募/加入，显示房间控制
                    // 连接状态指示（标题右侧）
                    ui.text("已连接")
                        .pos(w - 0.1, 0.06)
                        .anchor(1., 0.)
                        .size(0.4)
                        .color(Color::from_rgba(80, 200, 120, 255))
                        .draw();
                    let mut by = h - 0.16;
                    let leave_rect = Rect::new(0.05, by, bw, btn_h);
                    self.leave_btn.render_text(ui, leave_rect, t, "离开房间", 0.5, true);
                    if is_host {
                        let start_rect = Rect::new(0.05 + bw + 0.02, by, bw, btn_h);
                        self.start_game_btn.render_text(ui, start_rect, t, "开始游戏", 0.5, true);
                    } else {
                        let ready_rect = Rect::new(0.05 + bw + 0.02, by, bw, btn_h);
                        self.ready_btn.render_text(ui, ready_rect, t, "准备", 0.5, true);
                    }
                    by -= btn_h + 0.02;
                    if is_host {
                        // 等待其他玩家开关（房主）
                        let waiting_rect = Rect::new(0.05, by, w - 0.1, btn_h);
                        ui.text("等待其他玩家").pos(waiting_rect.x, waiting_rect.center().y).anchor(0.0, 0.5).size(0.5).color(WHITE).draw();
                        let switch_rect = Rect::new(waiting_rect.right() - 0.12, waiting_rect.y + 0.015, 0.1, btn_h - 0.03);
                        ui.fill_rect(switch_rect, semi_black(0.4));
                        if self.waiting_switch {
                            ui.fill_rect(Rect::new(switch_rect.x + switch_rect.w / 2., switch_rect.y, switch_rect.w / 2., switch_rect.h), WHITE);
                        }
                        by -= btn_h + 0.02;
                    }
                    // 发声设备：房主选择让哪个玩家的设备发声
                    let audio_rect = Rect::new(0.05, by, w - 0.1, btn_h);
                    ui.fill_rect(audio_rect, semi_black(0.3));
                    ui.text("发声设备").pos(audio_rect.x + 0.02, audio_rect.center().y).anchor(0.0, 0.5).size(0.5).color(WHITE).draw();
                    // 目标玩家列表（含房主自己）
                    let members = self.manager.lock().map(|m| m.get_members()).unwrap_or_default();
                    let targets: Vec<String> = members.iter().map(|m| m.name.clone()).collect();
                    if !targets.is_empty() {
                        self.audio_devices = targets;
                    }
                    if self.selected_device >= self.audio_devices.len() {
                        self.selected_device = 0;
                    }
                    let dev_text = if self.audio_devices.is_empty() {
                        "无成员".to_string()
                    } else {
                        format!("{} 的设备", self.audio_devices[self.selected_device])
                    };
                    ui.text(&dev_text)
                        .pos(audio_rect.right() - 0.02, audio_rect.center().y)
                        .anchor(1.0, 0.5)
                        .size(0.45)
                        .color(semi_white(0.8))
                        .draw();
                }
                _ => {
                    // 未连接：显示招募/加入
                    let by = h - 0.16;
                    let recruit_rect = Rect::new(0.05, by, bw, btn_h);
                    self.recruit_btn.render_text(ui, recruit_rect, t, "招募", 0.5, true);
                    let join_rect = Rect::new(0.05 + bw + 0.02, by, bw, btn_h);
                    if self.selected_server.is_some() {
                        self.join_btn.render_text(ui, join_rect, t, "加入房间", 0.5, true);
                    }
                }
            }
        });
    }

    /// 处理触摸输入
    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        if !self.visible {
            return false;
        }

        let w = WIDTH;
        // 面板完全滑入后右边缘在 x = w - 1（归一化坐标），点击面板右侧之外关闭
        if touch.position.x + 1. > w {
            self.hide();
            return true;
        }

        // 面板内相对坐标（面板左边缘在 x = -1，顶部在 y = -top）
        let rel = vec2(touch.position.x + 1., touch.position.y + self.top);
        let h = self.top * 2.;

        // 玩家名称输入框
        let input_rect = Rect::new(0.05, 0.16, w - 0.1, 0.07);
        if input_rect.contains(rel) {
            self.input_focused = true;
            return true;
        }

        // 刷新按钮（招募状态下禁用）
        let state = self.manager.lock().map(|m| m.get_state()).unwrap_or(LanState::Disconnected);
        let connected = matches!(state, LanState::Connected { .. });
        if !connected && self.refresh_btn.touch(touch, t) {
            if let Ok(mut manager) = self.manager.lock() {
                let _ = manager.start_discovery();
            }
            return true;
        }

        // 服务器列表项（通过按钮点击选中）
        if !connected {
            for (i, btn) in self.server_btns.iter_mut().enumerate() {
                if btn.touch(touch, t) {
                    self.selected_server = Some(i);
                    return true;
                }
            }
        }

        // 底部按钮区
        let btn_h = 0.07;
        let bw = (w - 0.1 - 0.02) / 2.;

        match state {
            LanState::Connected { is_host, .. } => {
                // 已连接：隐藏招募/加入，只响应房间控制
                let mut by = h - 0.16;
                if self.leave_btn.touch(touch, t) {
                    if let Ok(mut manager) = self.manager.lock() {
                        manager.disconnect();
                    }
                    return true;
                }
                if is_host {
                    if self.start_game_btn.touch(touch, t) {
                        info!("Host pressed start game button");
                        if let Ok(mut manager) = self.manager.lock() {
                            let path = manager.my_chart_path.clone();
                            let _ = manager.start_game(self.waiting_switch, path);
                        }
                        prpr::scene::show_message("已开始游戏").ok();
                        return true;
                    }
                } else if self.ready_btn.touch(touch, t) {
                    if let Ok(mut manager) = self.manager.lock() {
                        let _ = manager.ready(true);
                    }
                    return true;
                }
                by -= btn_h + 0.02;
                if is_host {
                    let waiting_rect = Rect::new(0.05, by, w - 0.1, btn_h);
                    if waiting_rect.contains(rel) {
                        self.waiting_switch = !self.waiting_switch;
                        return true;
                    }
                    by -= btn_h + 0.02;
                }
                // 发声设备行：点击切换设备
                let audio_rect = Rect::new(0.05, by, w - 0.1, btn_h);
                if audio_rect.contains(rel) && !self.audio_devices.is_empty() {
                    self.selected_device = (self.selected_device + 1) % self.audio_devices.len();
                    return true;
                }
            }
            _ => {
                // 未连接：显示招募/加入
                let by = h - 0.16;
                if self.recruit_btn.touch(touch, t) {
                    if let Ok(mut manager) = self.manager.lock() {
                        let mut config = crate::lan::LanConfig::default();
                        config.server_name = self.player_name.clone();
                        let _ = manager.create_room(config, self.player_name.clone());
                    }
                    return true;
                }
                if self.join_btn.touch(touch, t) && self.selected_server.is_some() {
                    if let Some(server) = self.selected_server.and_then(|i| self.servers.get(i)) {
                        if let Ok(mut manager) = self.manager.lock() {
                            let _ = manager.join_room(server.address.clone(), self.player_name.clone());
                        }
                    }
                    return true;
                }
            }
        }

        true
    }
}
