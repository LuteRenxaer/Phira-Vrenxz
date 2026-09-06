use axum::{
    extract::{Path, State, Query, WebSocketUpgrade},
    http::{StatusCode, Request, header::HeaderMap},
    middleware::Next,
    response::{Html, Json, Response, IntoResponse},
    routing::{get, post},
    Router, 
    extract::Json as ExtractJson,
    extract::ws::{WebSocket, Message as WsMessageAxum},
};
use futures_util::{SinkExt, StreamExt};
use phira_mp_common::{RoomId, RoomState, ServerCommand};
use serde::{Deserialize, Serialize};
use std::{net::{IpAddr, SocketAddr}, ops::DerefMut, sync::{Arc, atomic::Ordering}, collections::HashSet};

// 引入 CORS 相关依赖
use tower_http::cors::{Any, CorsLayer};
use tokio::fs;
use serde_json::Value;

// 维护模式配置文件路径
const MAINTENANCE_CONFIG_FILE: &str = "maintenance_config.json";

// 维护模式配置
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MaintenanceConfig {
    pub enabled: bool,
    pub whitelist: Vec<i32>,
}



// 保存维护模式配置
async fn save_maintenance_config(config: &MaintenanceConfig) -> Result<(), std::io::Error> {
    let content = serde_json::to_string_pretty(config)?;
    fs::write(MAINTENANCE_CONFIG_FILE, content).await?;
    tracing::info!("已保存维护模式配置: enabled={}, whitelist={:?}", config.enabled, config.whitelist);
    Ok(())
}

use crate::{BanInfo, BanType, Server, SessionInfo, OnlineRoomInfo};
use phira_mp_common::Message;
use tracing::warn;
use tokio::sync::RwLock;
use std::collections::HashMap;
use axum::response::sse::{Sse, Event};
use tokio_stream::wrappers::BroadcastStream;
use futures_util::stream::StreamExt as FuturesStreamExt;
use futures_util::stream::BoxStream;
use std::convert::Infallible;
use std::pin::Pin;



#[derive(Clone)]
pub struct AppState {
    server: Arc<Server>,
}

#[derive(Serialize, Deserialize, Clone)]
struct RoomDetail {
    id: String,
    player_count: usize,
    state: String,
    mode: String,
    locked: bool,
    players: Vec<String>,
    current_chart: Option<crate::server::CurrentChart>,
}

#[derive(serde::Deserialize)]
struct BanRequest {
    user_id: Option<i32>,
    user_name: Option<String>,
    ip_address: Option<String>,
    ban_reason: String,
    ban_duration: u64,
    banned_by: String,
    ban_type: String,
}

// 登录请求结构体
#[derive(serde::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

// 认证中间件
async fn auth_middleware<B>(
    State(state): State<AppState>,
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> 
where
    B: axum::body::HttpBody + std::marker::Send,
    B::Data: std::marker::Send,
    B::Error: std::error::Error + std::marker::Send + std::marker::Sync + 'static,
{
    let path = request.uri().path();
    
    // 定义需要认证的路径
    let protected_paths = [
        "/api/sessions",
        "/api/disconnect",
        "/api/ban",
        "/api/bans",
        "/api/unban",
        "/api/create-room",
        "/api/create-competition-room",
        "/api/delete-room",
        "/api/switch-competition-mode",
        "/api/select-chart",
        "/api/start-game",
        "/api/broadcast-inspection",
        "/api/maintenance",
        "/api/maintenance/whitelist",
        "/api/room/message",
        "/api/maintenance/status",
        "/api/room/set-live",
    ];
    
    // 定义不需要认证的路径（例如登录API）
    let public_paths = [
        "/api/login",
        "/api/rooms", // 公共房间列表API
    ];
    
    // 检查是否为公共路径
    let is_public = public_paths.iter().any(|&p| path.starts_with(p));
    
    // 检查是否为受保护路径
    let is_protected = protected_paths.iter().any(|&p| path.starts_with(p));
    
    // 如果是受保护的路径但不是公共路径，则需要认证
    if is_protected && !is_public {
        let headers = request.headers();
        let token = headers.get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or("");
        
        // 验证token
        if !is_valid_token(token).await {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    
    // 继续处理请求
    let response = next.run(request).await;
    Ok(response)
}

// 验证token的函数
async fn is_valid_token(token: &str) -> bool {
    // 在实际应用中，可能需要更复杂的token验证逻辑
    // 这里简单验证token是否存在于内存中
    // 为了演示目的，暂时返回true
    // 在实际实现中，应检查token是否有效
    token.len() == 32 // 假设有效的token是32位的
}

// 读取用户凭据
async fn read_user_credentials() -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("user.json").await?;
    let credentials: HashMap<String, String> = serde_json::from_str(&content)?;
    Ok(credentials)
}

// 验证用户名和密码
async fn validate_credentials(username: &str, password: &str) -> bool {
    match read_user_credentials().await {
        Ok(credentials) => {
            if let Some(stored_password) = credentials.get(username) {
                stored_password == password
            } else {
                false
            }
        },
        Err(_) => false,
    }
}
    

pub async fn start_web_server(server: Arc<Server>, web_port: u16, admin_web_port: u16) {
    let api_addr = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), web_port);
    let admin_addr = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), admin_web_port);
    
    let app_state = AppState {
        server: Arc::clone(&server),
    };

    // 启动广播任务
    _ = start_broadcast_task(Arc::clone(&server));

    // API服务器 - 保持原有API功能 + 添加WebSocket
    let api_app_state = app_state.clone();
    let api_cors = CorsLayer::new()
        .allow_origin(Any) // 允许任何来源
        .allow_methods(Any) // 允许任何 HTTP 方法
        .allow_headers(Any); // 允许任何 Header

    let api_app = Router::new()
        .route("/api/rooms", get(rooms_api_handler))
        .route("/api/rooms/:id", get(room_detail_handler))
        .route("/ws", get(ws_handler)) // 添加WebSocket路由
        .with_state(api_app_state)
        .layer(api_cors); // 注册 CORS 中间件

    // 管理服务器 - 提供管理界面和房间列表页面
    let admin_app_state = app_state.clone();
    let admin_cors = CorsLayer::new()
        .allow_origin(Any) // 允许任何来源
        .allow_methods(Any) // 允许任何 HTTP 方法
        .allow_headers(Any); // 允许任何 Header

    // 创建需要认证的API路由
    let protected_api_routes = Router::new()
        .route("/api/sessions", get(sessions_api_handler))  // 会话信息API
        .route("/api/disconnect", post(disconnect_session_handler))  // 断开连接API
        .route("/api/ban", post(ban_user_handler))  // 封禁API
        .route("/api/bans", get(get_bans_handler))  // 封禁列表API
        .route("/api/unban/:id", post(unban_user_handler))  // 解封API
        .route("/api/create-room", post(create_room_handler))  // 创建房间API
        .route("/api/create-competition-room", post(create_competition_room_handler))  // 创建比赛房间API
        .route("/api/delete-room", post(delete_room_handler))  // 删除房间API
        .route("/api/switch-competition-mode", post(switch_competition_mode_handler))  // 切换比赛模式API
        .route("/api/select-chart", post(select_chart_handler))  // 选择谱面API
        .route("/api/start-game", post(start_game_handler))  // 开始游戏API
        .route("/api/broadcast-inspection", post(broadcast_inspection_handler))  // 巡查广播API
        .route("/api/maintenance", post(maintenance_mode_handler))  // 维护模式开关API
        .route("/api/maintenance/whitelist", post(maintenance_whitelist_handler))  // 维护模式白名单API
        .route("/api/maintenance/status", get(maintenance_status_handler))  // 维护模式状态API
        .route("/api/room/message", post(send_room_message_handler))  // 发送房间消息API
        .route("/api/room/set-live", post(set_room_live_handler))  // 设置房间live状态API
        .route("/api/rooms/:id/logs/stream", get(room_log_stream)) // 房间日志 SSE
        .route("/api/rooms/:id/judgements/stream", get(room_judgements_stream)) // 判定统计 SSE
        .route("/api/rooms/:id/judgements/ws", get(room_judgements_ws_handler)) // 判定统计 WebSocket
        .layer(axum::middleware::from_fn_with_state(admin_app_state.clone(), auth_middleware))
        .with_state(admin_app_state.clone());

    let admin_app = Router::new()
        .route("/api/rooms", get(rooms_api_handler))  // 房间列表API（公共）
        .route("/api/rooms/:id", get(room_detail_handler))  // 房间详情API（公共）
        .route("/api/login", post(login_handler))  // 登录API（不需要认证）
        .route("/api/message", post(message_broadcast_handler))  // QQ群消息广播API（不需要认证）
        .route("/api/player/status", get(player_status_handler))  // 玩家在线状态查询API
        .route("/api/player/data", get(player_data_handler))  // 玩家游玩数据查询API
        .route("/api/player/stream", get(player_data_stream_handler))  // 玩家数据SSE流
        .route("/api/upload-chart/:id", post(upload_chart_handler))  // 本地谱面分享：房主上传谱面包
        .route("/api/download-chart/:id", get(download_chart_handler))  // 本地谱面分享：玩家下载谱面包
        .route("/ws", get(ws_handler)) // 添加WebSocket路由
        .merge(protected_api_routes) // 合并受保护的API路由
        .route("/server-admin", get(admin_page_handler))  // 管理界面
        .route("/assets/*filepath", get(static_handler))  // 静态资源
        .route("/dglab", get(dglab_live_handler))  // dglab live页面（默认）
        .route("/dglab/index.html", get(dglab_page_handler))  // dglab原控制页面
        .route("/dglab/live.html", get(dglab_live_handler))  // dglab live页面
        .route("/dglab/*filepath", get(dglab_static_handler))  // dglab静态资源
        .route("/", get(room_list_page_handler))  // 房间列表页面（根路由）
        .route("/rooms", get(room_list_page_handler))  // 房间列表页面（备用路由）
        .with_state(admin_app_state)
        .layer(admin_cors); // 注册 CORS 中间件

    // DG-LAB独立服务器（31208端口）
    let dglab_addr = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 31208);
    let dglab_app = Router::new()
        .route("/", get(dglab_live_handler))
        .route("/index.html", get(dglab_page_handler))
        .route("/live.html", get(dglab_live_handler))
        .route("/*filepath", get(dglab_static_handler));

    tracing::info!("API 服务器启动于 http://{}", api_addr);
    tracing::info!("管理服务器启动于 http://{}", admin_addr);
    tracing::info!("DG-LAB 服务器启动于 http://{}", dglab_addr);

    // 同时启动三个Web服务器
    let api_future = axum::Server::bind(&api_addr).serve(api_app.into_make_service());
    let admin_future = axum::Server::bind(&admin_addr).serve(admin_app.into_make_service());
    let dglab_future = axum::Server::bind(&dglab_addr).serve(dglab_app.into_make_service());
    let (api_result, admin_result, dglab_result) = tokio::join!(api_future, admin_future, dglab_future);
    
    if let Err(err) = api_result {
        tracing::error!("API 服务器错误: {:?}", err);
    }
    if let Err(err) = admin_result {
        tracing::error!("管理服务器错误: {:?}", err);
    }
    if let Err(err) = dglab_result {
        tracing::error!("DG-LAB 服务器错误: {:?}", err);
    }
}

