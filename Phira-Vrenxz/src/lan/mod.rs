//! 局域网联机模块

pub mod protocol;
pub mod discovery;
pub mod server;
pub mod client;
pub mod ui;
pub mod file_server;
pub mod waiting_ui;

pub use protocol::*;
pub use discovery::*;
pub use server::*;
pub use client::*;
pub use ui::*;
pub use file_server::*;
pub use waiting_ui::*;

use std::net::{TcpListener, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// 获取本机局域网 IP（用于文件下载服务器地址）。
/// 逐个尝试连接候选地址，通过 UDP 路由选择获得本机 IP；全部失败时返回 None。
fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    for target in ["8.8.8.8:80", "1.1.1.1:80", "192.168.1.1:1", "255.255.255.255:9"] {
        if socket.connect(target).is_ok() {
            if let Ok(a) = socket.local_addr() {
                let ip = a.ip().to_string();
                if ip != "0.0.0.0" {
                    return Some(ip);
                }
            }
        }
    }
    None
}

/// 在 TCP 端口范围内随机选取一个可用端口
fn pick_free_port() -> anyhow::Result<u16> {
    let count = (LAN_TCP_PORT_MAX - LAN_TCP_PORT_MIN + 1) as usize;
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    let start = seed % count;
    for i in 0..count {
        let port = LAN_TCP_PORT_MIN + ((start + i) % count) as u16;
        // 尝试绑定该端口，成功即代表可用（绑定后立即释放）
        if TcpListener::bind(("0.0.0.0", port)).is_ok() {
            return Ok(port);
        }
    }
    anyhow::bail!("No free TCP port available in range {}..={}", LAN_TCP_PORT_MIN, LAN_TCP_PORT_MAX)
}

/// 房主端 UDP 发现响应：监听广播端口，收到 Discover 请求即回 DiscoverResponse，
/// 使同一局域网内的其它客户端（包括本机多开）能搜索到本房间。
fn start_discovery_response(config: LanConfig, host_name: String) -> anyhow::Result<()> {
    use anyhow::Context;
    let socket = UdpSocket::bind(("0.0.0.0", LAN_BROADCAST_PORT))
        .with_context(|| "Failed to bind UDP discovery listener")?;
    socket.set_broadcast(true)?;
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            if let Ok((n, addr)) = socket.recv_from(&mut buf) {
                let msg_str = String::from_utf8_lossy(&buf[..n]);
                if let Ok(LanMessage::Discover { .. }) = serde_json::from_str(&msg_str) {
                    info!("Discovery request from {}, responding for room '{}' (tcp port {})", addr, config.room_name, config.tcp_port);
                    let response = LanMessage::DiscoverResponse {
                        name: config.server_name.clone(),
                        room_id: config.tcp_port as u32,
                        room_name: config.room_name.clone(),
                        host_name: host_name.clone(),
                        player_count: 1,
                        max_players: config.max_players,
                        waiting_for_players: config.waiting_for_players,
                        started: false,
                        // address 只放 TCP 端口，客户端会用 UDP 包的来源 IP 拼接
                        address: config.tcp_port.to_string(),
                    };
                    if let Ok(s) = serde_json::to_string(&response) {
                        if let Err(e) = socket.send_to(s.as_bytes(), addr) {
                            warn!("Failed to send discovery response to {}: {}", addr, e);
                        } else {
                            info!("Sent discovery response to {} (tcp port {})", addr, config.tcp_port);
                        }
                    }
                }
            }
        }
    });
    Ok(())
}

/// 局域网联机管理器
pub struct LanManager {
    /// 服务器发现器
    discovery: Option<LanDiscovery>,
    /// 房间服务器
    server: Option<LanServer>,
    /// 客户端
    client: Option<LanClient>,
    /// 当前状态
    state: Arc<Mutex<LanState>>,
    /// 自己的玩家名称
    my_name: String,
    /// 当前联机谱面路径（房主设定，用于同步）
    my_chart_path: Option<String>,
    /// 房主谱面的在线 id（用于成员本地查重）
    chart_id: Option<i32>,
    /// 房主谱面下载服务器
    file_server: Option<FileServer>,
}

