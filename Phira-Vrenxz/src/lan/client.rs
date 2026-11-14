//! 局域网客户端
//!
//! 处理与局域网服务器的通信

use crate::lan::protocol::{LanConfig, LanMessage, RoomInfo, MemberInfo, AudioDeviceInfo};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream as AsyncTcpStream;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// 局域网客户端
pub struct LanClient {
    /// 服务器地址
    server_addr: String,
    /// TCP连接
    connection: Option<TcpStream>,
    /// 房间信息
    room_info: Arc<Mutex<Option<RoomInfo>>>,
    /// 成员列表
    members: Arc<Mutex<Vec<MemberInfo>>>,
    /// 是否已开始
    started: Arc<Mutex<bool>>,
    /// 是否等待其他玩家
    waiting_for_players: Arc<Mutex<bool>>,
    /// 发声设备列表
    audio_devices: Arc<Mutex<Vec<AudioDeviceInfo>>>,
    /// 当前选择的设备
    selected_device: Arc<Mutex<usize>>,
    /// 消息处理器
    message_handlers: Arc<Mutex<HashMap<String, Box<dyn Fn(LanMessage) + Send + Sync>>>>,
    /// 运行标志
    running: Arc<Mutex<bool>>,
    /// 是否已断开（服务端关闭连接/房主离开）
    disconnected: Arc<std::sync::atomic::AtomicBool>,
    /// 房主开始游戏时携带的谱面路径
    started_chart: Arc<Mutex<Option<String>>>,
    /// 房主谱面下载服务器地址
    server_address: Arc<Mutex<Option<String>>>,
    /// 房主谱面的在线 id
    chart_id: Arc<Mutex<Option<i32>>>,
    /// 是否已收到“所有人开始游玩”信号
    start_playing: Arc<Mutex<bool>>,
}

impl LanClient {
    /// 创建新的客户端
    pub fn new(server_addr: String) -> Self {
        Self {
            server_addr,
            connection: None,
            room_info: Arc::new(Mutex::new(None)),
            members: Arc::new(Mutex::new(Vec::new())),
            started: Arc::new(Mutex::new(false)),
            waiting_for_players: Arc::new(Mutex::new(false)),
            audio_devices: Arc::new(Mutex::new(Vec::new())),
            selected_device: Arc::new(Mutex::new(0)),
            message_handlers: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
            disconnected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            started_chart: Arc::new(Mutex::new(None)),
            server_address: Arc::new(Mutex::new(None)),
            chart_id: Arc::new(Mutex::new(None)),
            start_playing: Arc::new(Mutex::new(false)),
        }
    }