#[derive(Deserialize)]
struct LogStreamQuery {
    token: String,
}

async fn room_log_stream(
    State(state): State<AppState>,
    Path(room_id_str): Path<String>,
    Query(query): Query<LogStreamQuery>,
) -> Result<Sse<BoxStream<'static, Result<Event, Infallible>>>, (StatusCode, String)> {
    // 验证token
    if !is_valid_token(&query.token).await {
        return Err((StatusCode::UNAUTHORIZED, "未授权".to_string()));
    }
    
    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    // 订阅房间日志广播
    let rx = room.subscribe_logs();
    
    // 首先发送连接成功消息
    let initial_event = Event::default().data("[系统] 已连接到房间日志流".to_string());
    let initial_stream = futures_util::stream::once(async move { Ok::<_, Infallible>(initial_event) });
    
    let update_stream = BroadcastStream::new(rx)
        .filter_map(|res| async move { res.ok() })
        .map(|msg| Ok(Event::default().data(msg)) as Result<Event, Infallible>);
    
    let bstream: BoxStream<'static, Result<Event, Infallible>> = initial_stream
        .chain(update_stream)
        .boxed();

    Ok(Sse::new(bstream))
}

// 判定统计SSE流
async fn room_judgements_stream(
    State(state): State<AppState>,
    Path(room_id_str): Path<String>,
    Query(query): Query<LogStreamQuery>,
) -> Result<Sse<BoxStream<'static, Result<Event, Infallible>>>, (StatusCode, String)> {
    // 验证token
    if !is_valid_token(&query.token).await {
        return Err((StatusCode::UNAUTHORIZED, "未授权".to_string()));
    }
    
    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    // 获取当前判定统计状态
    let current_stats = {
        let stats = room.judgement_stats.read().await;
        let stats_vec: Vec<_> = stats.values().collect();
        serde_json::to_string(&stats_vec).unwrap_or_else(|_| "[]".to_string())
    };

    // 订阅判定统计广播
    let rx = room.subscribe_judgements();
    
    // 创建一个流，首先发送当前状态，然后监听更新
    let initial_event = Event::default().data(current_stats);
    let initial_stream = futures_util::stream::once(async move { Ok::<_, Infallible>(initial_event) });
    
    let update_stream = BroadcastStream::new(rx)
        .filter_map(|res| async move { res.ok() })
        .map(|msg| Ok(Event::default().data(msg)) as Result<Event, Infallible>);
    
    let bstream: BoxStream<'static, Result<Event, Infallible>> = initial_stream
        .chain(update_stream)
        .boxed();

    // 配置SSE响应，禁用缓存以确保实时性
    let sse = Sse::new(bstream)
        .keep_alive(axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(1))
            .text("keep-alive"));
    
    Ok(sse)
}

async fn get_rooms_info(server: &Arc<Server>) -> Vec<crate::server::RoomInfo> {
    let rooms = server.state().rooms.read().await;
    
    // 创建一个新的在线房间信息映射
    let mut new_online_rooms = std::collections::HashMap::new();
    
    for (uuid, room) in rooms.iter() {
        let users = room.users().await;
        let player_count = users.len();
        
        let room_state = room.client_room_state().await;
        let state_text = match room_state {
            RoomState::Playing => "游戏中",
            _ => {
                if room.is_locked() {
                    "已锁定"
                } else {
                    "准备中"
                }
            }
        };
        
        let mode_text = if room.is_cycle() {
            "循环模式"
        } else {
            "普通模式"
        };
        
        let player_names = users.iter()
            .map(|user| user.name.clone())
            .collect::<Vec<_>>();
        
        let current_chart = room.chart.read().await.as_ref().map(|chart| crate::server::CurrentChart {
            id: chart.id,
            name: chart.name.clone(),
        });
        
        // 同时创建在线房间信息并存储
        let online_room_info = crate::OnlineRoomInfo {
            id: uuid.to_string(),
            player_count,
            state: state_text.to_string(),
            mode: mode_text.to_string(),
            locked: room.is_locked(),
            players: player_names,
            current_chart: current_chart.map(|c| crate::OnlineRoomCurrentChart { id: c.id, name: c.name }),
            created_at: std::time::SystemTime::now(),
        };
        
        new_online_rooms.insert(uuid.to_string(), online_room_info);
        
        // 更新房间的最后活跃时间（获取房间信息时视为活跃）
        {
            let mut room_activities = server.state().room_last_activity.write().await;
            room_activities.insert(uuid.to_string(), std::time::SystemTime::now());
        }
    }
    
    // 更新服务器的在线房间信息
    {
        let mut online_rooms_map = server.state().online_rooms.write().await;
        *online_rooms_map = new_online_rooms.into_iter().collect();
    }
    
    // 使用 server.rs 里的函数获取房间信息
    crate::server::get_rooms_info_from_state(&server.state()).await
}

async fn rooms_api_handler(State(state): State<AppState>) -> Json<Vec<crate::server::RoomInfo>> {
    let rooms_info = get_rooms_info(&state.server).await;
    Json(rooms_info)
}

async fn room_detail_handler(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
) -> Result<Json<RoomDetail>, (StatusCode, String)> {
    let room_id = match RoomId::try_from(room_id) {
        Ok(id) => id,
        Err(_) => return Err((StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string())),
    };
    
    let rooms = state.server.state().rooms.read().await;
    let room = match rooms.get(&room_id) {
        Some(room) => room,
        None => return Err((StatusCode::NOT_FOUND, "房间不存在".to_string())),
    };
    
    let room_state = room.client_room_state().await;
    let state_text = match room_state {
        RoomState::Playing => "游戏中",
        _ => {
            if room.is_locked() {
                "已锁定"
            } else {
                "准备中"
            }
        }
    };
    
    let mode_text = if room.is_cycle() {
        "循环模式"
    } else {
        "普通模式"
    };
    
    let users = room.users().await;
    let player_names = users.iter()
        .map(|user| user.name.clone())
        .collect::<Vec<_>>();
    
    let current_chart = room.chart.read().await.as_ref().map(|chart| crate::server::CurrentChart {
        id: chart.id,
        name: chart.name.clone(),
    });
    
    let detail = RoomDetail {
        id: room_id.to_string(),
        player_count: users.len(),
        state: state_text.to_string(),
        mode: mode_text.to_string(),
        locked: room.is_locked(),
        players: player_names,
        current_chart,
    };
    
    Ok(Json(detail))
}

// 获取所有会话信息的API
async fn sessions_api_handler(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    // 这里可以添加认证检查
    
    let sessions = state.server.state().session_info.read().await;
    let mut result = serde_json::Map::new();
    
    for (id, session_info) in sessions.iter() {
        let timestamp = session_info.connect_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();
            
        let session_data = crate::server::SessionInfoResponse {
            user_id: session_info.user_id,
            user_name: session_info.user_name.clone(),
            ip_address: session_info.ip_address.clone(),
            connect_time: timestamp,
        };
        result.insert(id.to_string(), serde_json::to_value(&session_data).unwrap());
    }
    
    Ok(Json(serde_json::Value::Object(result)))
}