/// 局域网联机状态
#[derive(Debug, Clone)]
pub enum LanState {
    /// 未连接
    Disconnected,
    /// 正在发现服务器
    Discovering,
    /// 已连接到房间
    Connected {
        room_id: u32,
        is_host: bool,
        members: Vec<String>,
    },
    /// 游戏中
    InGame {
        room_id: u32,
        waiting_for_players: bool,
        ready_players: Vec<String>,
    },
}

impl LanManager {
    /// 创建新的管理器
    pub fn new() -> Self {
        Self {
            discovery: None,
            server: None,
            client: None,
            state: Arc::new(Mutex::new(LanState::Disconnected)),
            my_name: String::new(),
            my_chart_path: None,
            chart_id: None,
            file_server: None,
        }
    }

    /// 开始发现服务器
    pub fn start_discovery(&mut self) -> Result<(), anyhow::Error> {
        // 先停止旧的 discovery。旧线程用独立的 running 标志，会在下一次读超时后退出；
        // discovery 使用随机端口，重新创建不会与旧 socket 冲突。
        if let Some(old) = self.discovery.take() {
            old.stop_discovery();
        }
        let discovery = LanDiscovery::new()?;
        discovery.start_discovery();
        self.discovery = Some(discovery);
        *self.state.lock().unwrap() = LanState::Discovering;
        Ok(())
    }

    /// 停止发现服务器（仅停止发现广播，保留房间/招募状态与监听，除非主动离开房间）
    pub fn stop_discovery(&mut self) {
        if let Some(discovery) = &self.discovery {
            discovery.stop_discovery();
        }
        // 注意：不再把 state 重置为 Disconnected，否则关闭面板后房主状态会丢失。
        // 只有调用 disconnect() 才会真正断开并重置状态。
    }

    /// 创建房间
    pub fn create_room(&mut self, mut config: LanConfig, host_name: String) -> Result<(), anyhow::Error> {
        self.my_name = host_name.clone();
        // 随机选取一个可用端口，避免与其它房间冲突
        config.tcp_port = pick_free_port()?;
        let server = LanServer::new(config.clone())?;
        server.start()?;
        self.server = Some(server);
        // 启动 UDP 发现响应，使其它客户端能搜索到本房间（不受系统代理影响）
        start_discovery_response(config.clone(), host_name)?;
        *self.state.lock().unwrap() = LanState::Connected {
            room_id: config.tcp_port as u32,
            is_host: true,
            members: Vec::new(),
        };
        Ok(())
    }

    /// 加入房间
    pub fn join_room(&mut self, server_addr: String, player_name: String) -> Result<(), anyhow::Error> {
        self.my_name = player_name.clone();
        let mut client = LanClient::new(server_addr);
        client.connect()?;
        
        // 发送加入请求
        client.send_message(LanMessage::Join {
            room_id: 0, // 简化处理
            player_name,
        })?;

        self.client = Some(client);
        *self.state.lock().unwrap() = LanState::Connected {
            room_id: 0, // 简化处理
            is_host: false,
            members: Vec::new(),
        };
        Ok(())
    }

    /// 获取发现到的服务器列表
    pub fn get_servers(&self) -> Vec<ServerInfo> {
        if let Some(discovery) = &self.discovery {
            discovery.get_servers()
        } else {
            Vec::new()
        }
    }

    /// 获取当前状态
    pub fn get_state(&self) -> LanState {
        self.state.lock().unwrap().clone()
    }

    /// 获取成员列表（房主场景包含房主自己；玩家场景来自客户端同步）
    pub fn get_members(&self) -> Vec<MemberInfo> {
        if let Some(server) = &self.server {
            let mut list = Vec::new();
            list.push(MemberInfo {
                name: self.my_name.clone(),
                is_host: true,
                ready: false,
                started: server.is_started(),
            });
            list.extend(server.get_members());
            list
        } else if let Some(client) = &self.client {
            client.get_members()
        } else {
            Vec::new()
        }
    }

    /// 我的玩家名称
    pub fn my_name(&self) -> &str {
        &self.my_name
    }