    /// 连接到服务器
    pub fn connect(&mut self) -> Result<()> {
        let stream = TcpStream::connect(&self.server_addr)
            .with_context(|| format!("Failed to connect to server: {}", self.server_addr))?;
        
        self.connection = Some(stream);
        *self.running.lock().unwrap() = true;

        // 启动消息接收线程
        let mut connection = self.connection.as_ref().unwrap().try_clone()?;
        let room_info = Arc::clone(&self.room_info);
        let members = Arc::clone(&self.members);
        let started = Arc::clone(&self.started);
        let waiting_for_players = Arc::clone(&self.waiting_for_players);
        let audio_devices = Arc::clone(&self.audio_devices);
        let selected_device = Arc::clone(&self.selected_device);
        let message_handlers = Arc::clone(&self.message_handlers);
        let running = Arc::clone(&self.running);
        let disconnected = Arc::clone(&self.disconnected);
        let started_chart = Arc::clone(&self.started_chart);
        let server_address = Arc::clone(&self.server_address);
        let chart_id = Arc::clone(&self.chart_id);
        let start_playing = Arc::clone(&self.start_playing);

        thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let mut reader = BufReader::new(connection);
            let mut line = String::new();

            while *running.lock().unwrap() {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        warn!("Server disconnected");
                        disconnected.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(msg) = serde_json::from_str::<LanMessage>(trimmed) {
                            Self::handle_message(msg, &room_info, &members, &started, &waiting_for_players, &audio_devices, &selected_device, &message_handlers, &started_chart, &server_address, &chart_id, &start_playing);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read from server: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// 断开连接
    pub fn disconnect(&mut self) {
        *self.running.lock().unwrap() = false;
        self.connection = None;
    }

    /// 发送消息（每条消息以换行分隔，便于服务端逐行解析）
    pub fn send_message(&mut self, msg: LanMessage) -> Result<()> {
        if let Some(ref mut stream) = self.connection {
            let msg_str = serde_json::to_string(&msg)?;
            stream.write_all(msg_str.as_bytes())?;
            stream.write_all(b"\n")?;
            debug!("Sent message: {:?}", msg);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Not connected to server"))
        }
    }

    /// 处理接收到的消息
    fn handle_message(
        msg: LanMessage,
        room_info: &Arc<Mutex<Option<RoomInfo>>>,
        members: &Arc<Mutex<Vec<MemberInfo>>>,
        started: &Arc<Mutex<bool>>,
        waiting_for_players: &Arc<Mutex<bool>>,
        audio_devices: &Arc<Mutex<Vec<AudioDeviceInfo>>>,
        selected_device: &Arc<Mutex<usize>>,
        message_handlers: &Arc<Mutex<HashMap<String, Box<dyn Fn(LanMessage) + Send + Sync>>>>,
        started_chart: &Arc<Mutex<Option<String>>>,
        server_address: &Arc<Mutex<Option<String>>>,
        chart_id: &Arc<Mutex<Option<i32>>>,
        start_playing: &Arc<Mutex<bool>>,
    ) {
        // 提前计算消息类型名称（避免部分移动后无法借用）
        let type_name = msg.type_name();
        // 提前克隆消息用于调用处理器（match 分支会部分移动 msg）
        let handler_msg = msg.clone();

        match msg {
            LanMessage::JoinResponse { success, error, room_info: room } => {
                if success {
                    *room_info.lock().unwrap() = room;
                    info!("Successfully joined room");
                } else {
                    warn!("Failed to join room: {:?}", error);
                }
            }

            LanMessage::RoomMembers { members: member_list } => {
                let count = member_list.len();
                *members.lock().unwrap() = member_list;
                debug!("Updated member list: {} members", count);
            }

            LanMessage::StartGame { waiting_for_players: wait_flag, chart_path, server_addr, chart_id: host_chart_id } => {
                *started.lock().unwrap() = true;
                *waiting_for_players.lock().unwrap() = wait_flag;
                *started_chart.lock().unwrap() = chart_path.clone();
                *server_address.lock().unwrap() = server_addr.clone();
                *chart_id.lock().unwrap() = host_chart_id;
                if let Some(path) = chart_path {
                    info!("Game started, host chart: {} from {}", path, server_addr.as_deref().unwrap_or("?"));
                } else {
                    info!("Game started");
                }
            }

            LanMessage::StartPlaying => {
                *start_playing.lock().unwrap() = true;
                info!("All synced, start playing");
            }

            LanMessage::AudioDevices { devices, selected_index } => {
                let count = devices.len();
                *audio_devices.lock().unwrap() = devices;
                *selected_device.lock().unwrap() = selected_index;
                debug!("Updated audio devices: {} devices", count);
            }

            LanMessage::DownloadProgress { progress } => {
                info!("Download progress: {}%", progress);
            }

            LanMessage::DownloadComplete { chart_id } => {
                info!("Download completed: {}", chart_id);
            }

            LanMessage::PlayerReady { player_name, ready } => {
                debug!("Player ready: {} - {}", player_name, ready);
                // 更新成员列表
                let mut members_guard = members.lock().unwrap();
                for member in members_guard.iter_mut() {
                    if member.name == player_name {
                        member.ready = ready;
                        break;
                    }
                }
            }

            LanMessage::WaitingStatus { waiting, ready_count, total_count } => {
                debug!("Waiting status: {} ({} / {})", waiting, ready_count, total_count);
                *waiting_for_players.lock().unwrap() = waiting;
            }

            _ => {
                warn!("Unhandled message type: {:?}", msg);
            }
        }

        // 调用注册的消息处理器
        if let Some(handler) = message_handlers.lock().unwrap().get(&type_name) {
            handler(handler_msg);
        }
    }

    /// 注册消息处理器
    pub fn register_handler<F>(&self, msg_type: String, handler: F)
    where
        F: Fn(LanMessage) + Send + Sync + 'static,
    {
        self.message_handlers.lock().unwrap().insert(msg_type, Box::new(handler));
    }

    /// 获取房间信息
    pub fn get_room_info(&self) -> Option<RoomInfo> {
        self.room_info.lock().unwrap().clone()
    }

    /// 获取成员列表
    pub fn get_members(&self) -> Vec<MemberInfo> {
        self.members.lock().unwrap().clone()
    }

    /// 获取是否已开始
    pub fn is_started(&self) -> bool {
        *self.started.lock().unwrap()
    }

    /// 获取是否等待其他玩家
    pub fn is_waiting_for_players(&self) -> bool {
        *self.waiting_for_players.lock().unwrap()
    }

    /// 是否已断开（服务端关闭连接/房主离开）
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 房主开始游戏时携带的谱面路径
    pub fn get_started_chart(&self) -> Option<String> {
        self.started_chart.lock().unwrap().clone()
    }

    /// 重置开始状态（游玩结束后调用，避免返回时再次自动触发）
    pub fn reset_started(&mut self) {
        *self.started.lock().unwrap() = false;
        *self.started_chart.lock().unwrap() = None;
        *self.server_address.lock().unwrap() = None;
    }

    /// 房主谱面下载服务器地址
    pub fn get_server_address(&self) -> Option<String> {
        self.server_address.lock().unwrap().clone()
    }

    /// 是否已收到“所有人开始游玩”信号
    pub fn get_start_playing(&self) -> bool {
        *self.start_playing.lock().unwrap()
    }

    /// 房主谱面的在线 id
    pub fn get_chart_id(&self) -> Option<i32> {
        *self.chart_id.lock().unwrap()
    }

    /// 获取音频设备列表
    pub fn get_audio_devices(&self) -> Vec<AudioDeviceInfo> {
        self.audio_devices.lock().unwrap().clone()
    }

    /// 获取当前选择的设备
    pub fn get_selected_device(&self) -> usize {
        *self.selected_device.lock().unwrap()
    }

    /// 设置当前选择的设备
    pub fn set_selected_device(&mut self, index: usize) -> Result<()> {
        *self.selected_device.lock().unwrap() = index;
        
        // 发送设备设置更新
        let devices = self.audio_devices.lock().unwrap().clone();
        let msg = LanMessage::AudioDevices {
            devices,
            selected_index: index,
        };
        self.send_message(msg)
    }
}

// 为 LanMessage 添加 type_name 方法
impl LanMessage {
    pub fn type_name(&self) -> String {
        match self {
            LanMessage::Discover { .. } => "discover".to_string(),
            LanMessage::DiscoverResponse { .. } => "discover_response".to_string(),
            LanMessage::Join { .. } => "join".to_string(),
            LanMessage::JoinResponse { .. } => "join_response".to_string(),
            LanMessage::RoomMembers { .. } => "room_members".to_string(),
            LanMessage::Ready { .. } => "ready".to_string(),
            LanMessage::StartGame { .. } => "start_game".to_string(),
            LanMessage::SyncReady { .. } => "sync_ready".to_string(),
            LanMessage::StartPlaying => "start_playing".to_string(),
            LanMessage::AudioDevices { .. } => "audio_devices".to_string(),
            LanMessage::DownloadChart { .. } => "download_chart".to_string(),
            LanMessage::DownloadChartResponse { .. } => "download_chart_response".to_string(),
            LanMessage::DownloadProgress { .. } => "download_progress".to_string(),
            LanMessage::DownloadComplete { .. } => "download_complete".to_string(),
            LanMessage::PlayerReady { .. } => "player_ready".to_string(),
            LanMessage::WaitingStatus { .. } => "waiting_status".to_string(),
        }
    }
}
