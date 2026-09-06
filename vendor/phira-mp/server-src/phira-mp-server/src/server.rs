use crate::{vacant_entry, IdMap, Room, SafeMap, Session, User, ReplayManager};
use anyhow::Result;
use phira_mp_common::RoomId;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, sync::Arc};
use tokio::{net::TcpListener, sync::{mpsc, RwLock}, task::JoinHandle};
use tracing::{info, warn};
use uuid::Uuid;
use crate::BanInfo;

#[derive(Clone, Serialize, Deserialize)]
pub struct CurrentChart {
    pub id: i32,
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub player_count: usize,
    pub state: String,
    pub mode: String,
    pub locked: bool,
    pub players: Vec<String>,
    pub current_chart: Option<CurrentChart>,
    pub is_competition: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionInfoResponse {
    pub user_id: i32,
    pub user_name: String,
    pub ip_address: String,
    pub connect_time: u64,
}

use crate::BanManager;

#[derive(Debug, Deserialize)]
pub struct Chart {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UploadConfig {
    pub enabled: bool,
    pub api_url: String,
    pub api_token: String,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_url: "http://183.66.27.19:40004/upload_direct".to_string(),
            api_token: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub monitors: Vec<i32>,
    #[serde(skip)]
    pub web_port: Option<u16>, // 添加web_port字段，用于存储Web服务器端口
    #[serde(default)]
    pub upload: UploadConfig,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self { 
            monitors: vec![2],
            web_port: None,
            upload: UploadConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Record {
    pub id: i32,
    pub player: i32,
    pub score: i32,
    pub perfect: i32,
    pub good: i32,
    pub bad: i32,
    pub miss: i32,
    pub max_combo: i32,
    pub accuracy: f32,
    pub full_combo: bool,
    pub std: f32,
    pub std_score: f32,
}

#[derive(Clone)]
pub struct SessionInfo {
    pub user_id: i32,
    pub user_name: String,
    pub ip_address: String,
    pub connect_time: std::time::SystemTime,
}

#[derive(Clone)]
pub struct OnlineRoomInfo {
    pub id: String,
    pub player_count: usize,
    pub state: String,
    pub mode: String,
    pub locked: bool,
    pub players: Vec<String>,
    pub current_chart: Option<OnlineRoomCurrentChart>,
    pub created_at: std::time::SystemTime,
}

#[derive(Clone)]
pub struct OnlineRoomCurrentChart {
    pub id: i32,
    pub name: String,
}

pub struct ServerState {
    pub config: ServerConfig,
    pub sessions: IdMap<Arc<Session>>,
    pub users: SafeMap<i32, Arc<User>>,

    pub rooms: SafeMap<RoomId, Arc<Room>>,

    pub lost_con_tx: mpsc::Sender<Uuid>,
    
    // 添加用于管理连接的字段
    pub session_info: SafeMap<Uuid, SessionInfo>,
    // 添加IP黑名单
    pub ip_blacklist: SafeMap<String, std::time::SystemTime>,
    // 添加封禁管理器
    pub ban_manager: BanManager,
    // 添加在线房间信息记录
    pub online_rooms: SafeMap<String, OnlineRoomInfo>,
    // 添加房间最后活跃时间记录
    pub room_last_activity: SafeMap<String, std::time::SystemTime>,
    // 添加管理界面创建的房间记录
    pub admin_created_rooms: SafeMap<String, bool>,
    // 维护模式
    pub maintenance_mode: RwLock<bool>,
    // 维护模式白名单ID列表
    pub maintenance_whitelist: RwLock<HashSet<i32>>,
    // WebSocket广播通道
    pub ws_tx: tokio::sync::broadcast::Sender<serde_json::Value>,
    // Replay recording manager
    pub replay_manager: RwLock<ReplayManager>,
    // 本地谱面分享中转缓存：chart_uuid -> zip 字节（房主上传，玩家经服务端下载）
    pub chart_cache: SafeMap<String, Vec<u8>>,
    // 服务端公网 IP（启动时查询，用于下发下载地址）
    pub public_ip: RwLock<String>,
}

pub struct Server {
    pub state: Arc<ServerState>,
    listener: TcpListener,
    lost_con_handle: JoinHandle<()>,
}
impl Server {
    pub fn new(listener: TcpListener, config: ServerConfig) -> Self {
        let (lost_con_tx, mut lost_con_rx) = mpsc::channel(16);
        let (ws_tx, _ws_rx) = tokio::sync::broadcast::channel(100);
        let ban_manager = BanManager::new();
        
        let state = Arc::new(ServerState {
            config,
            sessions: IdMap::default(),
            users: SafeMap::default(),
            rooms: SafeMap::default(),
            lost_con_tx,
            session_info: SafeMap::default(),
            ip_blacklist: SafeMap::default(),
            ban_manager,
            online_rooms: SafeMap::default(),
            room_last_activity: SafeMap::default(),
            admin_created_rooms: SafeMap::default(),
            maintenance_mode: RwLock::new(false),
            maintenance_whitelist: RwLock::new(HashSet::new()),
            ws_tx,
            replay_manager: RwLock::new(ReplayManager::new()),
            chart_cache: SafeMap::default(),
            public_ip: RwLock::new(String::new()),
        });
        
        // 后台查询服务端公网 IP（用于本地谱面分享时下发下载地址）
        {
            let state = Arc::clone(&state);
            tokio::spawn(async move {
                if let Ok(resp) = reqwest::get("https://api-ipv4.ip.sb/ip").await {
                    if let Ok(text) = resp.text().await {
                        let ip = text.trim().to_string();
                        if !ip.is_empty() {
                            *state.public_ip.write().await = ip.clone();
                            info!("public ip: {ip}");
                        }
                    }
                }
            });
        }
        
        // 在后台任务中从文件加载封禁列表和维护模式配置
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = state_clone.ban_manager.load_bans_from_file("bans.json").await {
                tracing::warn!("Failed to load bans from file: {}", e);
            }
            
            // 清理过期的封禁
            if let Err(e) = state_clone.ban_manager.cleanup_expired_bans().await {
                tracing::warn!("Failed to cleanup expired bans: {}", e);
            }
            
            // 加载维护模式配置
            if let Ok(content) = tokio::fs::read_to_string("maintenance_config.json").await {
                let config_result: Result<crate::MaintenanceConfig, _> = serde_json::from_str(&content);
                if let Ok(config) = config_result {
                    let mut maintenance_mode = state_clone.maintenance_mode.write().await;
                    *maintenance_mode = config.enabled;
                    
                    let mut whitelist = state_clone.maintenance_whitelist.write().await;
                    whitelist.clear();
                    for id in config.whitelist {
                        whitelist.insert(id);
                    }
                    
                    let whitelist_vec: Vec<i32> = whitelist.iter().cloned().collect();
                    tracing::info!("已加载维护模式配置: enabled={}, whitelist={:?}", config.enabled, whitelist_vec);
                }
            }
        });
        let lost_con_handle = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                while let Some(id) = lost_con_rx.recv().await {
                    warn!("lost connection with {id}");
                    if let Some(session) = state.sessions.write().await.remove(&id)
                        && session
                            .user
                            .session
                            .read()
                            .await
                            .as_ref()
                            .is_some_and(|it| it.ptr_eq(&Arc::downgrade(&session)))
                    {
                        Arc::clone(&session.user).dangle().await;
                    }
                    // 从session_info中移除断开的会话
                    state.session_info.write().await.remove(&id);
                    
                    // 广播房间和会话更新
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        broadcast_rooms_update(&state_clone).await;
                        broadcast_sessions_update(&state_clone).await;
                    });
                }
            }
        });

        // 创建清理任务，定期清理无效的session信息
        let _session_cleanup_handle = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(300)).await; // 每5分钟清理一次
                    let mut session_map = state.session_info.write().await;
                    let mut to_remove = Vec::new();
                    
                    // 检查session_info中的会话是否仍然存在于sessions中
                    for (id, _) in session_map.iter() {
                        if !state.sessions.read().await.contains_key(id) {
                            to_remove.push(*id);
                        }
                    }
                    
                    for id in to_remove {
                        session_map.remove(&id);
                    }
                }
            }
        });

        // 创建IP黑名单清理任务，定期清理过期的黑名单条目
        let _blacklist_cleanup_handle = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await; // 每分钟清理一次
                    let mut blacklist = state.ip_blacklist.write().await;
                    let now = std::time::SystemTime::now();
                    let mut to_remove = Vec::new();
                    
                    for (ip, expiry_time) in blacklist.iter() {
                        if now.duration_since(*expiry_time).unwrap_or(std::time::Duration::from_secs(0)).as_secs() >= 10 {
                            to_remove.push(ip.clone());
                        }
                    }
                    
                    for ip in to_remove {
                        blacklist.remove(&ip);
                    }
                }
            }
        });

        // 创建房间自动清理任务，定期清理超过10分钟没有用户的房间
        let _room_cleanup_handle = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await; // 每分钟检查一次
                    let now = std::time::SystemTime::now();
                    
                    // 获取超过10分钟没有用户的房间
                    let rooms_to_remove = {
                        let rooms = state.rooms.read().await;
                        let room_activities = state.room_last_activity.read().await;
                        let admin_rooms = state.admin_created_rooms.read().await;
                        
                        let mut to_remove = Vec::new();
                        for (room_id, room) in rooms.iter() {
                            // 检查房间是否没有用户（玩家和观察者）
                            let users = room.users().await;
                            let monitors = room.monitors().await;
                            
                            if users.is_empty() && monitors.is_empty() {
                                // 房间没有用户，检查最后活跃时间
                                if let Some(last_activity) = room_activities.get(&room_id.to_string()) {
                                    if now.duration_since(*last_activity).unwrap_or(std::time::Duration::from_secs(0)).as_secs() >= 600 { // 10分钟 = 600秒
                                        // 只有非管理界面创建的房间才自动清理
                                        if !admin_rooms.contains_key(&room_id.to_string()) {
                                            to_remove.push(room_id.clone());
                                        }
                                    }
                                } else {
                                    // 如果没有最后活跃时间记录，认为是过期的
                                    // 只有非管理界面创建的房间才自动清理
                                    if !admin_rooms.contains_key(&room_id.to_string()) {
                                        to_remove.push(room_id.clone());
                                    }
                                }
                            }
                        }
                        to_remove
                    };
                    
                    // 移除过期的房间
                    if !rooms_to_remove.is_empty() {
                        let mut rooms = state.rooms.write().await;
                        let mut room_activities = state.room_last_activity.write().await;
                        let mut admin_rooms = state.admin_created_rooms.write().await;
                        
                        for room_id in rooms_to_remove {
                            rooms.remove(&room_id);
                            room_activities.remove(&room_id.to_string());
                            // 如果房间在管理创建列表中，也移除它
                            admin_rooms.remove(&room_id.to_string());
                            info!("自动清理过期房间: {}", room_id);
                        }
                    }
                }
            }
        });

        Self {
            listener,
            state,
            lost_con_handle,
        }
    }
    pub fn state(&self) -> &Arc<ServerState> {
        &self.state
    }

    pub async fn accept(&self) -> Result<()> {
        let (stream, addr) = self.listener.accept().await?;
        let ip_str = addr.ip().to_string();
        
        let mut should_accept = true;
        // 检查IP是否在黑名单中
        let blacklist = self.state.ip_blacklist.read().await;
        if let Some(expiry_time) = blacklist.get(&ip_str) {
            let now = std::time::SystemTime::now();
            if now.duration_since(*expiry_time).unwrap_or(std::time::Duration::from_secs(0)).as_secs() < 10 {
                // IP在黑名单中且未过期，标记为不应接受
                warn!("来自黑名单IP {ip_str} 的连接将被拒绝（在认证阶段）");
                should_accept = false;
            } else {
                // 黑名单条目已过期，稍后会被清理任务移除
            }
        }
        drop(blacklist);
        
        let mut guard = self.state.sessions.write().await;
        let entry = vacant_entry(&mut guard);
        let session = Session::new(*entry.key(), stream, Arc::clone(&self.state), !should_accept).await?;
        info!(
            "received connections from {addr} ({}), version: {}",
            session.id,
            session.version()
        );
        entry.insert(session);
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.lost_con_handle.abort();
    }
}