    /// 是否为成员（已加入他人房间的客户端）
    pub fn is_member(&self) -> bool {
        self.client.is_some() && self.server.is_none()
    }

    /// 是否已开始游戏
    pub fn is_host(&self) -> bool {
        self.server.is_some()
    }

    pub fn is_started(&self) -> bool {        if matches!(*self.state.lock().unwrap(), LanState::InGame { .. }) {
            return true;
        }
        if let Some(client) = &self.client {
            client.is_started()
        } else if let Some(server) = &self.server {
            server.is_started()
        } else {
            false
        }
    }

    /// 开始游戏时房主的谱面路径（成员据此判断是否同一谱面）
    pub fn started_chart(&self) -> Option<String> {
        if let Some(client) = &self.client {
            client.get_started_chart()
        } else if let Some(server) = &self.server {
            self.my_chart_path.clone()
        } else {
            None
        }
    }

    /// 准备就绪
    pub fn ready(&mut self, ready: bool) -> Result<(), anyhow::Error> {
        if let Some(client) = &mut self.client {
            client.send_message(LanMessage::Ready { ready })?;
        }
        Ok(())
    }

    /// 设置当前联机谱面路径（由谱面预览界面调用）
    pub fn set_chart_path(&mut self, path: Option<String>) {
        self.my_chart_path = path;
    }

    /// 设置当前联机谱面的在线 id（用于成员本地查重）
    pub fn set_chart_id(&mut self, id: Option<i32>) {
        self.chart_id = id;
    }

    /// 成员端：房主谱面的在线 id
    pub fn started_chart_id(&self) -> Option<i32> {
        if let Some(client) = &self.client {
            client.get_chart_id()
        } else {
            self.chart_id
        }
    }

    /// 重置开始状态（游玩结束后调用，避免返回时再次自动触发开始）
    pub fn reset_start(&mut self) {
        if self.server.is_some() {
            // 房主：状态回到已连接（房间仍保留）
            *self.state.lock().unwrap() = LanState::Connected {
                room_id: 0,
                is_host: true,
                members: Vec::new(),
            };
        }
        if let Some(client) = &mut self.client {
            client.reset_started();
        }
    }

    /// 开始游戏：房主启动谱面下载服务器并广播，成员从房主设备下载谱面
    pub fn start_game(&mut self, waiting_for_players: bool, chart_path: Option<String>) -> Result<(), anyhow::Error> {
        if let Some(client) = &mut self.client {
            client.send_message(LanMessage::StartGame { waiting_for_players, chart_path: None, server_addr: None, chart_id: None })?;
        }
        if let Some(server) = &self.server {
            info!("Host starting game (waiting_for_players={})", waiting_for_players);
            // 启动谱面下载服务器，供成员从房主设备下载当前谱面
            let mut chart_id = None;
            let mut server_addr = None;
            if let Some(path) = &chart_path {
                let uuid = uuid::Uuid::new_v4().to_string();
                if let Ok(()) = crate::mp::serve::stage_local_chart(path, &uuid) {
                    match FileServer::new(0, crate::dir::charts()?) {
                        Ok(fs) => {
                            fs.set_current_chart(uuid.clone());
                            if fs.start().is_ok() {
                                let port = fs.port();
                                self.file_server = Some(fs);
                                chart_id = Some(uuid);
                                if let Some(ip) = get_local_ip() {
                                    server_addr = Some(format!("{}:{}", ip, port));
                                    info!("Chart download server at {}:{} (chart {})", ip, port, chart_id.as_deref().unwrap_or(""));
                                } else {
                                    warn!("Failed to resolve local IP, members cannot download chart");
                                }
                            }
                        }
                        Err(e) => warn!("Failed to start file server: {}", e),
                    }
                } else {
                    warn!("Failed to stage local chart: {}", path);
                }
            }
            server.broadcast_start(waiting_for_players, chart_id, server_addr, self.chart_id)?;
            *self.state.lock().unwrap() = LanState::InGame {
                room_id: 0,
                waiting_for_players,
                ready_players: Vec::new(),
            };
        }
        Ok(())
    }