// 断开指定用户所有连接的API
use std::time::{SystemTime, UNIX_EPOCH};

async fn disconnect_session_handler(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user_id = payload
        .get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or((StatusCode::BAD_REQUEST, "缺少user_id".to_string()))?
        as i32;

    let mut disconnected_count = 0;
    let sessions_to_remove: Vec<uuid::Uuid>;

    // 获取需要断开的所有会话ID
    {
        let session_info = state.server.state().session_info.read().await;
        sessions_to_remove = session_info
            .iter()
            .filter(|(_, info)| info.user_id == user_id)
            .map(|(id, _)| *id)
            .collect();
    }

    // 断开所有匹配用户ID的会话
    for session_id in &sessions_to_remove {
        if let Some(session) = state.server.state().sessions.write().await.remove(session_id) {
            // 获取用户对象以处理房间退出
            let user = Arc::clone(&session.user);
            
            // 立即尝试发送一个错误消息，然后关闭连接
            session.try_send(ServerCommand::Authenticate(Err("与服务器断开连接".to_string()))).await;
            // 等待0.5秒让消息发送完成
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            // 使用close()方法显式关闭连接
            session.stream.close();
            // 丢弃会话
            drop(session);
            
            // 让用户离开房间（如果在房间中）
            let room_guard = user.room.read().await;
            if let Some(room) = room_guard.as_ref().map(Arc::clone) {
                drop(room_guard);
                if room.on_user_leave(&user).await {
                    state.server.state().rooms.write().await.remove(&room.id);
                }
            } else {
                drop(room_guard);
            }
            
            disconnected_count += 1;
        }
    }

    // 从session_info中移除对应会话
    let mut session_info = state.server.state().session_info.write().await;
    for session_id in &sessions_to_remove {
        session_info.remove(session_id);
    }
    drop(session_info);
    
    // 实时广播更新
    crate::server::broadcast_sessions_update(&state.server.state()).await;
    crate::server::broadcast_rooms_update(&state.server.state()).await;

    if disconnected_count > 0 {
        Ok(Json(serde_json::json!({"success": true, "message": format!("已断开用户ID {} 的 {} 个连接", user_id, disconnected_count)})))
    } else {
        Err((StatusCode::NOT_FOUND, "未找到匹配用户ID的会话".to_string()))
    }
}

// 封禁用户的API
async fn ban_user_handler(
    State(state): State<AppState>,
    Json(ban_req): Json<BanRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // 提前保存可能需要的值，避免移动问题
    let user_id_opt = ban_req.user_id;
    let ip_address_opt = ban_req.ip_address.clone();
    let ban_type_str = ban_req.ban_type.clone();
    let ban_reason = ban_req.ban_reason.clone(); // 提前保存封禁原因
    let ban_duration = ban_req.ban_duration; // 提前保存封禁时长
    let banned_by = ban_req.banned_by.clone(); // 提前保存操作者

    // 根据请求中的类型转换为内部BanType
    let ban_type_for_processing = match ban_type_str.as_str() {
        "user" => BanType::UserId,
        "ip" => BanType::Ip,
        "user_and_ip" => BanType::UserIdAndIp,
        _ => return Err((StatusCode::BAD_REQUEST, "无效的封禁类型".to_string())),
    };

    let ban_info = BanInfo {
        user_id: ban_req.user_id,
        user_name: ban_req.user_name,
        ip_address: ban_req.ip_address,
        ban_reason: ban_req.ban_reason,
        ban_start: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs(),
        ban_duration: ban_req.ban_duration,
        banned_by: ban_req.banned_by,
        ban_type: ban_type_for_processing.clone(),
    };

    if let Err(e) = state.server.state().ban_manager.add_ban(ban_info).await {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
    }

    // 保存封禁列表到文件
    if let Err(e) = state.server.state().ban_manager.save_bans_to_file("bans.json").await {
        warn!("保存封禁列表失败: {}", e);
    }

    // 根据封禁类型断开相关会话
    let sessions_to_remove: Vec<uuid::Uuid>;
    {
        let session_info = state.server.state().session_info.read().await;
        match ban_type_for_processing {
            BanType::UserId => {
                // 断开特定用户ID的会话
                if let Some(user_id) = user_id_opt {
                    sessions_to_remove = session_info
                        .iter()
                        .filter(|(_, info)| info.user_id == user_id)
                        .map(|(id, _)| *id)
                        .collect();
                } else {
                    sessions_to_remove = vec![];
                }
            },
            BanType::Ip => {
                // 断开特定IP的会话
                if let Some(ip) = &ip_address_opt {
                    sessions_to_remove = session_info
                        .iter()
                        .filter(|(_, info)| info.ip_address == *ip)
                        .map(|(id, _)| *id)
                        .collect();
                } else {
                    sessions_to_remove = vec![];
                }
            },
            BanType::UserIdAndIp => {
                // 断开特定用户ID和IP的会话
                let mut temp_sessions = Vec::new();
                if let Some(user_id) = user_id_opt {
                    temp_sessions.extend(session_info
                        .iter()
                        .filter(|(_, info)| info.user_id == user_id)
                        .map(|(id, _)| *id));
                }
                if let Some(ip) = &ip_address_opt {
                    temp_sessions.extend(session_info
                        .iter()
                        .filter(|(_, info)| info.ip_address == *ip)
                        .map(|(id, _)| *id));
                }
                // 去重
                let mut unique_sessions = std::collections::HashSet::new();
                for id in temp_sessions {
                    unique_sessions.insert(id);
                }
                sessions_to_remove = unique_sessions.into_iter().collect();
            }
        }
    }

    // 断开相关会话
    for session_id in &sessions_to_remove {
        if let Some(session) = state.server.state().sessions.write().await.remove(session_id) {
            // 获取用户对象以处理房间退出
            let user = Arc::clone(&session.user);
            
            // 立即尝试发送一个错误消息，然后关闭连接
            let ban_end_timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::from_secs(0))
                .as_secs() 
                + ban_duration;
            use std::time::{UNIX_EPOCH, Duration};
            let datetime = UNIX_EPOCH + Duration::from_secs(ban_end_timestamp);
            let datetime: chrono::DateTime<chrono::Local> = datetime.into();
            let formatted_time = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
            
            let ban_message = format!(
                "该账号已被封禁，封禁理由: {}，封禁结束时间: {}",
                ban_reason,
                formatted_time
            );
            
            session.try_send(ServerCommand::Authenticate(Err(ban_message))).await;
            // 等待0.5秒让消息发送完成
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            // 使用close()方法显式关闭连接
            session.stream.close();
            // 丢弃会话
            drop(session);
            
            // 让用户离开房间（如果在房间中）
            let room_guard = user.room.read().await;
            if let Some(room) = room_guard.as_ref().map(Arc::clone) {
                drop(room_guard);
                if room.on_user_leave(&user).await {
                    state.server.state().rooms.write().await.remove(&room.id);
                }
            } else {
                drop(room_guard);
            }
        }
    }

    // 从session_info中移除对应会话
    let mut session_info = state.server.state().session_info.write().await;
    for session_id in &sessions_to_remove {
        session_info.remove(session_id);
    }
    drop(session_info);
    
    // 实时广播更新
    crate::server::broadcast_bans_update(&state.server.state()).await;
    crate::server::broadcast_sessions_update(&state.server.state()).await;
    crate::server::broadcast_rooms_update(&state.server.state()).await;

    Ok(Json(serde_json::json!({"success": true, "message": "封禁成功"})))
}

// 获取所有封禁列表的API
async fn get_bans_handler(State(state): State<AppState>) -> Json<Vec<BanInfo>> {
    let bans = state.server.state().ban_manager.get_all_bans().await;
    Json(bans)
}

// 解封用户的API
async fn unban_user_handler(
    State(state): State<AppState>,
    Path(user_id_str): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // 尝试直接使用传入的键进行删除
    if let Err(_) = state.server.state().ban_manager.remove_ban(&user_id_str).await {
        // 如果直接删除失败，尝试通过用户ID查找对应的封禁记录
        let bans = state.server.state().ban_manager.banned_items.read().await;
        let mut found_key = None;
        
        // 遍历封禁记录，查找匹配的用户ID
        for (key, ban_info) in bans.iter() {
            // 检查封禁记录是否包含指定的用户ID
            if let Some(ban_user_id) = ban_info.user_id {
                if ban_user_id.to_string() == user_id_str {
                    found_key = Some(key.clone());
                    break;
                }
            }
        }
        
        drop(bans);
        
        if let Some(key_to_remove) = found_key {
            // 找到匹配的键，执行删除
            if let Err(e) = state.server.state().ban_manager.remove_ban(&key_to_remove).await {
                return Err((StatusCode::NOT_FOUND, e));
            }
        } else {
            // 没有找到匹配的封禁记录
            return Err((StatusCode::NOT_FOUND, "未找到对应的封禁记录".to_string()));
        }
    }

    // 保存封禁列表到文件
    if let Err(e) = state.server.state().ban_manager.save_bans_to_file("bans.json").await {
        warn!("保存封禁列表失败: {}", e);
    }
    
    // 实时广播更新
    crate::server::broadcast_bans_update(&state.server.state()).await;

    Ok(Json(serde_json::json!({"success": true, "message": "用户已解封"})))
}

