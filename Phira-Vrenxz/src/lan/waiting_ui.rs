//! 等待其他玩家UI组件

use crate::lan::{LanManager, LanState};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{semi_black, semi_white},
    ui::{DRectButton, Ui},
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 等待其他玩家面板
pub struct WaitingPanel {
    /// 管理器
    manager: Arc<Mutex<LanManager>>,
    /// 是否显示
    visible: bool,
    /// 进入时间
    enter_time: f32,
    /// 已准备人数
    ready_count: u8,
    /// 总人数
    total_count: u8,
    /// 旋转角度
    rotation: f32,
    /// 离开按钮
    leave_btn: DRectButton,
}

impl WaitingPanel {
    /// 创建新的面板
    pub fn new(manager: Arc<Mutex<LanManager>>) -> Self {
        Self {
            manager,
            visible: false,
            enter_time: 0.0,
            ready_count: 0,
            total_count: 0,
            rotation: 0.0,
            leave_btn: DRectButton::new(),
        }
    }

    /// 显示面板
    pub fn show(&mut self, rt: f32, total_count: u8) {
        self.visible = true;
        self.enter_time = rt;
        self.total_count = total_count;
        self.ready_count = 0;
        self.rotation = 0.0;
    }

    /// 隐藏面板
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// 更新
    pub fn update(&mut self, t: f32) -> Result<()> {
        if !self.visible {
            return Ok(());
        }

        // 更新旋转角度
        self.rotation += 0.02;
        if self.rotation > std::f32::consts::PI * 2.0 {
            self.rotation = 0.0;
        }

        // 检查是否所有玩家都准备好了
        if self.ready_count >= self.total_count {
            self.hide();
        }

        Ok(())
    }

    /// 渲染
    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        if !self.visible {
            return;
        }

        // 绘制半透明背景
        let screen_rect = ui.screen_rect();
        ui.fill_rect(screen_rect, semi_black(0.8));

        // 绘制等待文本
        let wait_text = "正在等待其他玩家";
        let wait_text_rect = ui.text(wait_text)
            .pos(screen_rect.center().x, screen_rect.center().y - 0.1)
            .anchor(0.5, 0.5)
            .size(1.0)
            .color(WHITE)
            .measure();

        // 绘制旋转的圆圈
        let circle_radius = 0.05;
        let circle_center = wait_text_rect.center();
        
        // 绘制多个点形成旋转效果
        for i in 0..8 {
            let angle = self.rotation + (i as f32 * std::f32::consts::PI / 4.0);
            let x = circle_center.x + angle.cos() * circle_radius;
            let y = circle_center.y + angle.sin() * circle_radius;
            
            let point_size = 0.01;
            let point_rect = Rect::new(x - point_size, y - point_size, point_size * 2.0, point_size * 2.0);
            ui.fill_rect(point_rect, WHITE);
        }

        // 绘制进度文本
        let progress_text = format!("{}/{} 玩家已准备", self.ready_count, self.total_count);
        ui.text(&progress_text)
            .pos(screen_rect.center().x, circle_center.y + circle_radius + 0.05)
            .anchor(0.5, 0.0)
            .size(0.6)
            .color(semi_white(0.8))
            .draw();

        // 绘制离开按钮
        let leave_rect = Rect::new(screen_rect.center().x - 0.1, screen_rect.bottom() - 0.15, 0.2, 0.08);
        self.leave_btn.render_text(ui, leave_rect, t, "离开房间", 0.6, true);
    }

    /// 处理触摸输入
    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        if !self.visible {
            return false;
        }

        // 使用规范化屏幕区域（0-1）
        let screen_rect = Rect::new(0.0, 0.0, 1.0, 1.0);

        // 检查离开按钮
        let leave_rect = Rect::new(screen_rect.center().x - 0.1, screen_rect.bottom() - 0.15, 0.2, 0.08);
        if self.leave_btn.touch(touch, t) {
            self.hide();
            return true;
        }

        true
    }

    /// 更新准备人数
    pub fn update_ready_count(&mut self, count: u8) {
        self.ready_count = count;
    }

    /// 是否正在等待
    pub fn is_waiting(&self) -> bool {
        self.visible
    }
}

/// 自定义发声设备选择面板
pub struct AudioDevicePanel {
    /// 是否显示
    visible: bool,
    /// 设备列表
    devices: Vec<String>,
    /// 当前选择的设备
    selected_devices: Vec<bool>,
    /// 确认按钮
    confirm_btn: DRectButton,
    /// 取消按钮
    cancel_btn: DRectButton,
}

