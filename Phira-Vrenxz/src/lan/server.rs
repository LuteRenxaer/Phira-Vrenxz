//! 局域网房间服务器
//!
//! 使用 TCP 处理房间通信

use crate::lan::protocol::{LanConfig, LanMessage, RoomInfo, MemberInfo};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, info, warn};

/// 房间服务器
pub struct LanServer {
    /// 配置
    config: LanConfig,
    /// TCP监听器
    listener: TcpListener,
    /// 房间成员（名称 -> 信息）
    members: Arc<Mutex<HashMap<String, MemberInfo>>>,
    /// 客户端连接池（成员名 -> TCP连接），用于向所有成员广播
    clients: Arc<Mutex<Vec<(String, TcpStream)>>>,
    /// 已同步就绪的成员名（下载完成）
    sync_ready: Arc<Mutex<Vec<String>>>,
    /// 是否已开始
    started: Arc<Mutex<bool>>,
    /// 是否等待其他玩家
    waiting_for_players: Arc<Mutex<bool>>,
    /// 发声设备设置
    audio_devices: Arc<Mutex<Vec<String>>>,
    /// 当前选择的设备
    selected_device: Arc<Mutex<usize>>,
    /// 服务器运行标志
    running: Arc<Mutex<bool>>,
}

impl LanServer {
    /// 创建新的房间服务器
    pub fn new(config: LanConfig) -> Result<Self> {
        let listener = TcpListener::bind(format!("{}:{}", config.local_ip, config.tcp_port))
            .with_context(|| format!("Failed to bind TCP listener on {}:{}", config.local_ip, config.tcp_port))?;

        Ok(Self {
            config,
            listener,
            members: Arc::new(Mutex::new(HashMap::new())),
            clients: Arc::new(Mutex::new(Vec::new())),
            sync_ready: Arc::new(Mutex::new(Vec::new())),
            started: Arc::new(Mutex::new(false)),
            waiting_for_players: Arc::new(Mutex::new(false)),
            audio_devices: Arc::new(Mutex::new(Vec::new())),
            selected_device: Arc::new(Mutex::new(0)),
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// 启动服务器
    pub fn start(&self) -> Result<std::thread::JoinHandle<()>> {
        *self.running.lock().unwrap() = true;

        let members = Arc::clone(&self.members);
        let clients = Arc::clone(&self.clients);
        let sync_ready = Arc::clone(&self.sync_ready);
        let started = Arc::clone(&self.started);
        let waiting_for_players = Arc::clone(&self.waiting_for_players);
        let audio_devices = Arc::clone(&self.audio_devices);
        let selected_device = Arc::clone(&self.selected_device);
        let running = Arc::clone(&self.running);
        let listener = self.listener.try_clone()
            .with_context(|| "Failed to clone TCP listener")?;
        let config = self.config.clone();

        Ok(thread::spawn(move || {
            info!("LAN server started on {}:{}", config.local_ip, config.tcp_port);

            while *running.lock().unwrap() {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        let members = Arc::clone(&members);
                        let clients = Arc::clone(&clients);
                        let sync_ready = Arc::clone(&sync_ready);
                        let started = Arc::clone(&started);
                        let waiting_for_players = Arc::clone(&waiting_for_players);
                        let audio_devices = Arc::clone(&audio_devices);
                        let selected_device = Arc::clone(&selected_device);

                        thread::spawn(move || {
                            if let Err(e) = Self::handle_client(stream, addr, members, clients, sync_ready, started, waiting_for_players, audio_devices, selected_device) {
                                warn!("Failed to handle client {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Failed to accept connection: {}", e);
                    }
                }
            }

            info!("LAN server stopped");
        }))
    }

    /// 停止服务器
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    /// 关闭服务器并断开所有已连接的成员（房主离开时调用）
    pub fn close(&self) {
        *self.running.lock().unwrap() = false;
        // 关闭所有客户端连接，使成员客户端能检测到断开并退出房间
        self.clients.lock().unwrap().clear();
    }

    /// 处理客户端连接
    fn handle_client(
        mut stream: TcpStream,
        addr: std::net::SocketAddr,
        members: Arc<Mutex<HashMap<String, MemberInfo>>>,
        clients: Arc<Mutex<Vec<(String, TcpStream)>>>,
        sync_ready: Arc<Mutex<Vec<String>>>,
        started: Arc<Mutex<bool>>,
        waiting_for_players: Arc<Mutex<bool>>,
        audio_devices: Arc<Mutex<Vec<String>>>,
        selected_device: Arc<Mutex<usize>>,
    ) -> Result<()> {
        // 逐行读取（每条消息以换行分隔）
        let mut reader = BufReader::new(stream.try_clone()?);
        // 当前连接对应的成员名（加入房间后设置），断开时用于从连接池移除
        let mut my_name: Option<String> = None;
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    info!("Client disconnected: {}", addr);
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(msg) = serde_json::from_str::<LanMessage>(trimmed) {
                        if let Some(name) = Self::handle_message(
                            msg,
                            &mut stream,
                            &addr,
                            members.clone(),
                            clients.clone(),
                            sync_ready.clone(),
                            started.clone(),
                            waiting_for_players.clone(),
                            audio_devices.clone(),
                            selected_device.clone(),
                            &mut my_name,
                        )? {
                            my_name = Some(name);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read from client {}: {}", addr, e);
                    break;
                }
            }
        }

        // 断开连接：从成员列表和连接池中移除
        if let Some(name) = my_name {
            members.lock().unwrap().remove(&name);
            clients.lock().unwrap().retain(|(n, _)| n != &name);
        }

        Ok(())
    }

    /// 处理消息（加入房间时返回新成员名，用于登记连接池）
    fn handle_message(
        msg: LanMessage,
        stream: &mut TcpStream,
        addr: &std::net::SocketAddr,
        members: Arc<Mutex<HashMap<String, MemberInfo>>>,
        clients: Arc<Mutex<Vec<(String, TcpStream)>>>,
        sync_ready: Arc<Mutex<Vec<String>>>,
        started: Arc<Mutex<bool>>,
        waiting_for_players: Arc<Mutex<bool>>,
        audio_devices: Arc<Mutex<Vec<String>>>,
        selected_device: Arc<Mutex<usize>>,
        my_name: &mut Option<String>,
    ) -> Result<Option<String>> {
        match msg {
            LanMessage::Join { room_id, player_name } => {
                debug!("Join request from {} (room: {})", player_name, room_id);

                let wait_flag = *waiting_for_players.lock().unwrap();
                let started_flag = *started.lock().unwrap();
                let room_info = RoomInfo {
                    room_id,
                    room_name: "My Room".to_string(),
                    host_name: "Host".to_string(),
                    max_players: 4,
                    waiting_for_players: wait_flag,
                    started: started_flag,
                };

                // 通过 TCP 加入的均为玩家（房主不在此连接池中）
                {
                    let mut members_guard = members.lock().unwrap();
                    let member = MemberInfo {
                        name: player_name.clone(),
                        is_host: false,
                        ready: false,
                        started: false,
                    };
                    members_guard.insert(player_name.clone(), member);
                } // 释放 members 锁，避免后续广播重复加锁导致死锁

                // 发送加入响应
                let response = LanMessage::JoinResponse {
                    success: true,
                    error: None,
                    room_info: Some(room_info),
                };
                Self::write_msg(stream, &response)?;

                // 登记客户端连接，用于后续广播
                if let Ok(clone) = stream.try_clone() {
                    clients.lock().unwrap().push((player_name.clone(), clone));
                }

                // 广播成员列表更新
                Self::broadcast_members(members.clone(), clients.clone(), started.clone(), waiting_for_players.clone())?;

                Ok(Some(player_name))
            }

            LanMessage::Ready { ready } => {
                debug!("Ready status update: {}", ready);
                if let Some(name) = my_name.clone() {
                    {
                        let mut members_guard = members.lock().unwrap();
                        if let Some(member) = members_guard.get_mut(&name) {
                            member.ready = ready;
                        }
                    } // 释放 members 锁，避免后续广播重复加锁导致死锁
                    // 广播准备状态与成员列表
                    Self::broadcast(clients.clone(), LanMessage::PlayerReady { player_name: name, ready })?;
                    Self::broadcast_members(members.clone(), clients.clone(), started.clone(), waiting_for_players.clone())?;
                }
                Ok(None)
            }

            LanMessage::StartGame { waiting_for_players: wait_flag, chart_path, server_addr, chart_id } => {
                debug!("Start game request");
                *started.lock().unwrap() = true;
                *waiting_for_players.lock().unwrap() = wait_flag;
                // 广播开始游戏给所有成员（含房主谱面与下载地址）
                Self::broadcast(clients.clone(), LanMessage::StartGame { waiting_for_players: wait_flag, chart_path, server_addr, chart_id })?;
                Self::broadcast_members(members.clone(), clients.clone(), started.clone(), waiting_for_players.clone())?;
                Ok(None)
            }

            LanMessage::SyncReady { player_name } => {
                debug!("Member {} synced (downloaded)", player_name);
                let mut ready_guard = sync_ready.lock().unwrap();
                if !ready_guard.contains(&player_name) {
                    ready_guard.push(player_name.clone());
                }
                Ok(None)
            }

            LanMessage::StartPlaying => {
                debug!("Start playing broadcast");
                // 广播开始游玩给所有成员
                Self::broadcast(clients.clone(), LanMessage::StartPlaying)?;
                Ok(None)
            }

            LanMessage::AudioDevices { devices, selected_index } => {
                debug!("Audio devices update");
                let mut devices_guard = audio_devices.lock().unwrap();
                *devices_guard = devices.iter().map(|d| d.name.clone()).collect();
                *selected_device.lock().unwrap() = selected_index;
                Self::broadcast(clients.clone(), LanMessage::AudioDevices { devices, selected_index })?;
                Ok(None)
            }

            _ => {
                warn!("Unhandled message type from {}: {:?}", addr, msg);
                Ok(None)
            }
        }
    }

    /// 向单个连接写入一条消息（以换行分隔）
    fn write_msg(stream: &mut TcpStream, msg: &LanMessage) -> Result<()> {
        let msg_str = serde_json::to_string(msg)?;
        stream.write_all(msg_str.as_bytes())?;
        stream.write_all(b"\n")?;
        Ok(())
    }

    /// 向所有已连接客户端广播消息（并移除已断开的连接）
    fn broadcast(clients: Arc<Mutex<Vec<(String, TcpStream)>>>, msg: LanMessage) -> Result<()> {
        let msg_str = serde_json::to_string(&msg)?;
        let mut clients_guard = clients.lock().unwrap();
        clients_guard.retain_mut(|(_, stream)| {
            stream
                .write_all(msg_str.as_bytes())
                .and_then(|_| stream.write_all(b"\n"))
                .is_ok()
        });
        Ok(())
    }

    /// 广播成员列表
    fn broadcast_members(
        members: Arc<Mutex<HashMap<String, MemberInfo>>>,
        clients: Arc<Mutex<Vec<(String, TcpStream)>>>,
        started: Arc<Mutex<bool>>,
        waiting_for_players: Arc<Mutex<bool>>,
    ) -> Result<()> {
        let _ = (started, waiting_for_players);
        let member_list: Vec<MemberInfo> = members.lock().unwrap().values().cloned().collect();
        Self::broadcast(clients, LanMessage::RoomMembers { members: member_list })
    }

    /// 房主广播开始游戏给所有已加入的成员
    pub fn broadcast_start(&self, waiting_for_players: bool, chart_path: Option<String>, server_addr: Option<String>, chart_id: Option<i32>) -> Result<()> {
        // 记录房主的“等待其他玩家”开关，供 host_all_ready 判断是否需等所有成员准备
        *self.waiting_for_players.lock().unwrap() = waiting_for_players;
        Self::broadcast(self.clients.clone(), LanMessage::StartGame { waiting_for_players, chart_path, server_addr, chart_id })
    }

    /// 获取成员列表（仅通过 TCP 加入的玩家）
    pub fn get_members(&self) -> Vec<MemberInfo> {
        self.members.lock().unwrap().values().cloned().collect()
    }

    /// 是否已开始
    pub fn is_started(&self) -> bool {
        *self.started.lock().unwrap()
    }

    /// 已同步就绪（下载完成）的成员数
    pub fn sync_ready_count(&self) -> usize {
        self.sync_ready.lock().unwrap().len()
    }

    /// 玩家总数（不含房主）
    pub fn members_count(&self) -> usize {
        self.members.lock().unwrap().len()
    }

    /// 房主是否开启“等待其他玩家”（需等所有成员准备）
    pub fn is_waiting(&self) -> bool {
        *self.waiting_for_players.lock().unwrap()
    }

    /// 所有已加入成员是否都已准备
    pub fn all_members_ready(&self) -> bool {
        let members = self.members.lock().unwrap();
        !members.is_empty() && members.values().all(|m| m.ready)
    }

    /// 房主广播开始游玩给所有成员
    pub fn broadcast_start_playing(&self) -> Result<()> {
        Self::broadcast(self.clients.clone(), LanMessage::StartPlaying)
    }
}