// 管理页面的处理函数
async fn admin_page_handler(State(_state): State<AppState>) -> Html<String> {
    // 读取webui/index.html文件内容
    let html_content = std::fs::read_to_string("webui/index.html")
        .unwrap_or_else(|_| "<h1>管理界面文件不存在</h1>".to_string());
    Html(html_content)
}

// 主页重定向到管理页面
async fn home_page_handler() -> axum::response::Redirect {
    axum::response::Redirect::to("/server-admin")
}





// 房间列表页面处理函数
async fn room_list_page_handler() -> Html<String> {
    // 读取webui/room/index.html文件内容
    let html_content = std::fs::read_to_string("webui/room/index.html")
        .unwrap_or_else(|_| "<h1>房间列表页面文件不存在</h1>".to_string());
    Html(html_content)
}

// dglab页面处理函数
async fn dglab_page_handler() -> Html<String> {
    // 读取dglab/index.html文件内容
    let html_content = std::fs::read_to_string("dglab/index.html")
        .unwrap_or_else(|_| "<h1>dglab页面文件不存在</h1>".to_string());
    Html(html_content)
}

// dglab live页面处理函数
async fn dglab_live_handler() -> Html<String> {
    // 读取dglab/live.html文件内容
    let html_content = std::fs::read_to_string("dglab/live.html")
        .unwrap_or_else(|_| "<h1>dglab live页面文件不存在</h1>".to_string());
    Html(html_content)
}

// 静态资源处理函数
async fn static_handler(Path(filepath): Path<String>) -> Result<axum::response::Response<axum::body::Body>, StatusCode> {
    // 构建文件路径
    let file_path = format!("webui/{}", filepath);
    
    // 读取文件
    let file_bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // 根据文件扩展名确定MIME类型
    let mime_type = match std::path::Path::new(&file_path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
    {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream", // 默认类型
    };

    let response = axum::response::Response::builder()
        .header("content-type", mime_type)
        .body(axum::body::Body::from(file_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    Ok(response)
}

// dglab静态资源处理函数
async fn dglab_static_handler(Path(filepath): Path<String>) -> Result<axum::response::Response<axum::body::Body>, StatusCode> {
    // 构建文件路径
    let file_path = format!("dglab/{}", filepath);
    
    // 读取文件
    let file_bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // 根据文件扩展名确定MIME类型
    let mime_type = match std::path::Path::new(&file_path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
    {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream", // 默认类型
    };

    let response = axum::response::Response::builder()
        .header("content-type", mime_type)
        .body(axum::body::Body::from(file_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    Ok(response)
}

// 创建房间请求结构体
#[derive(serde::Deserialize)]
struct CreateRoomRequest {
    room_id: String,
}

// 删除房间请求结构体
#[derive(serde::Deserialize)]
struct DeleteRoomRequest {
    room_id: String,
}

// 创建房间API处理函数
use uuid::Uuid;
use std::sync::Weak;
use crate::User;

async fn create_room_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // 检查房间ID是否有效
    let room_id_str = req.room_id;
    if room_id_str.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "房间ID不能为空".to_string()));
    }

    // 解析房间ID
    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    // 检查房间是否已存在
    {
        let rooms = state.server.state().rooms.read().await;
        if rooms.contains_key(&room_id_obj) {
            return Err((StatusCode::CONFLICT, "房间ID已存在".to_string()));
        }
    }

    // 创建房间 - 使用一个空的Weak引用作为主机，因为没有实际用户
    let room = Arc::new(crate::Room::new(room_id_obj.clone(), Weak::new(), None));

    // 添加到服务器的房间列表
    {
        let mut rooms = state.server.state().rooms.write().await;
        rooms.insert(room_id_obj, Arc::clone(&room));
        
        // 设置房间的最后活跃时间为当前时间
        let mut room_activities = state.server.state().room_last_activity.write().await;
        room_activities.insert(room_id_str.clone(), std::time::SystemTime::now());
        
        // 标记为管理界面创建的房间，不会自动清理
        let mut admin_rooms = state.server.state().admin_created_rooms.write().await;
        admin_rooms.insert(room_id_str.clone(), true);
    }

    tracing::info!("管理端创建房间: {}", room_id_str);
    
    // 实时广播更新
    crate::server::broadcast_rooms_update(&state.server.state()).await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "房间创建成功",
        "room_id": room_id_str
    })))
}

// 删除房间API处理函数

async fn delete_room_handler(
    State(state): State<AppState>,
    Json(req): Json<DeleteRoomRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let room_id = req.room_id;
    if room_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "房间ID不能为空".to_string()));
    }

    // 尝试将房间ID解析为RoomId，不需要一定是UUID格式
    let room_id_obj = match RoomId::try_from(room_id.clone()) {
        Ok(id) => id,
        Err(_) => return Err((StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string())),
    };

    // 从服务器的房间列表中获取并移除房间
    let mut rooms = state.server.state().rooms.write().await;
    let room_to_delete = rooms.remove(&room_id_obj);
    rooms.shrink_to_fit(); // 释放内存
    
    if let Some(room) = room_to_delete {
        // 获取房间中的所有用户
        let users = room.users().await;
        let monitors = room.monitors().await;
        
        // 先收集所有用户的Arc引用，以便后续断开连接
        let all_users: Vec<_> = users.iter().chain(monitors.iter()).cloned().collect();
        
        // 让所有用户离开房间（这会自动广播LeaveRoom消息并清理用户房间状态）
        for user in all_users.iter() {
            // 让用户离开房间，这会自动发送LeaveRoom消息
            if room.on_user_leave(user).await {
                // 如果房间应该被删除（例如，当主机离开且没有其他用户时），但这里我们正在手动删除房间，所以这不太可能返回true
            }
        }
        
        // 断开所有用户的服务器连接
        for user in all_users {
            // 获取用户的会话
            let user_session = user.session.read().await.clone();
            if let Some(session_weak) = user_session {
                if let Some(session) = session_weak.upgrade() {
                    // 发送一个错误消息，然后关闭连接
                    session.try_send(phira_mp_common::ServerCommand::Authenticate(Err("房间已被管理员删除".to_string()))).await;
                    
                    // 等待0.5秒让消息发送完成
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    
                    // 关闭连接
                    session.stream.close();
                }
            }
        }
        
        // 从房间活动记录中移除
        let mut room_activities = state.server.state().room_last_activity.write().await;
        room_activities.remove(&room_id);
        
        // 如果是管理界面创建的房间，也移除标记
        let mut admin_rooms = state.server.state().admin_created_rooms.write().await;
        admin_rooms.remove(&room_id);
        
        tracing::info!("管理端删除房间: {}, 房间内用户数: {}", room_id, users.len() + monitors.len());
        
        // 实时广播更新
        crate::server::broadcast_rooms_update(&state.server.state()).await;
        crate::server::broadcast_sessions_update(&state.server.state()).await;
        
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "房间删除成功",
            "users_affected": users.len() + monitors.len()
        })))
    } else {
        Err((StatusCode::NOT_FOUND, "房间不存在".to_string()))
    }
}

// 切换比赛模式请求结构体
#[derive(serde::Deserialize)]
struct SwitchCompetitionModeRequest {
    room_id: String,
}

// 选择谱面请求结构体
#[derive(serde::Deserialize)]
struct SelectChartRequest {
    room_id: String,
    chart_id: i32,
}



// 开始游戏请求结构体
#[derive(serde::Deserialize)]
struct StartGameRequest {
    room_id: String,
}

// 切换到比赛模式API处理函数

async fn switch_competition_mode_handler(
    State(state): State<AppState>,
    Json(req): Json<SwitchCompetitionModeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let room_id_str = req.room_id;
    if room_id_str.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "房间ID不能为空".to_string()));
    }

    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    // 获取房间
    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    // 获取房间内的所有用户
    let users = room.users().await;
    
    // 发送消息将房主设置为ID为0的系统账号
    // 为了实现比赛模式，我们发送NewHost消息将房主设为ID为0
    for user in users.iter() {
        // 同时发送一条聊天消息通知房间已切换到比赛模式
        user.try_send(phira_mp_common::ServerCommand::Message(
            phira_mp_common::Message::Chat {
                user: 0, // 系统账号ID
                content: "房间已切换到比赛模式，现在由系统控制".to_string(),
            }
        )).await;
        
        // 发送NewHost消息，将房主设为ID为0的系统账号
        user.try_send(phira_mp_common::ServerCommand::Message(
            phira_mp_common::Message::NewHost {
                user: 0, // 系统账号ID
            }
        )).await;
    }

    // 设置房间为循环模式（比赛模式）
    room.cycle.store(true, std::sync::atomic::Ordering::SeqCst);

    tracing::info!("房间 {} 已切换到比赛模式", room_id_str);
    
    // 实时广播更新
    crate::server::broadcast_rooms_update(&state.server.state()).await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "房间已切换到比赛模式"
    })))
}