// 从 ServerState 生成房间信息
pub async fn get_rooms_info_from_state(state: &ServerState) -> Vec<RoomInfo> {
    let rooms = state.rooms.read().await;
    let mut rooms_info = Vec::new();
    let admin_rooms = state.admin_created_rooms.read().await;
    
    for (uuid, room) in rooms.iter() {
        let users = room.users().await;
        let player_count = users.len();
        
        let room_state = room.client_room_state().await;
        let state_text = match room_state {
            phira_mp_common::RoomState::Playing => "游戏中",
            _ => if room.is_locked() { "已锁定" } else { "准备中" }
        };
        
        let mode_text = if room.is_cycle() { "循环模式" } else { "普通模式" };
        let player_names: Vec<String> = users.iter().map(|u| u.name.clone()).collect();
        
        let current_chart = room.chart.read().await.as_ref().map(|c| CurrentChart {
            id: c.id,
            name: c.name.clone(),
        });
        
        let is_competition_room = admin_rooms.contains_key(&uuid.to_string());
        
        rooms_info.push(RoomInfo {
            id: uuid.to_string(),
            player_count,
            state: state_text.to_string(),
            mode: if is_competition_room { "比赛模式".to_string() } else { mode_text.to_string() },
            locked: room.is_locked(),
            players: player_names,
            current_chart,
            is_competition: is_competition_room,
        });
    }
    
    rooms_info
}

