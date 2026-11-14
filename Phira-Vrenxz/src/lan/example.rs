//! 局域网联机功能使用示例

use crate::lan::*;
use anyhow::Result;
use std::sync::{Arc, Mutex};

/// 局域网联机示例
pub struct LanExample {
    /// 管理器
    manager: Arc<Mutex<LanManager>>,
    /// UI面板
    panel: LanPanel,
    /// 等待面板
    waiting_panel: WaitingPanel,
    /// 音频设备面板
    audio_panel: AudioDevicePanel,
}

impl LanExample {
    /// 创建新的示例
    pub fn new() -> Self {
        let manager = Arc::new(Mutex::new(LanManager::new()));
        
        Self {
            manager: manager.clone(),
            panel: LanPanel::new(manager.clone()),
            waiting_panel: WaitingPanel::new(manager.clone()),
            audio_panel: AudioDevicePanel::new(),
        }
    }

    /// 初始化示例
    pub fn init(&mut self) -> Result<()> {
        // 注册消息处理器
        if let Ok(mut manager) = self.manager.lock() {
            // 注册房间成员处理器
            manager.register_handler("room_members".to_string(), |msg| {
                if let LanMessage::RoomMembers { members } = msg {
                    println!("Room members updated: {:?}", members);
                }
            });

            // 注册玩家准备状态处理器
            manager.register_handler("player_ready".to_string(), |msg| {
                if let LanMessage::PlayerReady { player_name, ready } = msg {
                    println!("Player {} ready: {}", player_name, ready);
                }
            });

            // 注册等待状态处理器
            manager.register_handler("waiting_status".to_string(), |msg| {
                if let LanMessage::WaitingStatus { waiting, ready_count, total_count } = msg {
                    println!("Waiting status: {} ({} / {})", waiting, ready_count, total_count);
                }
            });
        }

        Ok(())
    }

    /// 显示局域网联机面板
    pub fn show_panel(&mut self) {
        self.panel.show(0.0);
    }

    /// 更新示例
    pub fn update(&mut self, t: f32) -> Result<()> {
        // 更新面板
        self.panel.update(t)?;
        self.waiting_panel.update(t)?;
        self.audio_panel.update(t)?;

        // 检查状态变化
        if let Ok(manager) = self.manager.lock() {
            match manager.get_state() {
                LanState::InGame { waiting_for_players, ready_players, room_id: _ } => {
                    if waiting_for_players {
                        // 显示等待面板
                        self.waiting_panel.show(0.0, ready_players.len() as u8);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// 渲染示例
    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        // 渲染主面板
        self.panel.render(ui, t);
        
        // 渲染等待面板
        self.waiting_panel.render(ui, t);
        
        // 渲染音频设备面板
        self.audio_panel.render(ui, t);
    }

    /// 处理触摸输入
    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        // 处理主面板触摸
        if self.panel.touch(touch, t) {
            return true;
        }

        // 处理等待面板触摸
        if self.waiting_panel.touch(touch, t) {
            return true;
        }

        // 处理音频设备面板触摸
        if self.audio_panel.touch(touch, t) {
            return true;
        }

        false
    }

    /// 创建房间
    pub fn create_room(&mut self) -> Result<()> {
        if let Ok(mut manager) = self.manager.lock() {
            let config = LanConfig {
                server_name: "My Room".to_string(),
                room_name: "Test Room".to_string(),
                max_players: 4,
                waiting_for_players: false,
                local_ip: "0.0.0.0".to_string(),
                tcp_port: 27016,
            };
            
            manager.create_room(config)?;
        }
        Ok(())
    }

    /// 加入房间
    pub fn join_room(&mut self, server_addr: String) -> Result<()> {
        if let Ok(mut manager) = self.manager.lock() {
            manager.join_room(server_addr, "Player".to_string())?;
        }
        Ok(())
    }

    /// 准备就绪
    pub fn ready(&mut self, ready: bool) -> Result<()> {
        if let Ok(mut manager) = self.manager.lock() {
            manager.ready(ready)?;
        }
        Ok(())
    }

    /// 开始游戏
    pub fn start_game(&mut self, waiting_for_players: bool) -> Result<()> {
        if let Ok(mut manager) = self.manager.lock() {
            manager.start_game(waiting_for_players)?;
        }
        Ok(())
    }

    /// 更新音频设备设置
    pub fn update_audio_devices(&mut self, devices: Vec<AudioDeviceInfo>, selected_index: usize) -> Result<()> {
        if let Ok(mut manager) = self.manager.lock() {
            manager.update_audio_devices(devices, selected_index)?;
        }
        Ok(())
    }

    /// 请求下载谱面
    pub fn request_download_chart(&mut self, chart_id: String, chart_name: String) -> Result<()> {
        if let Ok(mut manager) = self.manager.lock() {
            manager.request_download_chart(chart_id, chart_name)?;
        }
        Ok(())
    }

    /// 断开连接
    pub fn disconnect(&mut self) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.disconnect();
        }
    }
}

/// 使用示例
pub fn example_usage() {
    let mut example = LanExample::new();
    
    // 初始化
    if let Err(e) = example.init() {
        println!("Failed to initialize example: {}", e);
        return;
    }

    // 显示面板
    example.show_panel();

    // 游戏循环
    loop {
        // 更新
        if let Err(e) = example.update(0.0) {
            println!("Update error: {}", e);
            break;
        }

        // 渲染
        // 这里需要实际的UI上下文
        // example.render(ui, 0.0);

        // 处理输入
        // 这里需要实际的触摸输入
        // example.touch(&touch, 0.0);

        // 其他游戏逻辑...
    }
}