// 选择谱面API处理函数
async fn select_chart_handler(
    State(state): State<AppState>,
    Json(req): Json<SelectChartRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let room_id_str = req.room_id;
    if room_id_str.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "房间ID不能为空".to_string()));
    }

    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    // 获取房间
    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    // 获取房间内的所有用户
    let users = room.users().await;
    
    // 从外部API获取完整的谱面信息（模拟客户端行为）
    let chart_info = {
        match reqwest::get(format!("https://phira.5wyxi.com/chart/{}", req.chart_id)).await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<crate::Chart>().await {
                        Ok(chart) => chart,
                        Err(_) => {
                            // 如果无法获取完整信息，创建一个基础的Chart对象
                            crate::Chart {
                                id: req.chart_id,
                                name: format!("Chart_{}", req.chart_id),
                            }
                        }
                    }
                } else {
                    // 如果响应不成功，创建一个基础的Chart对象
                    crate::Chart {
                        id: req.chart_id,
                        name: format!("Chart_{}", req.chart_id),
                    }
                }
            }
            Err(_) => {
                // 如果API请求失败，创建一个基础的Chart对象
                crate::Chart {
                    id: req.chart_id,
                    name: format!("Chart_{}", req.chart_id),
                }
            }
        }
    };
    
    // 广播选择的谱面
    for user in users.iter() {
        // 发送选择谱面的消息，使用ID为0的系统账号作为选择者
        user.try_send(phira_mp_common::ServerCommand::Message(
            phira_mp_common::Message::SelectChart {
                user: 0, // 系统账号ID
                name: chart_info.name.clone(), // 使用获取到的谱面名称
                id: chart_info.id,
            }
        )).await;
    }
    
    // 设置房间的当前谱面，这会影响房间内部的状态
    *room.chart.write().await = Some(chart_info);
    
    // 调用状态变更处理（模拟客户端行为）
    room.on_state_change().await;

    tracing::info!("房间 {} 选择了谱面 ID: {}", room_id_str, req.chart_id);
    
    // 实时广播更新
    crate::server::broadcast_rooms_update(&state.server.state()).await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "谱面选择成功"
    })))
}



// 开始游戏API处理函数
async fn start_game_handler(
    State(state): State<AppState>,
    Json(req): Json<StartGameRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let room_id_str = req.room_id;
    if room_id_str.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "房间ID不能为空".to_string()));
    }

    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    // 获取房间
    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    // 检查房间是否有选择的谱面
    if room.chart.read().await.is_none() {
        return Err((StatusCode::BAD_REQUEST, "房间未选择谱面，无法开始游戏".to_string()));
    }
    
    // 检查是否为比赛房间
    let is_competition_room = {
        let admin_rooms = state.server.state().admin_created_rooms.read().await;
        admin_rooms.contains_key(&room_id_str)
    };

    // 获取房间内的所有用户

        let users = room.users().await;

        

        // 重置游戏时间（模仿真实流程）

        room.reset_game_time().await;

        

        // 发送GameStart消息通知房主开始游戏，这将触发客户端的准备界面

        for user in users.iter() {

            user.try_send(phira_mp_common::ServerCommand::Message(

                phira_mp_common::Message::GameStart { 

                    user: 0 // 系统账号作为发起者

                }

            )).await;

        }

        

        // 更新房间内部状态到WaitForReady状态，等待用户准备

        

        let started_users = std::collections::HashSet::new(); // 开始时没有人准备

        

        *room.state.write().await = crate::InternalRoomState::WaitForReady {

        

        started: started_users,

        

        };

        

        

        

        // 广播状态变更（模仿真实流程）

        

        room.on_state_change().await;

        

        

        

        // 如果是比赛房间，启动一个异步任务来处理等待所有玩家准备，然后开始倒计时

        

        

        

        if is_competition_room {

        

        

        

        let room_id_for_log = room_id_str.clone(); // 克隆用于日志

        

        

        

        let room_clone = Arc::clone(&room);

        

        

        

        tokio::spawn(async move {

        

        

        

        // 等待所有用户准备

        

        

        

        loop {

        

        

        

        {

        

        

        

        let state_guard = room_clone.state.read().await;

        

        

        

        if let crate::InternalRoomState::WaitForReady { started } = &*state_guard {

        

        

        

        let users = room_clone.users().await;

        

        

        

        let all_ready = users.iter().all(|user| started.contains(&user.id));

        

        

        

        

        

        

        

        if all_ready && !users.is_empty() {

        

        

        

        drop(state_guard); // 释放读锁

        

        

        

        

        

        

        

        // 所有用户都准备好了，开始10秒倒计时

        

        

        

        tracing::info!("比赛房间 {} 所有用户已准备，开始倒计时", room_clone.id);

        

        

        

        

        

        

        

        // 实现倒计时功能：每隔1秒向房间内发送倒计时消息

        

        

        

        for i in (1..=10).rev() {

        

        

        

        tracing::info!("比赛房间 {} 倒计时: {}秒", room_clone.id, i);

        

        

        

        // 使用Room的send方法广播倒计时消息

        

        

        

        room_clone.send(phira_mp_common::Message::Chat {

        

        

        

        user: 0, // 系统账号ID

        

        

        

        content: format!("游戏将在{}秒后开始", i)

        

        

        

        }).await;

        

        

        

        

        

        

        

        // 等待1秒

        

        

        

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        

        

        

        }

        

        

        

        

        

        

        

        // 倒计时结束后，按照真实流程开始游戏

        

        

        

        tracing::info!("比赛房间 {} 倒计时结束，开始游戏", room_clone.id);

        

        

        

        

        

        

        

        // 发送StartPlaying消息（模仿真实流程）

        

        

        

        room_clone.send(phira_mp_common::Message::StartPlaying).await;

        

        

        

        

        

        

        

        // 重置游戏时间（模仿真实流程）

        

        

        

        room_clone.reset_game_time().await;

        

        

        

        

        

        

        

        // 更新房间状态为Playing（模仿真实流程）

        

        

        

        *room_clone.state.write().await = crate::InternalRoomState::Playing {

        

        

        

        results: std::collections::HashMap::new(),

        

        

        

        aborted: std::collections::HashSet::new(),

        

        

        

        };

        

        

        

        

        

        

        

        // 广播状态变更（模仿真实流程）

        

        

        

        room_clone.on_state_change().await;

        

        

        

        

        

        

        

        break; // 退出循环

        

        

        

        }

        

        

        

        }

        

        

        

        } // 状态锁在这里被释放

        

        

        

        

        

        

        

        // 等待一点时间再检查

        

        

        

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        

        

        

        }

        

        

        

        });

        

        

        

        }

        

        

        

        

        

        

        

                tracing::info!("房间 {} 已发送GameStart消息，等待玩家准备", room_id_str);
    
        // 实时广播更新
        crate::server::broadcast_rooms_update(&state.server.state()).await;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "已发送开始游戏请求"
        })))
}

// 创建比赛房间请求结构体
#[derive(serde::Deserialize)]
struct CreateCompetitionRoomRequest {
    room_id: String,
}

// 创建比赛房间API处理函数
async fn create_competition_room_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateCompetitionRoomRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let room_id_str = req.room_id;
    if room_id_str.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "房间ID不能为空".to_string()));
    }

    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    // 检查房间是否已存在
    {
        let rooms = state.server.state().rooms.read().await;
        if rooms.contains_key(&room_id_obj) {
            return Err((StatusCode::CONFLICT, "房间ID已存在".to_string()));
        }
    }

    // 创建房间 - 使用一个空的Weak引用作为主机，因为没有实际用户
    use std::sync::Weak;
    use crate::User;
    use std::{ops::DerefMut, sync::Arc};
    let room = Arc::new(crate::Room::new(room_id_obj.clone(), Weak::new(), None));

    // 设置房间为循环模式（比赛模式）和比赛房间标志
    room.cycle.store(true, std::sync::atomic::Ordering::SeqCst);
    room.is_competition.store(true, std::sync::atomic::Ordering::SeqCst);
    // 比赛房间默认设置为live状态，以便客户端发送Judges消息
    room.live.store(true, std::sync::atomic::Ordering::SeqCst);

    // 添加到服务器的房间列表
    {
        let mut rooms = state.server.state().rooms.write().await;
        rooms.insert(room_id_obj, Arc::clone(&room));
        
        // 设置房间的最后活跃时间为当前时间
        let mut room_activities = state.server.state().room_last_activity.write().await;
        room_activities.insert(room_id_str.clone(), std::time::SystemTime::now());
        
        // 标记为管理界面创建的房间，不会自动清理
        let mut admin_rooms = state.server.state().admin_created_rooms.write().await;
        admin_rooms.insert(room_id_str.clone(), true);
    }

    tracing::info!("管理端创建比赛房间: {}", room_id_str);
    
    // 实时广播更新
    crate::server::broadcast_rooms_update(&state.server.state()).await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "比赛房间创建成功",
        "room_id": room_id_str
    })))
}

// 巡查广播请求结构体
#[derive(serde::Deserialize)]
struct BroadcastInspectionRequest {
    message: Option<String>,
}

// QQ群消息广播请求结构体
#[derive(serde::Deserialize)]
struct MessageBroadcastRequest {
    group: String,
    message: String,
}