// 从 ServerState 生成会话信息
pub async fn get_sessions_info_from_state(state: &ServerState) -> HashMap<String, SessionInfoResponse> {
    let session_info = state.session_info.read().await;
    let mut sessions_response = HashMap::new();
    
    for (id, info) in session_info.iter() {
        let timestamp = info.connect_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();
            
        let session_data = SessionInfoResponse {
            user_id: info.user_id,
            user_name: info.user_name.clone(),
            ip_address: info.ip_address.clone(),
            connect_time: timestamp,
        };
        sessions_response.insert(id.to_string(), session_data);
    }
    
    sessions_response
}

// 广播房间更新
pub async fn broadcast_rooms_update(state: &ServerState) {
    let rooms = get_rooms_info_from_state(state).await;
    let _ = state.ws_tx.send(serde_json::json!({
        "type": "RoomsUpdate",
        "data": rooms
    }));
}

// 广播会话更新
pub async fn broadcast_sessions_update(state: &ServerState) {
    let sessions = get_sessions_info_from_state(state).await;
    let _ = state.ws_tx.send(serde_json::json!({
        "type": "SessionsUpdate",
        "data": sessions
    }));
}

// 广播封禁更新
pub async fn broadcast_bans_update(state: &ServerState) {
    let bans = state.ban_manager.get_all_bans().await;
    let _ = state.ws_tx.send(serde_json::json!({
        "type": "BansUpdate",
        "data": bans
    }));
}

// 广播维护模式更新
pub async fn broadcast_maintenance_update(state: &ServerState, enabled: bool) {
    let _ = state.ws_tx.send(serde_json::json!({
        "type": "MaintenanceUpdate",
        "data": enabled
    }));
}
