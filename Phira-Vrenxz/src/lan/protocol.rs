//! 局域网联机协议定义
//!
//! 使用 UDP 广播进行服务器发现，使用 TCP 进行房间通信
//! 协议格式：JSON

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// 局域网联机消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LanMessage {
    /// 服务器发现请求
    #[serde(rename = "discover")]
    Discover {
        /// 客户端版本
        version: String,
    },

    /// 服务器发现响应
    #[serde(rename = "discover_response")]
    DiscoverResponse {
        /// 服务器名称
        name: String,
        /// 房间ID
        room_id: u32,
        /// 房间名称
        room_name: String,
        /// 房主名称
        host_name: String,
        /// 房间人数
        player_count: u8,
        /// 最大人数
        max_players: u8,
        /// 是否等待其他玩家
        waiting_for_players: bool,
        /// 是否已开始
        started: bool,
        /// 服务器地址（用于TCP连接）
        address: String,
    },

    /// 加入房间请求
    #[serde(rename = "join")]
    Join {
        /// 房间ID
        room_id: u32,
        /// 玩家名称
        player_name: String,
    },

    /// 加入房间响应
    #[serde(rename = "join_response")]
    JoinResponse {
        /// 是否成功
        success: bool,
        /// 错误信息
        error: Option<String>,
        /// 房间信息
        room_info: Option<RoomInfo>,
    },

    /// 房间成员列表
    #[serde(rename = "room_members")]
    RoomMembers {
        /// 成员列表
        members: Vec<MemberInfo>,
    },

    /// 准备就绪
    #[serde(rename = "ready")]
    Ready {
        /// 是否准备
        ready: bool,
    },

    /// 开始游戏（同步阶段：房主广播谱面信息，成员下载谱面）
    #[serde(rename = "start_game")]
    StartGame {
        /// 是否等待其他玩家
        waiting_for_players: bool,
        /// 房主当前谱面的标识（用于下载/同步）
        #[serde(default)]
        chart_path: Option<String>,
        /// 房主谱面下载服务器地址（成员从该地址下载谱面）
        #[serde(default)]
        server_addr: Option<String>,
        /// 房主谱面的在线 id（成员据此检查本地是否已有相同谱面）
        #[serde(default)]
        chart_id: Option<i32>,
    },

    /// 成员下载完成、同步就绪（通知房主）
    #[serde(rename = "sync_ready")]
    SyncReady {
        /// 玩家名称
        player_name: String,
    },

    /// 所有人同步完成，同时开始游玩（房主广播）
    #[serde(rename = "start_playing")]
    StartPlaying,

    /// 发声设备设置
    #[serde(rename = "audio_devices")]
    AudioDevices {
        /// 设备列表
        devices: Vec<AudioDeviceInfo>,
        /// 当前选择的设备索引
        selected_index: usize,
    },

    /// 文件下载请求
    #[serde(rename = "download_chart")]
    DownloadChart {
        /// 谱面ID
        chart_id: String,
        /// 谱面名称
        chart_name: String,
    },

    /// 文件下载响应
    #[serde(rename = "download_chart_response")]
    DownloadChartResponse {
        /// 是否成功
        success: bool,
        /// 错误信息
        error: Option<String>,
    },

    /// 文件下载进度
    #[serde(rename = "download_progress")]
    DownloadProgress {
        /// 进度百分比 (0-100)
        progress: u8,
    },

    /// 文件下载完成
    #[serde(rename = "download_complete")]
    DownloadComplete {
        /// 谱面ID
        chart_id: String,
    },

    /// 玩家准备就绪通知
    #[serde(rename = "player_ready")]
    PlayerReady {
        /// 玩家名称
        player_name: String,
        /// 是否准备
        ready: bool,
    },

    /// 等待其他玩家状态
    #[serde(rename = "waiting_status")]
    WaitingStatus {
        /// 是否正在等待
        waiting: bool,
        /// 已准备人数
        ready_count: u8,
        /// 总人数
        total_count: u8,
    },
}

/// 房间信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    /// 房间ID
    pub room_id: u32,
    /// 房间名称
    pub room_name: String,
    /// 房主名称
    pub host_name: String,
    /// 最大人数
    pub max_players: u8,
    /// 是否等待其他玩家
    pub waiting_for_players: bool,
    /// 是否已开始
    pub started: bool,
}

/// 成员信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberInfo {
    /// 成员名称
    pub name: String,
    /// 是否房主
    pub is_host: bool,
    /// 是否准备
    pub ready: bool,
    /// 是否已开始
    pub started: bool,
}

/// 发声设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// 设备名称
    pub name: String,
    /// 设备ID
    pub id: String,
    /// 是否当前设备
    pub is_current: bool,
}

/// UDP 广播配置
pub const LAN_BROADCAST_PORT: u16 = 27015;
pub const LAN_BROADCAST_ADDR: &str = "255.255.255.255:27015";
pub const LAN_DISCOVERY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

/// TCP 服务器端口范围
pub const LAN_TCP_PORT_MIN: u16 = 27016;
pub const LAN_TCP_PORT_MAX: u16 = 27025;

/// 局域网联机配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanConfig {
    /// 服务器名称
    pub server_name: String,
    /// 房间名称
    pub room_name: String,
    /// 最大玩家数
    pub max_players: u8,
    /// 是否等待其他玩家
    pub waiting_for_players: bool,
    /// 本地IP地址
    pub local_ip: String,
    /// TCP端口
    pub tcp_port: u16,
}

impl Default for LanConfig {
    fn default() -> Self {
        Self {
            server_name: "Phira LAN".to_string(),
            room_name: "My Room".to_string(),
            max_players: 4,
            waiting_for_players: false,
            local_ip: "0.0.0.0".to_string(),
            tcp_port: LAN_TCP_PORT_MIN,
        }
    }
}