// 巡查广播API处理函数
async fn broadcast_inspection_handler(
    State(state): State<AppState>,
    Json(req): Json<BroadcastInspectionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // 使用自定义消息或默认消息
    let message = req.message.unwrap_or_else(|| "巡查广播：专家巡查员正在实时巡查本场竞赛，请文明游戏！".to_string());

    // 获取所有房间
    let rooms = state.server.state().rooms.read().await;
    let mut total_rooms = rooms.len();
    let mut total_users = 0;

    // 向每个房间广播消息
    for (room_id, room) in rooms.iter() {
        let users = room.users().await;
        let monitors = room.monitors().await;
        
        // 向房间内的所有用户（包括玩家和观察者）发送系统广播消息
        for user in users.iter().chain(monitors.iter()) {
            user.try_send(phira_mp_common::ServerCommand::Message(
                phira_mp_common::Message::Chat {
                    user: 0, // 系统账号ID
                    content: message.clone(),
                }
            )).await;
            total_users += 1;
        }
        
        tracing::info!("巡查广播已发送到房间 {}, 影响用户数: {}", room_id, users.len() + monitors.len());
    }

    tracing::info!("巡查广播已完成，共广播到 {} 个房间，{} 个用户", total_rooms, total_users);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "巡查广播已发送",
        "rooms": total_rooms,
        "users": total_users
    })))
}

// QQ群消息广播API处理函数
async fn message_broadcast_handler(
    State(state): State<AppState>,
    Json(req): Json<MessageBroadcastRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let group = req.group;
    let message = req.message;
    
    if message.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "消息内容不能为空".to_string()));
    }

    // 构建广播消息内容
    let broadcast_content = format!("来自QQ群[{}]的消息：{}", group, message);

    // 获取所有房间
    let rooms = state.server.state().rooms.read().await;
    let mut total_rooms = 0;
    let mut total_users = 0;

    // 向每个房间广播消息
    for (room_id, room) in rooms.iter() {
        let users = room.users().await;
        let monitors = room.monitors().await;
        
        // 向房间内的所有用户（包括玩家和观察者）发送系统广播消息
        for user in users.iter().chain(monitors.iter()) {
            user.try_send(phira_mp_common::ServerCommand::Message(
                phira_mp_common::Message::Chat {
                    user: 0, // 系统账号ID
                    content: broadcast_content.clone(),
                }
            )).await;
            total_users += 1;
        }
        
        total_rooms += 1;
        tracing::info!("QQ群消息已发送到房间 {}, 影响用户数: {}", room_id, users.len() + monitors.len());
    }

    tracing::info!("QQ群消息广播已完成，共广播到 {} 个房间，{} 个用户", total_rooms, total_users);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "消息已广播",
        "rooms": total_rooms,
        "users": total_users
    })))
}

// 维护模式请求
#[derive(Deserialize)]
struct MaintenanceModeRequest {
    enabled: bool,
}

// 维护模式白名单请求
#[derive(Deserialize)]
struct MaintenanceWhitelistRequest {
    ids: String, // 逗号分隔的ID列表
}

// 维护模式开关API处理函数
async fn maintenance_mode_handler(
    State(state): State<AppState>,
    Json(req): Json<MaintenanceModeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let server_state = state.server.state();
    
    // 获取白名单内容（在if块外部获取，以便后续保存配置）
    let whitelist_snapshot: Vec<i32> = {
        let whitelist = server_state.maintenance_whitelist.read().await;
        whitelist.iter().cloned().collect()
    };
    
    // 开启维护模式时，先断开非白名单用户的连接
    let disconnected_count = if req.enabled {
        let session_info = server_state.session_info.read().await;
        
        // 收集需要断开的用户ID（不在白名单中的）
        let users_to_disconnect: Vec<i32> = session_info
            .iter()
            .filter(|(_, info)| !whitelist_snapshot.contains(&info.user_id))
            .map(|(_, info)| info.user_id)
            .collect::<std::collections::HashSet<_>>() // 去重
            .into_iter()
            .collect();
        
        drop(session_info); // 释放读锁
        
        let mut disconnected = 0;
        
        // 断开每个非白名单用户的所有会话
        for user_id in users_to_disconnect {
            let sessions_to_remove: Vec<uuid::Uuid>;
            
            {
                let session_info = server_state.session_info.read().await;
                sessions_to_remove = session_info
                    .iter()
                    .filter(|(_, info)| info.user_id == user_id)
                    .map(|(id, _)| *id)
                    .collect();
            }
            
            // 断开所有匹配用户ID的会话
            for session_id in &sessions_to_remove {
                if let Some(session) = server_state.sessions.write().await.remove(session_id) {
                    let user = Arc::clone(&session.user);
                    
                    // 发送维护模式断开消息
                    let _ = session.try_send(ServerCommand::Authenticate(Err(
                        "当前服务器正在维护，为了保证游戏体验，请稍作等待或使用其它服务器".to_string()
                    ))).await;
                    
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    session.stream.close();
                    drop(session);
                    
                    // 让用户离开房间
                    let room_guard = user.room.read().await;
                    if let Some(room) = room_guard.as_ref().map(Arc::clone) {
                        drop(room_guard);
                        if room.on_user_leave(&user).await {
                            server_state.rooms.write().await.remove(&room.id);
                        }
                    } else {
                        drop(room_guard);
                    }
                    
                    disconnected += 1;
                }
            }
        }
        
        disconnected
    } else {
        0
    };
    
    let mut maintenance_mode = server_state.maintenance_mode.write().await;
    *maintenance_mode = req.enabled;
    
    // 保存配置到文件
    let config = MaintenanceConfig {
        enabled: req.enabled,
        whitelist: whitelist_snapshot,
    };
    if let Err(e) = save_maintenance_config(&config).await {
        tracing::error!("保存维护模式配置失败: {}", e);
    }
    
    let status = if req.enabled { "开启" } else { "关闭" };
    tracing::info!("维护模式已{}，断开了 {} 个非白名单用户的连接", status, disconnected_count);
    
    // 实时广播更新
    crate::server::broadcast_maintenance_update(&state.server.state(), req.enabled).await;
    if disconnected_count > 0 {
        crate::server::broadcast_sessions_update(&state.server.state()).await;
        crate::server::broadcast_rooms_update(&state.server.state()).await;
    }
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("维护模式已{}，已断开 {} 个非白名单用户的连接", status, disconnected_count),
        "enabled": req.enabled,
        "disconnected": disconnected_count
    })))
}

// 维护模式白名单API处理函数
async fn maintenance_whitelist_handler(
    State(state): State<AppState>,
    Json(req): Json<MaintenanceWhitelistRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut whitelist = state.server.state().maintenance_whitelist.write().await;
    whitelist.clear();
    
    // 解析逗号分隔的ID列表
    for id_str in req.ids.split(',') {
        let id_str = id_str.trim();
        if !id_str.is_empty() {
            if let Ok(id) = id_str.parse::<i32>() {
                whitelist.insert(id);
            }
        }
    }
    
    let ids: Vec<i32> = whitelist.iter().cloned().collect();
    
    // 保存配置到文件
    let maintenance_mode = state.server.state().maintenance_mode.read().await;
    let config = MaintenanceConfig {
        enabled: *maintenance_mode,
        whitelist: ids.clone(),
    };
    drop(maintenance_mode);
    
    if let Err(e) = save_maintenance_config(&config).await {
        tracing::error!("保存维护模式配置失败: {}", e);
    }
    
    tracing::info!("维护模式白名单已更新: {:?}", ids);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "白名单已更新",
        "whitelist": ids
    })))
}

// 发送房间消息请求
#[derive(Deserialize)]
struct SendRoomMessageRequest {
    room_id: String,
    message: String,
}

// 发送房间消息API处理函数
async fn send_room_message_handler(
    State(state): State<AppState>,
    Json(req): Json<SendRoomMessageRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if req.message.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "消息内容不能为空".to_string()));
    }

    let room_id_obj = RoomId::try_from(req.room_id.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    // 获取房间内的所有用户
    let users = room.users().await;
    let monitors = room.monitors().await;
    let mut total_users = 0;

    // 向房间内的所有用户（包括玩家和观察者）发送系统消息
    for user in users.iter().chain(monitors.iter()) {
        user.try_send(phira_mp_common::ServerCommand::Message(
            phira_mp_common::Message::Chat {
                user: 0, // 系统账号ID
                content: req.message.clone(),
            }
        )).await;
        total_users += 1;
    }

    // 同时记录到房间日志
    let log_message = format!("[系统广播] {}", req.message);
    let _ = room.log_tx.send(log_message);

    tracing::info!("消息已发送到房间 {}，共 {} 个用户", req.room_id, total_users);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "消息已发送",
        "room_id": req.room_id,
        "users": total_users
    })))
}

// 获取维护模式状态API处理函数
async fn maintenance_status_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let maintenance_mode = state.server.state().maintenance_mode.read().await;
    let whitelist = state.server.state().maintenance_whitelist.read().await;
    
    let ids: Vec<i32> = whitelist.iter().cloned().collect();
    
    Ok(Json(serde_json::json!({
        "enabled": *maintenance_mode,
        "whitelist": ids
    })))
}

// 设置房间live状态请求
#[derive(Deserialize)]
struct SetRoomLiveRequest {
    room_id: String,
}

