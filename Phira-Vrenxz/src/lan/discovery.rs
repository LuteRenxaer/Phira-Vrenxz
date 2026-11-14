//! 局域网服务器发现模块
//!
//! 使用 UDP 广播进行服务器发现

use crate::lan::protocol::{LanConfig, LanMessage};
use anyhow::{Context, Result};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

/// 服务器发现器
pub struct LanDiscovery {
    /// UDP socket
    socket: Arc<UdpSocket>,
    /// 发现间隔
    interval: Duration,
    /// 是否正在运行
    running: Arc<Mutex<bool>>,
    /// 发现到的服务器列表
    servers: Arc<Mutex<Vec<ServerInfo>>>,
}

/// 服务器信息
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// 服务器名称
    pub name: String,
    /// 房间ID
    pub room_id: u32,
    /// 房间名称
    pub room_name: String,
    /// 房主名称
    pub host_name: String,
    /// 房间人数
    pub player_count: u8,
    /// 最大人数
    pub max_players: u8,
    /// 是否等待其他玩家
    pub waiting_for_players: bool,
    /// 是否已开始
    pub started: bool,
    /// 服务器地址
    pub address: String,
}

impl LanDiscovery {
    /// 创建新的发现器
    pub fn new() -> Result<Self> {
        // 绑定随机端口，避免多个实例 / 多次创建时端口冲突
        let socket = UdpSocket::bind(("0.0.0.0", 0))
            .with_context(|| "Failed to bind UDP socket for LAN discovery")?;
        socket.set_broadcast(true)
            .with_context(|| "Failed to enable broadcast on UDP socket")?;
        // 设置读超时，使后台线程能周期性醒来检查退出标志并释放 socket
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));

        Ok(Self {
            socket: Arc::new(socket),
            interval: Duration::from_secs(2),
            running: Arc::new(Mutex::new(false)),
            servers: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// 开始发现服务器
    pub fn start_discovery(&self) -> std::thread::JoinHandle<()> {
        let socket = Arc::clone(&self.socket);
        let running = Arc::clone(&self.running);
        let servers = Arc::clone(&self.servers);
        let interval = self.interval;

        thread::spawn(move || {
            info!("Starting LAN server discovery...");
            *running.lock().unwrap() = true;

            while *running.lock().unwrap() {
                // 发送发现请求
                let discover_msg = LanMessage::Discover {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                };
                let msg_bytes = serde_json::to_string(&discover_msg)
                    .unwrap_or_else(|e| {
                        warn!("Failed to serialize discover message: {}", e);
                        String::new()
                    });

                if let Err(e) = socket.send_to(msg_bytes.as_bytes(), "255.255.255.255:27015") {
                    warn!("Failed to send discovery request: {}", e);
                } else {
                    info!("Sent LAN discovery broadcast (local socket {:?})", socket.local_addr().ok());
                }

                // 等待响应
                let mut buf = [0u8; 4096];
                match socket.recv_from(&mut buf) {
                    Ok((n, addr)) => {
                        let msg_str = String::from_utf8_lossy(&buf[..n]);
                        info!("Received {} bytes from {}: {}", n, addr, msg_str.trim());
                        if let Ok(msg) = serde_json::from_str::<LanMessage>(&msg_str) {
                            if let LanMessage::DiscoverResponse { name: server_name, room_id, room_name, host_name, player_count, max_players, waiting_for_players, started, address } = msg {
                                // 房主返回的 address 只含 TCP 端口，这里用 UDP 包的来源 IP 拼接成完整地址
                                let address = format!("{}:{}", addr.ip(), address);
                                let server = ServerInfo {
                                    name: server_name.clone(),
                                    room_id,
                                    room_name,
                                    host_name,
                                    player_count,
                                    max_players,
                                    waiting_for_players,
                                    started,
                                    address,
                                };

                                // 更新服务器列表
                                let mut servers_guard = servers.lock().unwrap();
                                // 移除旧的服务器（相同房间ID）
                                servers_guard.retain(|s| s.room_id != room_id);
                                // 添加新服务器
                                servers_guard.push(server);
                                info!("Discovered server '{}' (room {}) from {}", server_name, room_id, addr.ip());
                            }
                        }
                    }
                    Err(e) => {
                        // 读超时是用于周期性检查退出标志的正常现象，不打印
                        let timeout = matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut);
                        if !timeout {
                            warn!("Failed to receive discovery response: {}", e);
                        }
                    }
                }

                thread::sleep(interval);
            }

            info!("LAN server discovery stopped");
        })
    }

    /// 停止发现
    pub fn stop_discovery(&self) {
        *self.running.lock().unwrap() = false;
    }

    /// 获取发现到的服务器列表
    pub fn get_servers(&self) -> Vec<ServerInfo> {
        self.servers.lock().unwrap().clone()
    }

    /// 清空服务器列表
    pub fn clear_servers(&self) {
        self.servers.lock().unwrap().clear();
    }
}

/// 创建并启动服务器
pub fn start_server(config: LanConfig) -> Result<LanDiscovery> {
    let discovery = LanDiscovery::new()?;
    discovery.start_discovery();

    info!("LAN server started: {} (room: {})", config.server_name, config.room_name);

    Ok(discovery)
}