    /// 成员端：通知房主本成员已同步就绪（谱面可用，无需下载或已下载）
    pub fn notify_sync_ready(&mut self) {
        let my_name = self.my_name.clone();
        if let Some(client) = &mut self.client {
            let _ = client.send_message(LanMessage::SyncReady { player_name: my_name });
        }
    }

    /// 成员端：从房主设备下载谱面到本地，下载完成后通知房主同步就绪
    pub fn download_host_chart(&mut self) -> anyhow::Result<String> {
        let client = self.client.as_ref().ok_or_else(|| anyhow::anyhow!("尚未加入房间"))?;
        let chart_id = client.get_started_chart().ok_or_else(|| anyhow::anyhow!("未收到房主谱面信息"))?;
        let server_addr = client.get_server_address().ok_or_else(|| anyhow::anyhow!("未收到房主下载地址"))?;
        info!("Member downloading chart {} from {}", chart_id, server_addr);
        let extract_dir = format!("{}/download/{}", crate::dir::charts()?, chart_id);
        std::fs::create_dir_all(&extract_dir)?;
        let downloader = FileDownloader::new(server_addr, Box::new(|p| debug!("下载进度: {}%", p)));
        downloader.download_chart(&chart_id, &extract_dir)?;
        // 通知房主本成员已同步就绪
        let my_name = self.my_name.clone();
        if let Some(client) = &mut self.client {
            let _ = client.send_message(LanMessage::SyncReady { player_name: my_name });
        }
        Ok(chart_id)
    }

    /// 房主：是否所有成员都已就绪。
    /// 开启“等待其他玩家”时需所有成员点“准备”；否则仅需所有成员完成谱面下载。
    pub fn host_all_ready(&self) -> bool {
        if let Some(server) = &self.server {
            if server.is_waiting() {
                server.all_members_ready()
            } else {
                let count = server.members_count();
                count > 0 && server.sync_ready_count() >= count
            }
        } else {
            false
        }
    }

    /// 房主：广播开始游玩给所有成员
    pub fn host_start_playing(&mut self) -> anyhow::Result<()> {
        if let Some(server) = &self.server {
            server.broadcast_start_playing()?;
        }
        Ok(())
    }

    /// 成员：是否已收到“所有人开始游玩”信号
    pub fn member_start_playing(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.get_start_playing())
    }

    /// 房主是否开启“等待其他玩家”（成员据此决定是否需点准备）
    pub fn is_waiting(&self) -> bool {
        self.client
            .as_ref()
            .is_some_and(|c| c.is_waiting_for_players())
    }

    /// 更新发声设备设置
    pub fn update_audio_devices(&mut self, devices: Vec<AudioDeviceInfo>, selected_index: usize) -> Result<(), anyhow::Error> {
        if let Some(client) = &mut self.client {
            client.send_message(LanMessage::AudioDevices { devices, selected_index })?;
        }
        Ok(())
    }

    /// 获取当前音频设备设置
    pub fn get_audio_devices(&self) -> Vec<AudioDeviceInfo> {
        if let Some(client) = &self.client {
            client.get_audio_devices()
        } else {
            Vec::new()
        }
    }

    /// 请求下载谱面
    pub fn request_download_chart(&mut self, chart_id: String, chart_name: String) -> Result<(), anyhow::Error> {
        if let Some(client) = &mut self.client {
            client.send_message(LanMessage::DownloadChart { chart_id, chart_name })?;
        }
        Ok(())
    }

    /// 断开连接（房主离开时同时关闭服务器并断开所有成员）
    pub fn disconnect(&mut self) {
        if let Some(mut client) = self.client.take() {
            client.disconnect();
        }
        if let Some(server) = self.server.take() {
            server.close();
        }
        *self.state.lock().unwrap() = LanState::Disconnected;
    }

    /// 检测到服务端断开（房主离开/房间关闭）时，自动退出房间
    pub fn check_disconnected(&mut self) {
        if let Some(client) = &self.client {
            if client.is_disconnected() {
                self.client.take();
                *self.state.lock().unwrap() = LanState::Disconnected;
            }
        }
    }
}