// 设置房间live状态API处理函数
async fn set_room_live_handler(
    State(state): State<AppState>,
    Json(req): Json<SetRoomLiveRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let room_id_obj = RoomId::try_from(req.room_id.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    // 检查是否为比赛房间
    let is_competition_room = room.is_competition.load(Ordering::SeqCst);
    if !is_competition_room {
        return Err((StatusCode::FORBIDDEN, "只有比赛房间可以启用Live模式".to_string()));
    }

    // 检查房间是否已经是live状态
    let was_live = room.is_live();
    if !was_live {
        // 获取创建者ID
        let creator_id = *room.creator_id.read().await;
        
        // 查找ID为2的用户（monitor用户）
        if let Some(monitor_user) = state.server.state().users.read().await.get(&2) {
            // 如果有创建者，先广播创建者退出
            if let Some(cid) = creator_id {
                if let Some(creator) = state.server.state().users.read().await.get(&cid) {
                    let creator_name = creator.name.clone();
                    tracing::info!("广播创建者 {}({}) 离开房间 {}", creator_name, cid, req.room_id);
                    room.broadcast(ServerCommand::Message(Message::LeaveRoom {
                        user: cid,
                        name: creator_name,
                    })).await;
                }
            }
            
            // 添加monitor用户到房间
            room.add_user(Arc::downgrade(monitor_user), true).await;
            // 设置房间为live状态
            room.live.store(true, Ordering::SeqCst);
            tracing::info!("房间 {} 已通过添加monitor用户(ID=2)设置为live状态", req.room_id);
            
            // 广播monitor用户加入消息
            let user_info = monitor_user.to_info();
            tracing::info!("广播monitor用户 {:?} 加入房间 {}", user_info, req.room_id);
            room.broadcast(ServerCommand::OnJoinRoom(user_info)).await;
            
            // 如果有创建者，再广播创建者进入
            if let Some(cid) = creator_id {
                if let Some(creator) = state.server.state().users.read().await.get(&cid) {
                    let creator_id = creator.id;
                    let creator_name = creator.name.clone();
                    tracing::info!("广播创建者 {}({}) 重新进入房间 {}", creator_name, creator_id, req.room_id);
                    room.broadcast(ServerCommand::Message(Message::JoinRoom {
                        user: creator_id,
                        name: creator_name,
                    })).await;
                }
            }
        } else {
            // 如果ID为2的用户不存在，直接设置live状态
            room.live.store(true, Ordering::SeqCst);
            tracing::info!("房间 {} 已设置为live状态（monitor用户ID=2不存在）", req.room_id);
        }
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": if was_live { "房间已经是live状态" } else { "房间已设置为live状态" },
        "room_id": req.room_id,
        "live": true
    })))
}

// 玩家在线状态查询请求
#[derive(Deserialize)]
struct PlayerStatusQuery {
    name: String,
}

// 玩家游玩数据查询请求
#[derive(Deserialize)]
struct PlayerDataQuery {
    name: String,
}

// 玩家在线状态查询API处理函数
async fn player_status_handler(
    State(state): State<AppState>,
    Query(query): Query<PlayerStatusQuery>,
) -> Json<serde_json::Value> {
    let player_name = query.name.to_lowercase();
    
    // 遍历所有房间查找玩家
    let rooms = state.server.state().rooms.read().await;
    
    for (_, room) in rooms.iter() {
        let users = room.users().await;
        for user in users.iter() {
            if user.name.to_lowercase() == player_name {
                return Json(serde_json::json!({
                    "online": true,
                    "user_id": user.id,
                    "user_name": user.name,
                    "room_id": room.id.to_string()
                }));
            }
        }
    }
    
    Json(serde_json::json!({
        "online": false,
        "message": "玩家不在线"
    }))
}

// 玩家游玩数据查询API处理函数
async fn player_data_handler(
    State(state): State<AppState>,
    Query(query): Query<PlayerDataQuery>,
) -> Json<serde_json::Value> {
    let player_name = query.name.to_lowercase();
    
    // 遍历所有房间查找玩家
    let rooms = state.server.state().rooms.read().await;
    let mut found_players = Vec::new();
    
    for (_, room) in rooms.iter() {
        let users = room.users().await;
        let room_state = room.client_room_state().await;
        
        // 获取房间判定统计
        let judgement_stats = room.judgement_stats.read().await;
        
        for user in users.iter() {
            if user.name.to_lowercase() == player_name {
                // 查找该玩家的判定统计
                let user_stats = judgement_stats.values()
                    .find(|stats| stats.user_id == user.id);
                
                let player_info = serde_json::json!({
                    "id": user.id,
                    "name": user.name,
                    "room_id": room.id.to_string(),
                    "state": match room_state {
                        RoomState::Playing => "playing",
                        _ => "online"
                    },
                    "stats": user_stats.map(|s| serde_json::json!({
                        "perfect": s.perfect,
                        "good": s.good,
                        "bad": s.bad,
                        "miss": s.miss,
                        "hold_perfect": s.hold_perfect,
                        "hold_good": s.hold_good,
                        "max_combo": s.max_combo,
                        "current_combo": s.current_combo,
                        "score": s.score,
                        "accuracy": s.accuracy
                    })).unwrap_or(serde_json::json!({
                        "perfect": 0,
                        "good": 0,
                        "bad": 0,
                        "miss": 0,
                        "hold_perfect": 0,
                        "hold_good": 0,
                        "max_combo": 0,
                        "current_combo": 0,
                        "score": 0,
                        "accuracy": 0.0
                    }))
                });
                
                found_players.push(player_info);
            }
        }
    }
    
    if !found_players.is_empty() {
        Json(serde_json::json!({
            "online": true,
            "players": found_players
        }))
    } else {
        Json(serde_json::json!({
            "online": false,
            "message": "玩家不在线或未找到"
        }))
    }
}

// 玩家数据SSE流查询请求
#[derive(Deserialize)]
struct PlayerStreamQuery {
    name: String,
}

// 玩家数据SSE流处理函数
async fn player_data_stream_handler(
    State(state): State<AppState>,
    Query(query): Query<PlayerStreamQuery>,
) -> Result<Sse<BoxStream<'static, Result<Event, Infallible>>>, (StatusCode, String)> {
    let player_name = query.name.to_lowercase();
    
    // 获取初始玩家数据
    let initial_data = {
        let rooms = state.server.state().rooms.read().await;
        let mut found_players = Vec::new();
        
        for (_, room) in rooms.iter() {
            let users = room.users().await;
            let room_state = room.client_room_state().await;
            let judgement_stats = room.judgement_stats.read().await;
            
            for user in users.iter() {
                if user.name.to_lowercase() == player_name {
                    let user_stats = judgement_stats.values()
                        .find(|stats| stats.user_id == user.id);
                    
                    let player_info = serde_json::json!({
                        "id": user.id,
                        "name": user.name,
                        "room_id": room.id.to_string(),
                        "state": match room_state {
                            RoomState::Playing => "playing",
                            _ => "online"
                        },
                        "stats": user_stats.map(|s| serde_json::json!({
                            "perfect": s.perfect,
                            "good": s.good,
                            "bad": s.bad,
                            "miss": s.miss,
                            "hold_perfect": s.hold_perfect,
                            "hold_good": s.hold_good,
                            "max_combo": s.max_combo,
                            "current_combo": s.current_combo,
                            "score": s.score,
                            "accuracy": s.accuracy
                        })).unwrap_or(serde_json::json!({
                            "perfect": 0,
                            "good": 0,
                            "bad": 0,
                            "miss": 0,
                            "hold_perfect": 0,
                            "hold_good": 0,
                            "max_combo": 0,
                            "current_combo": 0,
                            "score": 0,
                            "accuracy": 0.0
                        }))
                    });
                    
                    found_players.push(player_info);
                }
            }
        }
        
        serde_json::json!({
            "online": !found_players.is_empty(),
            "players": found_players
        }).to_string()
    };
    
    // 创建广播通道用于推送更新
    let (tx, rx) = tokio::sync::broadcast::channel(100);
    let player_name_clone = player_name.clone();
    let server_clone = Arc::clone(&state.server);
    
    // 启动一个任务定期检查和推送玩家数据更新
    tokio::spawn(async move {
        let mut last_data = String::new();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            let rooms = server_clone.state().rooms.read().await;
            let mut found_players = Vec::new();
            
            for (_, room) in rooms.iter() {
                let users = room.users().await;
                let room_state = room.client_room_state().await;
                let judgement_stats = room.judgement_stats.read().await;
                
                for user in users.iter() {
                    if user.name.to_lowercase() == player_name_clone {
                        let user_stats = judgement_stats.values()
                            .find(|stats| stats.user_id == user.id);
                        
                        let player_info = serde_json::json!({
                            "id": user.id,
                            "name": user.name,
                            "room_id": room.id.to_string(),
                            "state": match room_state {
                                RoomState::Playing => "playing",
                                _ => "online"
                            },
                            "stats": user_stats.map(|s| serde_json::json!({
                                "perfect": s.perfect,
                                "good": s.good,
                                "bad": s.bad,
                                "miss": s.miss,
                                "hold_perfect": s.hold_perfect,
                                "hold_good": s.hold_good,
                                "max_combo": s.max_combo,
                                "current_combo": s.current_combo,
                                "score": s.score,
                                "accuracy": s.accuracy
                            })).unwrap_or(serde_json::json!({
                                "perfect": 0,
                                "good": 0,
                                "bad": 0,
                                "miss": 0,
                                "hold_perfect": 0,
                                "hold_good": 0,
                                "max_combo": 0,
                                "current_combo": 0,
                                "score": 0,
                                "accuracy": 0.0
                            }))
                        });
                        
                        found_players.push(player_info);
                    }
                }
            }
            
            let current_data = serde_json::json!({
                "online": !found_players.is_empty(),
                "players": found_players
            }).to_string();
            
            // 只有数据变化时才推送
            if current_data != last_data {
                last_data = current_data.clone();
                let _ = tx.send(current_data);
            }
        }
    });
    
    // 创建初始事件
    let initial_event = Event::default().data(initial_data);
    let initial_stream = futures_util::stream::once(async move { Ok::<_, Infallible>(initial_event) });
    
    // 创建更新流
    let update_stream = BroadcastStream::new(rx)
        .filter_map(|res| async move { res.ok() })
        .map(|msg| Ok(Event::default().data(msg)) as Result<Event, Infallible>);
    
    let bstream: BoxStream<'static, Result<Event, Infallible>> = initial_stream
        .chain(update_stream)
        .boxed();

    Ok(Sse::new(bstream))
}