impl AudioDevicePanel {
    /// 创建新的面板
    pub fn new() -> Self {
        Self {
            visible: false,
            devices: Vec::new(),
            selected_devices: Vec::new(),
            confirm_btn: DRectButton::new(),
            cancel_btn: DRectButton::new(),
        }
    }

    /// 显示面板
    pub fn show(&mut self, devices: Vec<String>) {
        self.visible = true;
        self.devices = devices.clone();
        self.selected_devices = vec![true; devices.len()]; // 默认选择所有设备
    }

    /// 隐藏面板
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// 获取选择的设备索引
    pub fn get_selected_devices(&self) -> Vec<usize> {
        self.selected_devices
            .iter()
            .enumerate()
            .filter(|&(_, &selected)| selected)
            .map(|(i, _)| i)
            .collect()
    }

    /// 更新
    pub fn update(&mut self, _t: f32) -> Result<()> {
        Ok(())
    }

    /// 渲染
    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        if !self.visible {
            return;
        }

        // 绘制半透明背景
        let screen_rect = ui.screen_rect();
        ui.fill_rect(screen_rect, semi_black(0.8));

        // 绘制标题
        let title_rect = Rect::new(0.0, 0.0, screen_rect.w, 0.1);
        ui.fill_rect(title_rect, semi_black(0.6));
        ui.text("选择发声设备")
            .pos(title_rect.center().x, title_rect.center().y)
            .anchor(0.5, 0.5)
            .size(0.8)
            .color(WHITE)
            .draw();

        // 绘制设备列表
        let list_rect = Rect::new(0.1, 0.15, screen_rect.w - 0.2, screen_rect.h - 0.35);
        ui.fill_rect(list_rect, semi_black(0.3));

        let mut y = 0.05;
        for (i, device) in self.devices.iter().enumerate() {
            let item_rect = Rect::new(0.02, y, list_rect.w - 0.04, 0.08);
            
            // 绘制复选框
            let checkbox_rect = Rect::new(item_rect.x, item_rect.center().y - 0.02, 0.04, 0.04);
            ui.fill_rect(checkbox_rect, semi_black(0.4));
            
            if self.selected_devices[i] {
                // 绘制选中标记
                ui.fill_rect(Rect::new(checkbox_rect.x + 0.01, checkbox_rect.y + 0.01, 0.02, 0.02), WHITE);
            }
            
            // 绘制设备名称
            ui.text(device)
                .pos(checkbox_rect.right() + 0.02, item_rect.center().y)
                .anchor(0.0, 0.5)
                .size(0.5)
                .color(WHITE)
                .draw();
            
            y += item_rect.h + 0.02;
        }

        // 绘制按钮
        let button_y = screen_rect.bottom() - 0.1;
        let confirm_rect = Rect::new(screen_rect.center().x - 0.15, button_y, 0.14, 0.08);
        self.confirm_btn.render_text(ui, confirm_rect, t, "确认", 0.6, true);

        let cancel_rect = Rect::new(screen_rect.center().x + 0.01, button_y, 0.14, 0.08);
        self.cancel_btn.render_text(ui, cancel_rect, t, "取消", 0.6, true);
    }

    /// 处理触摸输入
    pub fn touch(&mut self, touch: &Touch, _t: f32) -> bool {
        if !self.visible {
            return false;
        }

        // 使用规范化屏幕区域（0-1）
        let screen_rect = Rect::new(0.0, 0.0, 1.0, 1.0);

        // 检查设备列表项
        let list_rect = Rect::new(0.1 * screen_rect.w, 0.15 * screen_rect.h, (screen_rect.w - 0.2) * screen_rect.w, (screen_rect.h - 0.35) * screen_rect.h);
        if list_rect.contains(touch.position) {
            let relative_y = touch.position.y - list_rect.y;
            let item_height = 0.08 * screen_rect.h;
            let item_index = (relative_y / item_height) as usize;
            
            if item_index < self.devices.len() {
                self.selected_devices[item_index] = !self.selected_devices[item_index];
            }
            return true;
        }

        // 检查确认按钮
        let button_y = screen_rect.bottom() - 0.1;
        let confirm_rect = Rect::new(screen_rect.center().x - 0.15, button_y, 0.14, 0.08);
        if self.confirm_btn.touch(touch, _t) {
            self.hide();
            return true;
        }

        // 检查取消按钮
        let cancel_rect = Rect::new(screen_rect.center().x + 0.01, button_y, 0.14, 0.08);
        if self.cancel_btn.touch(touch, _t) {
            self.hide();
            return true;
        }

        true
    }
}