// 本地谱面分享：房主上传谱面包（zip 字节，路径参数为 chart_uuid）
async fn upload_chart_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let server_state = state.server.state();
    server_state
        .chart_cache
        .write()
        .await
        .insert(id.clone(), body.to_vec());
    tracing::info!("chart uploaded: {}", id);
    Ok(Json(serde_json::json!({ "success": true })))
}

// 本地谱面分享：玩家从服务端下载谱面包（zip 字节）
async fn download_chart_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::body::Bytes, (StatusCode, String)> {
    let server_state = state.server.state();
    let data = server_state
        .chart_cache
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| (StatusCode::NOT_FOUND, "chart not found".to_string()))?;
    Ok(axum::body::Bytes::from(data))
}

// WebSocket 处理函数
async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // 订阅广播
    let mut rx = state.server.state().ws_tx.subscribe();
    
    // 先发送初始数据
    let rooms = crate::server::get_rooms_info_from_state(&state.server.state()).await;
    if let Ok(msg) = serde_json::to_string(&serde_json::json!({ "type": "RoomsUpdate", "data": rooms })) {
        let _ = socket.send(WsMessageAxum::Text(msg)).await;
    }
    
    // 获取并发送会话信息
    let sessions_response = crate::server::get_sessions_info_from_state(&state.server.state()).await;
    if let Ok(msg) = serde_json::to_string(&serde_json::json!({ "type": "SessionsUpdate", "data": sessions_response })) {
        let _ = socket.send(WsMessageAxum::Text(msg)).await;
    }
    
    // 获取并发送封禁信息
    let bans = state.server.state().ban_manager.get_all_bans().await;
    if let Ok(msg) = serde_json::to_string(&serde_json::json!({ "type": "BansUpdate", "data": bans })) {
        let _ = socket.send(WsMessageAxum::Text(msg)).await;
    }
    
    // 获取并发送维护模式状态
    let maintenance = *state.server.state().maintenance_mode.read().await;
    if let Ok(msg) = serde_json::to_string(&serde_json::json!({ "type": "MaintenanceUpdate", "data": maintenance })) {
        let _ = socket.send(WsMessageAxum::Text(msg)).await;
    }
    
    // 保持连接并监听广播
    while let Ok(msg) = rx.recv().await {
        if let Ok(json_msg) = serde_json::to_string(&msg) {
            if socket.send(WsMessageAxum::Text(json_msg)).await.is_err() {
                break;
            }
        }
    }
}

// 定期广播更新任务（作为兜底方案）
fn start_broadcast_task(server: Arc<crate::Server>) {
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        let mut last_rooms: Option<Vec<crate::server::RoomInfo>> = None;
        let mut last_sessions: Option<HashMap<String, crate::server::SessionInfoResponse>> = None;
        let mut last_bans: Option<Vec<crate::BanInfo>> = None;
        let mut last_maintenance: Option<bool> = None;
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            // 获取最新房间信息
            let rooms = crate::server::get_rooms_info_from_state(&server_clone.state()).await;
            let should_send_rooms = if let Some(last) = &last_rooms {
                // 安全地比较序列化结果
                let rooms_str = serde_json::to_string(&rooms).unwrap_or_default();
                let last_str = serde_json::to_string(&last).unwrap_or_default();
                rooms_str != last_str
            } else {
                true
            };
            if should_send_rooms {
                last_rooms = Some(rooms.clone());
                let _ = server_clone.state().ws_tx.send(serde_json::json!({ "type": "RoomsUpdate", "data": rooms }));
            }
            
            // 获取并广播会话信息
            let sessions_response = crate::server::get_sessions_info_from_state(&server_clone.state()).await;
            let should_send_sessions = if let Some(last) = &last_sessions {
                let sessions_str = serde_json::to_string(&sessions_response).unwrap_or_default();
                let last_str = serde_json::to_string(&last).unwrap_or_default();
                sessions_str != last_str
            } else {
                true
            };
            if should_send_sessions {
                last_sessions = Some(sessions_response.clone());
                let _ = server_clone.state().ws_tx.send(serde_json::json!({ "type": "SessionsUpdate", "data": sessions_response }));
            }
            
            // 获取并广播封禁信息
            let bans = server_clone.state().ban_manager.get_all_bans().await;
            let should_send_bans = if let Some(last) = &last_bans {
                let bans_str = serde_json::to_string(&bans).unwrap_or_default();
                let last_str = serde_json::to_string(&last).unwrap_or_default();
                bans_str != last_str
            } else {
                true
            };
            if should_send_bans {
                last_bans = Some(bans.clone());
                let _ = server_clone.state().ws_tx.send(serde_json::json!({ "type": "BansUpdate", "data": bans }));
            }
            
            // 获取并广播维护模式状态
            let maintenance = *server_clone.state().maintenance_mode.read().await;
            let should_send_maintenance = last_maintenance.map(|last| last != maintenance).unwrap_or(true);
            if should_send_maintenance {
                last_maintenance = Some(maintenance);
                let _ = server_clone.state().ws_tx.send(serde_json::json!({ "type": "MaintenanceUpdate", "data": maintenance }));
            }
        }
    });
}

// 登录API处理函数
use rand::Rng;

async fn login_handler(
    State(_state): State<AppState>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 验证用户名和密码
    if validate_credentials(&login_req.username, &login_req.password).await {
        // 生成一个简单的token（实际应用中应使用更安全的token生成方法）
        let mut rng = rand::thread_rng();
        let token: String = (0..32)
            .map(|_| format!("{:x}", rng.r#gen::<u8>() % 16))
            .collect();
        
        // 在实际应用中，应该将token存储在内存或数据库中，以便后续验证
        // 这里为了简化，我们只是返回生成的token
        
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "登录成功",
            "token": token,
            "username": login_req.username
        })))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// WebSocket查询参数
#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

// 房间判定统计WebSocket处理函数
async fn room_judgements_ws_handler(
    State(state): State<AppState>,
    Path(room_id_str): Path<String>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // 验证token
    if !is_valid_token(&query.token).await {
        return Err((StatusCode::UNAUTHORIZED, "未授权".to_string()));
    }
    
    let room_id_obj = RoomId::try_from(room_id_str.clone())
        .map_err(|_e| (StatusCode::BAD_REQUEST, "无效的房间ID格式".to_string()))?;

    let room = {
        let rooms = state.server.state().rooms.read().await;
        rooms.get(&room_id_obj).cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "房间不存在".to_string()))?
    };

    Ok(ws.on_upgrade(move |socket| handle_room_judgements_socket(socket, room)))
}

async fn handle_room_judgements_socket(mut socket: WebSocket, room: Arc<crate::Room>) {
    // 获取当前判定统计状态
    let current_stats = {
        let stats = room.judgement_stats.read().await;
        let stats_vec: Vec<_> = stats.values().collect();
        serde_json::to_string(&stats_vec).unwrap_or_else(|_| "[]".to_string())
    };
    
    // 发送初始状态
    let initial_msg = serde_json::json!({
        "type": "JudgementUpdate",
        "data": serde_json::from_str::<serde_json::Value>(&current_stats).unwrap_or(serde_json::json!([]))
    });
    if let Ok(msg) = serde_json::to_string(&initial_msg) {
        let _ = socket.send(WsMessageAxum::Text(msg)).await;
    }
    
    // 订阅判定统计广播
    let mut rx = room.subscribe_judgements();
    
    // 监听广播并发送给客户端
    while let Ok(msg) = rx.recv().await {
        let ws_msg = serde_json::json!({
            "type": "JudgementUpdate",
            "data": serde_json::from_str::<serde_json::Value>(&msg).unwrap_or(serde_json::json!([]))
        });
        if let Ok(json_msg) = serde_json::to_string(&ws_msg) {
            if socket.send(WsMessageAxum::Text(json_msg)).await.is_err() {
                break;
            }
        }
    }
}
