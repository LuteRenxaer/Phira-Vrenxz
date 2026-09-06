use crate::{
    l10n::{Language, LANGUAGE},
    tl, BanInfo, BanType, Chart, InternalRoomState, Record, Room, ServerState,
};
use anyhow::{anyhow, bail, Result};
use phira_mp_common::{
    ClientCommand, HEARTBEAT_DISCONNECT_TIMEOUT, JoinRoomResponse, Message, ServerCommand, Stream,
    UserInfo,
};
use serde::Deserialize;
use std::{
    collections::{HashSet, hash_map::Entry},
    ops::DerefMut,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::{Mutex, Notify, OnceCell, RwLock, oneshot},
    task::JoinHandle,
    time,
};
use tracing::{Instrument, debug, debug_span, error, info, trace, warn};
use uuid::Uuid;

const HOST: &str = "https://phira.5wyxi.com";

pub struct User {
    pub id: i32,
    pub name: String,
    pub lang: Language,

    pub server: Arc<ServerState>,
    pub session: RwLock<Option<Weak<Session>>>,
    pub room: RwLock<Option<Arc<Room>>>,

    pub monitor: AtomicBool,
    pub game_time: AtomicU32,

    pub dangle_mark: Mutex<Option<Arc<()>>>,
}

impl User {
    pub fn new(id: i32, name: String, lang: Language, server: Arc<ServerState>) -> Self {
        Self {
            id,
            name,
            lang,
            server,
            session: RwLock::default(),
            room: RwLock::default(),
            monitor: AtomicBool::default(),
            game_time: AtomicU32::default(),
            dangle_mark: Mutex::default(),
        }
    }

    pub fn to_info(&self) -> UserInfo {
        UserInfo {
            id: self.id,
            name: self.name.clone(),
            monitor: self.monitor.load(Ordering::SeqCst),
        }
    }

    pub fn can_monitor(&self) -> bool {
        self.server.config.monitors.contains(&self.id)
    }

    pub async fn set_session(&self, session: Weak<Session>) {
        *self.session.write().await = Some(session);
        *self.dangle_mark.lock().await = None;
    }

    pub async fn try_send(&self, cmd: ServerCommand) {
        if let Some(session) = self.session.read().await.as_ref().and_then(Weak::upgrade) {
            session.try_send(cmd).await;
        } else {
            warn!("sending {cmd:?} to dangling user {}", self.id);
        }
    }

    pub async fn dangle(self: Arc<Self>) {
        warn!(user = self.id, "user dangling");
        let guard = self.room.read().await;
        let room = guard.as_ref().map(Arc::clone);
        drop(guard);
        if let Some(room) = room {
            let playing = matches!(*room.state.read().await, InternalRoomState::Playing { .. });
            if playing {
                // 多人房间：游玩中断连立即中止并移除该玩家，避免拖住其他玩家结算。
                // 单人房间：若只剩该玩家，保留房间并落入下方 10 秒重连宽限逻辑，
                // 避免一次短暂断连/重连就摧毁整个房间，否则游玩结束后离开房间
                // 会因 user.room 已为 None 而得到 "no room" 报错。
                let solo = room.users().await.len() <= 1;
                if !solo {
                    warn!(user = self.id, "lost connection on playing, aborting");
                    self.server.users.write().await.remove(&self.id);
                    if room.on_user_leave(&self).await {
                        self.server.rooms.write().await.remove(&room.id);
                    }
                    return;
                }
            }
        }
        let dangle_mark = Arc::new(());
        *self.dangle_mark.lock().await = Some(Arc::clone(&dangle_mark));
        tokio::spawn(async move {
            time::sleep(Duration::from_secs(10)).await;
            if Arc::strong_count(&dangle_mark) > 1 {
                let guard = self.room.read().await;
                let room = guard.as_ref().map(Arc::clone);
                drop(guard);
                if let Some(room) = room {
                    self.server.users.write().await.remove(&self.id);
                    if room.on_user_leave(&self).await {
                        self.server.rooms.write().await.remove(&room.id);
                    }
                }
            }
        });
    }
}

pub struct Session {
    pub id: Uuid,
    pub stream: Stream<ServerCommand, ClientCommand>,
    pub user: Arc<User>,

    monitor_task_handle: JoinHandle<()>,
}

impl Session {
    pub async fn new(id: Uuid, stream: TcpStream, server: Arc<ServerState>, is_blacklisted: bool) -> Result<Arc<Self>> {
        stream.set_nodelay(true)?;
        let addr = stream.peer_addr()?;
        let this = Arc::new(OnceCell::<Arc<Session>>::new());
        let this_inited = Arc::new(Notify::new());
        let (tx, rx) = oneshot::channel::<Arc<User>>();
        let last_recv: Arc<std::sync::Mutex<Instant>> = Arc::new(std::sync::Mutex::new(Instant::now()));
        // 大帧（如本地谱面包上传）接收期间也要持续刷新心跳时间戳，
        // 否则一次较慢的大帧传输会触发心跳超时，被误判为掉线并销毁房间。
        let activity_last_recv = Arc::clone(&last_recv);
        let activity: Option<Arc<dyn Fn() + Send + Sync>> = Some(Arc::new(move || {
            *activity_last_recv.lock().unwrap() = Instant::now();
        }));
        let stream = Stream::<ServerCommand, ClientCommand>::new(
            None,
            stream,
            activity,
            Box::new({
                let this = Arc::clone(&this);
                let this_inited = Arc::clone(&this_inited);
                let mut tx = Some(tx);
                let server = Arc::clone(&server);
                let last_recv = Arc::clone(&last_recv);
                let waiting_for_authenticate = Arc::new(AtomicBool::new(true));
                let panicked = Arc::new(AtomicBool::new(false));
                
                // 检查IP黑名单状态
                let ip_str = addr.ip().to_string();
                let ip_is_blacklisted = is_blacklisted;
                
                move |send_tx, cmd| {
                    let this = Arc::clone(&this);
                    let this_inited = Arc::clone(&this_inited);
                    let tx = tx.take();
                    let server = Arc::clone(&server);
                    let last_recv = Arc::clone(&last_recv);
                    let waiting_for_authenticate = Arc::clone(&waiting_for_authenticate);
                    let panicked = Arc::clone(&panicked);
                    let ip_str_clone = ip_str.clone();
                    let is_blacklisted_clone = ip_is_blacklisted;
                    
                    async move {
                        *last_recv.lock().unwrap() = Instant::now();
                        if panicked.load(Ordering::SeqCst) {
                            return;
                        }
                        if matches!(cmd, ClientCommand::Ping) {
                            let _ = send_tx.send(ServerCommand::Pong).await;
                            return;
                        }
                        if waiting_for_authenticate.load(Ordering::SeqCst) {
                            if let ClientCommand::Authenticate { token } = cmd {
                                let Some(tx) = tx else { return };
                                
                                // 检查IP是否在黑名单中（再次检查以确保最新状态）
                                let blacklist = server.ip_blacklist.read().await;
                                let ip_in_blacklist = if let Some(expiry_time) = blacklist.get(&ip_str_clone) {
                                    let now = std::time::SystemTime::now();
                                    now.duration_since(*expiry_time).unwrap_or(std::time::Duration::from_secs(0)).as_secs() < 10
                                } else {
                                    false
                                };
                                drop(blacklist);
                                
                                if is_blacklisted_clone || ip_in_blacklist {
                                    // IP在黑名单中，发送403错误
                                    warn!("拒绝来自黑名单IP {ip_str_clone} 的认证请求");
                                    let _ = send_tx
                                        .send(ServerCommand::Authenticate(Err("该账号已被封禁，无法连接该服务器".to_string())))
                                        .await;
                                    // 等待0.5秒让消息发送完成
                                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                    // 设置恐慌状态以断开连接
                                    panicked.store(true, Ordering::SeqCst);
                                    if let Err(err) = server.lost_con_tx.send(id).await {
                                        error!("failed to mark lost connection ({id}): {err:?}");
                                    }
                                    return;
                                }
                                
                                // 获取认证信息
                                let token = token.into_inner();
                                if token.len() > 32 {
                                    warn!("invalid token");
                                    let _ = send_tx
                                        .send(ServerCommand::Authenticate(Err("invalid token".to_string())))
                                        .await;
                                    // 等待0.5秒让消息发送完成
                                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                    panicked.store(true, Ordering::SeqCst);
                                    if let Err(err) = server.lost_con_tx.send(id).await {
                                        error!("failed to mark lost connection ({id}): {err:?}");
                                    }
                                    return;
                                }
                                debug!("session {id}: authenticate {token}");
                                #[derive(Debug, Deserialize)]
                                struct UserInfo {
                                    id: i32,
                                    name: String,
                                    language: String,
                                }
                                
                                let resp_result: Result<UserInfo, reqwest::Error> = {
                                    let client_result = reqwest::Client::new()
                                        .get(format!("{HOST}/me"))
                                        .header(
                                            reqwest::header::AUTHORIZATION,
                                            format!("Bearer {token}"),
                                        )
                                        .send()
                                        .await;
                                        
                                    match client_result {
                                        Ok(response) => {
                                            let response_result = response.error_for_status();
                                            match response_result {
                                                Ok(response) => {
                                                    let json_result: reqwest::Result<UserInfo> = response.json().await;
                                                    match json_result {
                                                        Ok(user_info) => Ok(user_info),
                                                        Err(e) => Err(e),
                                                    }
                                                },
                                                Err(e) => Err(e),
                                            }
                                        },
                                        Err(e) => Err(e),
                                    }
                                };
                                
                                let resp = match resp_result {
                                    Ok(resp) => {
                                        debug!("session {id} <- {resp:?}");
                                        resp
                                    },
                                    Err(err) => {
                                        warn!("failed to fetch info: {err:?}");
                                        let _ = send_tx
                                            .send(ServerCommand::Authenticate(Err(err.to_string())))
                                            .await;
                                        // 等待0.5秒让消息发送完成
                                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                        panicked.store(true, Ordering::SeqCst);
                                        if let Err(err) = server.lost_con_tx.send(id).await {
                                            error!("failed to mark lost connection ({id}): {err:?}");
                                        }
                                        return;
                                    }
                                };
                                
                                // 在移动resp之前保存需要的值
                                let user_id = resp.id;
                                let user_name = resp.name.clone();
                                let user_language = resp.language;
                                
                                // 检查是否处于维护模式
                                let is_maintenance = *server.maintenance_mode.read().await;
                                if is_maintenance {
                                    let whitelist = server.maintenance_whitelist.read().await;
                                    if !whitelist.contains(&user_id) {
                                        warn!("拒绝非白名单用户 {user_id} ({user_name}) 在维护模式下的连接");
                                        let maintenance_message = "当前服务器正在维护，为了保证游戏体验，请稍作等待或使用其它服务器，如有疑问请联系服主（2357444016）或加群（702640128）联系".to_string();
                                        
                                        let _ = send_tx
                                            .send(ServerCommand::Authenticate(Err(maintenance_message)))
                                            .await;
                                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                        panicked.store(true, Ordering::SeqCst);
                                        if let Err(err) = server.lost_con_tx.send(id).await {
                                            error!("failed to mark lost connection ({id}): {err:?}");
                                        }
                                        return;
                                    }
                                }
                                
                                // 检查用户ID和IP是否被封禁
                                if let Some(ban_info) = server.ban_manager.is_user_or_ip_banned(user_id, &addr.ip().to_string()).await {
                                    warn!("拒绝被封禁用户 {user_id} ({user_name}) 或IP {} 的认证请求", addr.ip());
                                    let ban_end_timestamp = ban_info.ban_end_time();
                                    // 将Unix时间戳转换为本地时间格式
                                    use std::time::{UNIX_EPOCH, Duration};
                                    let datetime = UNIX_EPOCH + Duration::from_secs(ban_end_timestamp);
                                    let datetime: chrono::DateTime<chrono::Local> = datetime.into();
                                    let formatted_time = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
                                    
                                    let ban_message = format!(
                                        "由于您的账号存在违规行为，账号已被封禁，如有疑问请联系客服。\n封禁结束时间【{}】\n封禁理由：【{}】",
                                        formatted_time,
                                        ban_info.ban_reason
                                    );
                                    
                                    let _ = send_tx
                                        .send(ServerCommand::Authenticate(Err(ban_message)))
                                        .await;
                                    // 等待0.5秒让消息发送完成
                                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                    panicked.store(true, Ordering::SeqCst);
                                    if let Err(err) = server.lost_con_tx.send(id).await {
                                        error!("failed to mark lost connection ({id}): {err:?}");
                                    }
                                    return;
                                }
                                
                                let mut users_guard = server.users.write().await;
                                if let Some(user) = users_guard.get(&resp.id) {
                                    info!("reconnect");
                                    let _ = tx.send(Arc::clone(user));
                                    this_inited.notified().await;
                                    user.set_session(Arc::downgrade(this.get().unwrap()))
                                        .await;
                                } else {
                                    let user = Arc::new(User::new(
                                        resp.id,
                                        resp.name,
                                        user_language
                                            .parse()
                                            .map(Language)
                                            .unwrap_or_default(),
                                        Arc::clone(&server),
                                    ));
                                    let _ = tx.send(Arc::clone(&user));
                                    this_inited.notified().await;
                                    user.set_session(Arc::downgrade(this.get().unwrap()))
                                        .await;
                                    users_guard.insert(resp.id, user);
                                }
                                
                                // 记录连接信息
                                let session_info = crate::SessionInfo {
                                    user_id,
                                    user_name,
                                    ip_address: addr.ip().to_string(),
                                    connect_time: std::time::SystemTime::now(),
                                };
                                server.session_info.write().await.insert(id, session_info);
                                
                                let user = &this.get().unwrap().user;
                                let room_state = match user.room.read().await.as_ref() {
                                    Some(room) => Some(room.client_state(user).await),
                                    None => None,
                                };
                                let _ = send_tx
                                    .send(ServerCommand::Authenticate(Ok((
                                        user.to_info(),
                                        room_state,
                                    ))))
                                    .await;
                                waiting_for_authenticate.store(false, Ordering::SeqCst);
                                return;
                            } else {
                                warn!("packet before authentication, ignoring: {cmd:?}");
                                return;
                            }
                        }
                        let user = this.get().map(|it| Arc::clone(&it.user)).unwrap();
                        if let Some(resp) = LANGUAGE
                            .scope(Arc::new(user.lang.clone()), process(user, cmd))
                            .await
                            && let Err(err) = send_tx.send(resp).await
                        {
                            error!("failed to handle message, aborting connection {id}: {err:?}",);
                            panicked.store(true, Ordering::SeqCst);
                            if let Err(err) = server.lost_con_tx.send(id).await {
                                error!("failed to mark lost connection ({id}): {err:?}");
                            }
                        }
                    }
                }
            }),
        )
        .await?;
        let monitor_task_handle = tokio::spawn({
            let last_recv = Arc::clone(&last_recv);
            async move {
                loop {
                    let recv = *last_recv.lock().unwrap();
                    time::sleep_until((recv + HEARTBEAT_DISCONNECT_TIMEOUT).into()).await;

                    if *last_recv.lock().unwrap() + HEARTBEAT_DISCONNECT_TIMEOUT > Instant::now() {
                        continue;
                    }

                    if let Err(err) = server.lost_con_tx.send(id).await {
                        error!("failed to mark lost connection ({id}): {err:?}");
                    }
                    break;
                }
            }
        });

        let user = rx.await?;

        let res = Arc::new(Self {
            id,
            stream,
            user,

            monitor_task_handle,
        });
        let _ = this.set(Arc::clone(&res));
        this_inited.notify_one();
        Ok(res)
    }

    pub fn version(&self) -> u8 {
        self.stream.version()
    }

    pub fn name(&self) -> &str {
        &self.user.name
    }

    pub async fn try_send(&self, cmd: ServerCommand) {
        if let Err(err) = self.stream.send(cmd).await {
            error!("failed to deliver command to {}: {err:?}", self.id);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.monitor_task_handle.abort();
    }
}

async fn handle_played_score(
    user: Arc<User>,
    id: i32,
    score: u32,
    accuracy: f32,
    full_combo: bool,
    max_combo: u32,
    perfect: u32,
    good: u32,
    bad: u32,
    miss: u32,
) -> Result<()> {
    let room = user
        .room
        .read()
        .await
        .as_ref()
        .map(Arc::clone)
        .ok_or_else(|| anyhow!("芙宁娜的房间已经关了"))?;
    let record = Record {
        id,
        player: user.id,
        score: score as i32,
        perfect: perfect as i32,
        good: good as i32,
        bad: bad as i32,
        miss: miss as i32,
        max_combo: max_combo as i32,
        accuracy,
        full_combo,
        std: 0.,
        std_score: 0.,
    };
    debug!(room = room.id.to_string(), user = user.id, "user played: {record:?}");
    room.update_final_stats(user.id, user.name.clone(), &record).await;
    room.send(Message::Played {
        user: user.id,
        score: record.score,
        accuracy: record.accuracy,
        full_combo: record.full_combo,
    })
    .await;

    let mut guard = room.state.write().await;
    if let InternalRoomState::Playing { results, aborted } = guard.deref_mut() {
        if aborted.contains(&user.id) {
            bail!("aborted");
        }
        if results.insert(user.id, record).is_some() {
            bail!("already uploaded");
        }
        let users = room.users().await;
        let all_finished = users
            .iter()
            .all(|it| results.contains_key(&it.id) || aborted.contains(&it.id));
        info!(
            room_id = %room.id.to_string(),
            user_count = users.len(),
            results_count = results.len(),
            aborted_count = aborted.len(),
            all_finished = all_finished,
            "Checking if all players finished"
        );
        drop(guard);
        room.check_all_ready().await;

        if all_finished {
            info!(room_id = %room.id.to_string(), "All players finished, will save and upload replays");
            let room_id = room.id.to_string();
            let server = Arc::clone(&user.server);
            let upload_config = server.config.upload.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let mut replay_manager = server.replay_manager.write().await;
                if upload_config.enabled && !upload_config.api_token.is_empty() {
                    match replay_manager.save_and_upload_replays(&room_id, &upload_config.api_url, &upload_config.api_token).await {
                        Ok(results) => {
                            let saved_count = results.len();
                            let uploaded_count = results.iter().filter(|(_, r)| r.is_some()).count();
                            let success_count = results.iter().filter(|(_, r)| r.as_ref().map(|r| r.success).unwrap_or(false)).count();
                            info!(room_id = %room_id, saved = saved_count, uploaded = uploaded_count, success = success_count, "Saved and uploaded replay recordings");
                        }
                        Err(e) => error!(room_id = %room_id, error = %e, "Failed to save and upload replay recordings"),
                    }
                } else {
                    match replay_manager.save_replays(&room_id).await {
                        Ok(paths) => info!(room_id = %room_id, file_count = paths.len(), "Saved replay recordings (upload disabled)"),
                        Err(e) => error!(room_id = %room_id, error = %e, "Failed to save replay recordings"),
                    }
                }
                replay_manager.remove_room_manager(&room_id);
            });
        }
    }
    Ok(())
}

async fn process(user: Arc<User>, cmd: ClientCommand) -> Option<ServerCommand> {
    #[inline]
    fn err_to_str<T>(result: Result<T>) -> Result<T, String> {
        result.map_err(|it| it.to_string())
    }

    macro_rules! get_room {
        (~ $d:ident) => {
            let $d = match user.room.read().await.as_ref().map(Arc::clone) {
                Some(room) => room,
                None => {
                    warn!("芙宁娜的房间已经关了");
                    return None;
                }
            };
        };
        ($d:ident) => {
            let $d = user
                .room
                .read()
                .await
                .as_ref()
                .map(Arc::clone)
                .ok_or_else(|| anyhow!("芙宁娜的房间已经关了"))?;
        };
        ($d:ident, $($pt:tt)*) => {
            let $d = user
                .room
                .read()
                .await
                .as_ref()
                .map(Arc::clone)
                .ok_or_else(|| anyhow!("芙宁娜的房间已经关了"))?;
            if !matches!(&*$d.state.read().await, $($pt)*) {
                bail!("invalid state");
            }
        };
    }
    match cmd {
        ClientCommand::Ping => unreachable!(),
        ClientCommand::Authenticate { .. } => Some(ServerCommand::Authenticate(Err(
            "repeated authenticate".to_owned(),
        ))),
        ClientCommand::Chat { message } => {
            let res: Result<()> = async move {
                get_room!(room);
                room.send_as(&user, message.into_inner()).await;
                Ok(())
            }
            .await;
            Some(ServerCommand::Chat(err_to_str(res)))
        }
        ClientCommand::Touches { frames } => {
            get_room!(~ room);
            if room.is_live() {
                debug!("received {} touch events from {}", frames.len(), user.id);
                if let Some(frame) = frames.last() {
                    user.game_time.store(frame.time.to_bits(), Ordering::SeqCst);
                }
                
                // 录制Touch事件
                let room_id = room.id.to_string();
                let user_id = user.id;
                let frames_clone = frames.clone();
                let server = Arc::clone(&user.server);
                tokio::spawn(async move {
                    let mut replay_manager = server.replay_manager.write().await;
                    replay_manager.record_touch_frames(&room_id, user_id, &frames_clone);
                });
                
                tokio::spawn(async move {
                    room.broadcast_monitors(ServerCommand::Touches {
                        player: user.id,
                        frames,
                    })
                    .await;
                });
            } else {
                warn!("received touch events in non-live mode");
            }
            None
        }
        ClientCommand::Judges { judges } => {
            // 检查用户是否在房间中
            let room_opt = user.room.read().await.as_ref().map(Arc::clone);
            match room_opt {
                Some(room) => {
                    let user_name = user.name.clone();
                    let user_id = user.id;
                    let room_id = room.id.clone();
                    let judges_clone = judges.clone();
                    let judges_clone2 = judges.clone();
                    
                    // 录制Judge事件
                    let room_id_str = room_id.to_string();
                    let server = Arc::clone(&user.server);
                    tokio::spawn(async move {
                        let mut replay_manager = server.replay_manager.write().await;
                        replay_manager.record_judge_events(&room_id_str, user_id, &judges_clone);
                    });
                    
                    tokio::spawn(async move {
                        // 使用批量更新方法，只广播一次
                        room.update_judgement_stats_batch(user_id, user_name, &judges_clone2).await;
                        
                        // 如果房间是live模式，广播给观察者（用于观战）
                        if room.is_live() {
                            room.broadcast_monitors(ServerCommand::Judges {
                                player: user_id,
                                judges: judges_clone2,
                            })
                            .await;
                        }
                    });
                }
                None => {
                    warn!("user {} sent judges but not in any room", user.id);
                }
            }
            None
        }
        ClientCommand::CreateRoom { id } => {
            let res: Result<()> = async move {
                let mut room_guard = user.room.write().await;
                if room_guard.is_some() {
                    bail!("already in room");
                }

                let mut map_guard = user.server.rooms.write().await;
                let room = Arc::new(Room::new(id.clone(), Arc::downgrade(&user), Some(user.id)));
                match map_guard.entry(id.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(Arc::clone(&room));
                    }
                    Entry::Occupied(_) => {
                        bail!(tl!("create-id-occupied"));
                    }
                }
                drop(map_guard);
                
                // 设置用户的房间
                *room_guard = Some(room.clone());
                drop(room_guard);
                
                // 发送创建房间消息（使用spawn避免阻塞）
                let room_clone = room.clone();
                let user_id = user.id;
                let user_name = user.name.clone();
                tokio::spawn(async move {
                    room_clone.send(Message::CreateRoom { user: user_id, name: user_name }).await;
                });

                info!(user = user.id, room = id.to_string(), "user create room");
                
                // 广播房间更新
                let state_clone = Arc::clone(&user.server);
                tokio::spawn(async move {
                    crate::server::broadcast_rooms_update(&state_clone).await;
                });
                
                // 初始化回放录制管理器
                let chart_id = room.chart.read().await.as_ref().map(|c| c.id).unwrap_or(0);
                let chart_name = room.chart.read().await.as_ref().map(|c| c.name.clone()).unwrap_or_default();
                let room_id_str = id.to_string();
                {
                    let mut replay_manager = user.server.replay_manager.write().await;
                    replay_manager.create_room_manager(room_id_str.clone(), chart_id, chart_name.clone());
                    // 初始化房主和录制器的缓存
                    replay_manager.init_player(&room_id_str, user.id, user.name.clone());
                    replay_manager.init_player(&room_id_str, crate::replay::RECORDER_BOT_USER_ID, crate::replay::RECORDER_BOT_USER_NAME.to_string());
                    info!(room_id = %room_id_str, chart_id = chart_id, chart_name = %chart_name, "Created replay manager for room");
                }
                
                // 普通房间创建后0.5秒，让虚拟monitor用户进入房间
                let room_clone = room.clone();
                let room_id = id.clone();
                let server = Arc::clone(&user.server);
                let creator_id = user.id;
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    
                    // 获取或创建ID为2的虚拟monitor用户
                    let monitor_user = {
                        let users = server.users.read().await;
                        if let Some(user) = users.get(&2) {
                            Arc::clone(user)
                        } else {
                            drop(users);
                            // 创建虚拟monitor用户
                            let virtual_user = Arc::new(User::new(
                                2,
                                "Monitor".to_string(),
                                Language::default(),
                                Arc::clone(&server),
                            ));
                            // 设置monitor标志
                            virtual_user.monitor.store(true, Ordering::SeqCst);
                            // 添加到服务器用户列表
                            server.users.write().await.insert(2, Arc::clone(&virtual_user));
                            info!("创建虚拟monitor用户(ID=2)");
                            virtual_user
                        }
                    };
                    
                    // 检查房间是否还是非live状态（普通房间）
                    if !room_clone.is_live() && !room_clone.is_competition.load(Ordering::SeqCst) {
                        // 确保用户的monitor标志为true
                        monitor_user.monitor.store(true, Ordering::SeqCst);
                        
                        // 添加monitor用户到房间
                        if room_clone.add_user(Arc::downgrade(&monitor_user), true).await {
                            // 设置房间为live状态
                            room_clone.live.store(true, Ordering::SeqCst);
                            info!(room = room_id.to_string(), "普通房间自动添加monitor用户(ID=2)并设置为live状态");
                            
                            // 获取monitor用户信息（monitor标志已设置）
                            let user_info = monitor_user.to_info();
                            let monitor_name = monitor_user.name.clone();
                            
                            // 向房间内所有用户广播monitor加入（包括创建者）
                            room_clone.broadcast(ServerCommand::OnJoinRoom(user_info.clone())).await;
                            
                            // 向房间内所有用户发送JoinRoom消息
                            room_clone.send(Message::JoinRoom {
                                user: 2,
                                name: monitor_name.clone(),
                            }).await;
                            
                            // 额外向创建者发送房间状态更新，确保创建者知道房间现在是live状态
                            if let Some(creator) = server.users.read().await.get(&creator_id) {
                                if let Some(ref room_ref) = *creator.room.read().await {
                                    let client_state = room_ref.client_state(creator).await;
                                    creator.try_send(ServerCommand::ChangeState(client_state.state)).await;
                                }
                            }
                            
                            tracing::info!("已向房间 {} 广播monitor用户(ID=2, monitor=true)加入消息", room_id);
                        }
                    }
                    
                    // 创建或获取回放录制器虚拟用户
                    let recorder_user = {
                        let users = server.users.read().await;
                        if let Some(user) = users.get(&crate::replay::RECORDER_BOT_USER_ID) {
                            Arc::clone(user)
                        } else {
                            drop(users);
                            // 创建虚拟录制器用户
                            let virtual_user = Arc::new(User::new(
                                crate::replay::RECORDER_BOT_USER_ID,
                                crate::replay::RECORDER_BOT_USER_NAME.to_string(),
                                Language::default(),
                                Arc::clone(&server),
                            ));
                            // 设置bot标志
                            virtual_user.monitor.store(true, Ordering::SeqCst);
                            // 添加到服务器用户列表
                            server.users.write().await.insert(crate::replay::RECORDER_BOT_USER_ID, Arc::clone(&virtual_user));
                            info!("创建回放录制器虚拟用户(ID={})", crate::replay::RECORDER_BOT_USER_ID);
                            virtual_user
                        }
                    };
                    
                    // 添加录制器用户到房间（作为monitor，不参与游戏）
                    if room_clone.add_user(Arc::downgrade(&recorder_user), true).await {
                        info!(room = room_id.to_string(), "回放录制器已加入房间");
                        
                        // 向房间内所有用户广播录制器加入
                        let user_info = recorder_user.to_info();
                        room_clone.broadcast(ServerCommand::OnJoinRoom(user_info)).await;
                        
                        room_clone.send(Message::JoinRoom {
                            user: crate::replay::RECORDER_BOT_USER_ID,
                            name: recorder_user.name.clone(),
                        }).await;
                    }
                    
                    // 再次广播房间更新（monitor加入后）
                    crate::server::broadcast_rooms_update(&server).await;
                });
                
                Ok(())
            }
            .await;
            Some(ServerCommand::CreateRoom(err_to_str(res)))
        }
        ClientCommand::JoinRoom { id, monitor } => {
            let res: Result<JoinRoomResponse> = async move {
                let mut room_guard = user.room.write().await;
                if room_guard.is_some() {
                    bail!("already in room");
                }
                let room = user.server.rooms.read().await.get(&id).map(Arc::clone);
                let Some(room) = room else { bail!("芙宁娜都还没开房间呢，这么着急干啥？") };
                if room.locked.load(Ordering::SeqCst) {
                    bail!(tl!("join-room-locked"));
                }
                if !matches!(*room.state.read().await, InternalRoomState::SelectChart) {
                    bail!(tl!("join-game-ongoing"));
                }
                if monitor && !user.can_monitor() {
                    bail!(tl!("join-cant-monitor"));
                }
                if !room.add_user(Arc::downgrade(&user), monitor).await {
                    bail!(tl!("join-room-full"));
                }
                info!(
                    user = user.id,
                    room = id.to_string(),
                    monitor,
                    "user join room"
                );
                user.monitor.store(monitor, Ordering::SeqCst);
                if monitor && !room.live.fetch_or(true, Ordering::SeqCst) {
                    info!(room = id.to_string(), "room goes live");
                }
                room.broadcast(ServerCommand::OnJoinRoom(user.to_info()))
                    .await;
                // 发送欢迎消息
                let web_url = if let Some(_web_port) = user.server.config.web_port {
                    format!("你可以使用【云崽】芙卡洛斯的 #phira 指令来查看该服务器的房间列表。")
                } else {
                    "".to_string()
                };
                room.broadcast(ServerCommand::Message(Message::Chat {
                    user: 0, // 使用0表示系统消息
                    content: format!("欢迎 {} 加入房间！{}", user.name, web_url),
                }))
                .await;
                room.send(Message::JoinRoom {
                    user: user.id,
                    name: user.name.clone(),
                })
                .await;
                *room_guard = Some(Arc::clone(&room));
                
                // 初始化新加入玩家的回放缓存
                if !monitor {
                    let room_id = id.to_string();
                    let user_id = user.id;
                    let user_name = user.name.clone();
                    let server = Arc::clone(&user.server);
                    tokio::spawn(async move {
                        let mut replay_manager = server.replay_manager.write().await;
                        replay_manager.init_player(&room_id, user_id, user_name);
                        info!(room_id = %room_id, user_id = user_id, "Initialized replay cache for joined player");
                    });
                }
                
                // 广播房间和会话更新
                let state_clone = Arc::clone(&user.server);
                tokio::spawn(async move {
                    crate::server::broadcast_rooms_update(&state_clone).await;
                    crate::server::broadcast_sessions_update(&state_clone).await;
                });
                
                Ok(JoinRoomResponse {
                    state: room.client_room_state().await,
                    users: room
                        .users()
                        .await
                        .into_iter()
                        .chain(room.monitors().await.into_iter())
                        .map(|it| it.to_info())
                        .collect(),
                    live: room.is_live(),
                })
            }
            .await;
            Some(ServerCommand::JoinRoom(err_to_str(res)))
        }
        ClientCommand::LeaveRoom => {
            let res: Result<()> = async move {
                // 若用户已不在任何房间（例如单人房间游玩中断连导致房间被清理，
                // user.room 已为 None），离开应视为成功的空操作，而不是返回
                // "no room" 报错，否则客户端将无法退出并一直看到该错误。
                let Some(room) = user.room.read().await.as_ref().map(Arc::clone) else {
                    info!(user = user.id, "user not in any room, leave is no-op");
                    return Ok(());
                };
                info!(
                    user = user.id,
                    room = room.id.to_string(),
                    "user leave room"
                );
                if room.on_user_leave(&user).await {
                    user.server.rooms.write().await.remove(&room.id);
                }
                
                // 广播房间和会话更新
                let state_clone = Arc::clone(&user.server);
                tokio::spawn(async move {
                    crate::server::broadcast_rooms_update(&state_clone).await;
                    crate::server::broadcast_sessions_update(&state_clone).await;
                });
                
                Ok(())
            }
            .await;
            Some(ServerCommand::LeaveRoom(err_to_str(res)))
        }
        ClientCommand::LockRoom { lock } => {
            let res: Result<()> = async move {
                get_room!(room);
                room.check_host(&user).await?;
                info!(
                    user = user.id,
                    room = room.id.to_string(),
                    lock,
                    "lock room"
                );
                room.locked.store(lock, Ordering::SeqCst);
                room.send(Message::LockRoom { lock }).await;
                Ok(())
            }
            .await;
            Some(ServerCommand::LockRoom(err_to_str(res)))
        }
        ClientCommand::CycleRoom { cycle } => {
            let res: Result<()> = async move {
                get_room!(room);
                room.check_host(&user).await?;
                info!(
                    user = user.id,
                    room = room.id.to_string(),
                    cycle,
                    "cycle room"
                );
                room.cycle.store(cycle, Ordering::SeqCst);
                room.send(Message::CycleRoom { cycle }).await;
                Ok(())
            }
            .await;
            Some(ServerCommand::CycleRoom(err_to_str(res)))
        }
        ClientCommand::SelectChart { id } => {
            let res: Result<()> = async move {
                get_room!(room, InternalRoomState::SelectChart);
                room.check_host(&user).await?;
                let span = debug_span!(
                    "select chart",
                    user = user.id,
                    room = room.id.to_string(),
                    chart = id,
                );
                async move {
                    trace!("fetch");
                    let res: Chart = reqwest::get(format!("{HOST}/chart/{id}"))
                        .await?
                        .error_for_status()?
                        .json()
                        .await?;
                    debug!("chart is {res:?}");
                    
                    // 更新 ReplayManager 的谱面信息
                    let room_id = room.id.to_string();
                    let chart_id = res.id;
                    let chart_name = res.name.clone();
                    let chart_name_for_log = chart_name.clone();
                    let server = Arc::clone(&user.server);
                    tokio::spawn(async move {
                        let mut replay_manager = server.replay_manager.write().await;
                        info!(room_id = %room_id, "Attempting to update replay manager chart info");
                        if let Some(manager) = replay_manager.get_room_manager(&room_id) {
                            manager.chart_id = chart_id;
                            manager.chart_name = chart_name;
                            info!(room_id = %room_id, chart_id = chart_id, chart_name = %chart_name_for_log, "Updated replay manager chart info");
                        } else {
                            warn!(room_id = %room_id, "Replay manager not found for room");
                        }
                    });
                    
                    room.send(Message::SelectChart {
                        user: user.id,
                        name: res.name.clone(),
                        id: res.id,
                    })
                    .await;
                    *room.chart.write().await = Some(res);
                    room.on_state_change().await;
                    Ok(())
                }
                .instrument(span)
                .await
            }
            .await;
            Some(ServerCommand::SelectChart(err_to_str(res)))
        }

        // 房主选择本地谱面，房间进入 LocalChart 分享阶段
        ClientCommand::SelectLocalChart { id, name } => {
            let res: Result<()> = async move {
                get_room!(room, InternalRoomState::SelectChart);
                room.check_host(&user).await?;
                let id = id.into_inner();
                let name = name.into_inner();
                info!(
                    room = room.id.to_string(),
                    user = user.id,
                    chart = %id,
                    "host selected local chart"
                );
                // 记录本地谱面 (UUID id, name)
                *room.local_chart.write().await = Some((id.clone(), name.clone()));
                // 房间进入 LocalChart 状态
                *room.state.write().await = InternalRoomState::LocalChart {
                    started: HashSet::new(),
                };
                // 广播给房间内所有人：进入本地谱面分享
                room.broadcast(ServerCommand::ChangeLocalChart { local: true, chart_id: id.clone() })
                    .await;
                room.send(Message::SelectLocalChart {
                    user: user.id,
                    name,
                    id,
                })
                .await;
                room.on_state_change().await;
                Ok(())
            }
            .await;
            Some(ServerCommand::SelectLocalChart(err_to_str(res)))
        }

        // 房主改选在线谱面，取消本地谱面分享
        ClientCommand::SelectOnlineChart { id } => {
            let res: Result<()> = async move {
                get_room!(room);
                room.check_host(&user).await?;
                // 若当前处于本地谱面分享状态，先取消
                if matches!(*room.state.read().await, InternalRoomState::LocalChart { .. }) {
                    info!(room = room.id.to_string(), user = user.id, "cancel local chart sharing");
                    *room.local_chart.write().await = None;
                    room.broadcast(ServerCommand::ChangeLocalChart { local: false, chart_id: String::new() })
                        .await;
                    *room.state.write().await = InternalRoomState::SelectChart;
                }
                // 校验必须在 SelectChart 状态（取消后即是）
                {
                    let state = room.state.read().await;
                    if !matches!(*state, InternalRoomState::SelectChart) {
                        bail!(tl!("chart-select-not-now"));
                    }
                }
                let span = debug_span!(
                    "select online chart",
                    user = user.id,
                    room = room.id.to_string(),
                    chart = id,
                );
                async move {
                    trace!("fetch");
                    let res: Chart = reqwest::get(format!("{HOST}/chart/{id}"))
                        .await?
                        .error_for_status()?
                        .json()
                        .await?;
                    debug!("chart is {res:?}");
                    room.send(Message::SelectChart {
                        user: user.id,
                        name: res.name.clone(),
                        id: res.id,
                    })
                    .await;
                    *room.chart.write().await = Some(res);
                    room.on_state_change().await;
                    Ok(())
                }
                .instrument(span)
                .await
            }
            .await;
            Some(ServerCommand::SelectOnlineChart(err_to_str(res)))
        }

        // 房主作为下载服务器就绪，通知玩家开始从服务端下载
        ClientCommand::SendChart { .. } => {
            let res: Result<()> = async move {
                get_room!(room);
                room.check_host(&user).await?;
                let (chart_id, chart_name) = {
                    let guard = room.local_chart.read().await;
                    guard.clone().ok_or_else(|| anyhow::anyhow!(tl!("start-no-chart-selected")))?
                };
                info!(
                    room = room.id.to_string(),
                    user = user.id,
                    chart = %chart_id,
                    "host is sharing local chart via server relay"
                );
                room.send(Message::SendChart { user: user.id }).await;
                // 服务端打洞/中转：把服务端下载地址分发给所有需要下载的玩家。
                // 地址为 服务端公网IP:web端口 + chart_uuid（玩家经服务端下载，无需直连房主）。
                // 忽略房主（房主不会收到自己的消息）与回放录制器（ID = -999）
                let host_name = user.name.clone();
                let public_ip = user.server.public_ip.read().await.clone();
                let web_port = user.server.config.web_port.unwrap_or(0);
                let recorder_id = crate::replay::RECORDER_BOT_USER_ID;
                for player in room.users().await {
                    if player.id == user.id || player.id == recorder_id {
                        continue;
                    }
                    player
                        .try_send(ServerCommand::StartDownload {
                            host_id: user.id,
                            host_name: host_name.clone(),
                            addr: public_ip.clone(),
                            port: web_port,
                            chart_id: chart_id.clone(),
                            chart_name: chart_name.clone(),
                        })
                        .await;
                }
                // 触发就绪判定：若无其他需要下载的玩家（如房间只有房主），立即通知房主并开始游戏
                room.check_all_ready().await;
                Ok(())
            }
            .await;
            Some(ServerCommand::SendChart(err_to_str(res)))
        }

        // 玩家下载完成，标记就绪
        ClientCommand::DownloadReady => {
            let res: Result<()> = async move {
                get_room!(room);
                // 房主尚未上传完谱面时，禁止其他玩家提前就绪
                if !room.chart_uploaded.load(Ordering::SeqCst) {
                    bail!(tl!("chart-not-uploaded-yet"));
                }
                let mut guard = room.state.write().await;
                if let InternalRoomState::LocalChart { started } = guard.deref_mut() {
                    if !started.insert(user.id) {
                        bail!("already ready");
                    }
                    info!(room = room.id.to_string(), user = user.id, "local chart downloaded");
                    room.send(Message::DownloadReady { user: user.id }).await;
                    drop(guard);
                    room.check_all_ready().await;
                }
                Ok(())
            }
            .await;
            Some(ServerCommand::DownloadReady(err_to_str(res)))
        }

        // 本地谱面分享：房主上传谱面包到服务端中转（经 game 连接，兼容内网穿透）
        ClientCommand::UploadChart { id, data } => {
            let res: Result<()> = async move {
                let id = id.into_inner();
                let size = data.len();
                user.server.chart_cache.write().await.insert(id.clone(), data);
                // 标记该房间房主已上传完成，玩家此后才可就绪/下载
                if let Some(room) = user.room.read().await.as_ref().map(Arc::clone) {
                    room.chart_uploaded.store(true, Ordering::SeqCst);
                }
                info!(user = user.id, chart = %id, size, "host uploaded chart");
                Ok(())
            }
            .await;
            Some(ServerCommand::UploadChart(err_to_str(res)))
        }

        // 本地谱面分享：玩家从服务端获取谱面包
        ClientCommand::DownloadChart { id } => {
            let res: Result<Vec<u8>> = async move {
                let id = id.into_inner();
                user.server
                    .chart_cache
                    .read()
                    .await
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| anyhow!("chart not found: {id}"))
            }
            .await;
            Some(ServerCommand::DownloadChart(err_to_str(res)))
        }

        // 房主取消本地谱面分享：删除服务端缓存、重置所有玩家就绪状态
        ClientCommand::CancelLocalChart => {
            let res: Result<()> = async move {
                get_room!(room);
                room.check_host(&user).await?;
                info!(room = room.id.to_string(), user = user.id, "host canceled local chart sharing");
                if let Some((chart_id, _)) = room.local_chart.read().await.clone() {
                    user.server.chart_cache.write().await.remove(&chart_id);
                    info!(room = room.id.to_string(), chart = %chart_id, "cleared chart cache on cancel");
                }
                room.chart_uploaded.store(false, Ordering::SeqCst);
                // 重置 LocalChart 就绪集合，让所有玩家可以重新准备
                if let InternalRoomState::LocalChart { started } = &mut *room.state.write().await {
                    started.clear();
                }
                // 广播取消事件，让所有客户端重置就绪/开始按钮状态
                room.broadcast(ServerCommand::LocalChartCanceled).await;
                room.on_state_change().await;
                Ok(())
            }
            .await;
            Some(ServerCommand::CancelLocalChart(err_to_str(res)))
        }

        // 玩家取消已就绪（尚未开始游玩前可取消）
        ClientCommand::CancelDownloadReady => {
            let res: Result<()> = async move {
                get_room!(room);
                let mut guard = room.state.write().await;
                if let InternalRoomState::LocalChart { started } = guard.deref_mut() {
                    started.remove(&user.id);
                    info!(room = room.id.to_string(), user = user.id, "player canceled local chart ready");
                }
                Ok(())
            }
            .await;
            Some(ServerCommand::CancelDownloadReady(err_to_str(res)))
        }

        ClientCommand::RequestStart => {
            let res: Result<()> = async move {
                get_room!(room);
                room.check_host(&user).await?;
                // 本地谱面分享阶段：房主点开始 -> 通知房主启动下载服务器
                if matches!(*room.state.read().await, InternalRoomState::LocalChart { .. }) {
                    let (chart_id, chart_name) = {
                        let guard = room.local_chart.read().await;
                        let (id, name) = guard.clone().ok_or_else(|| anyhow::anyhow!(tl!("start-no-chart-selected")))?;
                        (id, name)
                    };
                    debug!(room = room.id.to_string(), "local chart request start, host will serve");
                    // 通知房主启动下载服务器
                    user.try_send(ServerCommand::StartServing {
                        chart_id,
                        chart_name,
                    })
                    .await;
                    return Ok(());
                }
                if room.chart.read().await.is_none() {
                    bail!(tl!("start-no-chart-selected"));
                }
                debug!(room = room.id.to_string(), "room wait for ready");
                room.reset_game_time().await;
                room.send(Message::GameStart { user: user.id }).await;
                *room.state.write().await = InternalRoomState::WaitForReady {
                    started: std::iter::once(user.id).collect::<HashSet<_>>(),
                };
                room.on_state_change().await;
                
                // 启动异步任务让monitor用户和回放录制器自动准备
                let room_clone = room.clone();
                tokio::spawn(async move {
                    // 等待一小段时间确保消息已发送
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    
                    // 查找monitor用户（ID=2）并自动准备
                    if let Some(monitor_user) = room_clone.monitors().await.iter().find(|u| u.id == 2) {
                        // 检查房间状态是否为WaitForReady
                        let guard = room_clone.state.read().await;
                        if let InternalRoomState::WaitForReady { started } = &*guard {
                            if !started.contains(&2) {
                                drop(guard);
                                // 发送Ready消息
                                room_clone.send(Message::Ready { user: 2 }).await;
                                // 将monitor用户添加到started集合
                                let mut guard = room_clone.state.write().await;
                                if let InternalRoomState::WaitForReady { started } = &mut *guard {
                                    started.insert(2);
                                }
                                drop(guard);
                                info!(room = room_clone.id.to_string(), "monitor用户(ID=2)自动准备");
                                // 检查是否所有用户都准备好了
                                room_clone.check_all_ready().await;
                            }
                        }
                    }
                    
                    // 查找回放录制器（ID=-999）并自动准备
                    let recorder_id = crate::replay::RECORDER_BOT_USER_ID;
                    if let Some(recorder_user) = room_clone.monitors().await.iter().find(|u| u.id == recorder_id) {
                        // 检查房间状态是否为WaitForReady
                        let guard = room_clone.state.read().await;
                        if let InternalRoomState::WaitForReady { started } = &*guard {
                            if !started.contains(&recorder_id) {
                                drop(guard);
                                // 发送Ready消息
                                room_clone.send(Message::Ready { user: recorder_id }).await;
                                // 将录制器用户添加到started集合
                                let mut guard = room_clone.state.write().await;
                                if let InternalRoomState::WaitForReady { started } = &mut *guard {
                                    started.insert(recorder_id);
                                }
                                drop(guard);
                                info!(room = room_clone.id.to_string(), "回放录制器(ID={})自动准备", recorder_id);
                                // 检查是否所有用户都准备好了
                                room_clone.check_all_ready().await;
                            }
                        }
                    }
                });
                
                room.check_all_ready().await;
                Ok(())
            }
            .await;
            Some(ServerCommand::RequestStart(err_to_str(res)))
        }
        ClientCommand::Ready => {
            let res: Result<()> = async move {
                get_room!(room);
                let mut guard = room.state.write().await;
                if let InternalRoomState::WaitForReady { started } = guard.deref_mut() {
                    if !started.insert(user.id) {
                        bail!("already ready");
                    }
                    room.send(Message::Ready { user: user.id }).await;
                    drop(guard);
                    room.check_all_ready().await;
                }
                Ok(())
            }
            .await;
            Some(ServerCommand::Ready(err_to_str(res)))
        }
        ClientCommand::CancelReady => {
            let res: Result<()> = async move {
                get_room!(room);
                let mut guard = room.state.write().await;
                if let InternalRoomState::WaitForReady { started } = guard.deref_mut() {
                    if !started.remove(&user.id) {
                        bail!("not ready");
                    }
                    if room.check_host(&user).await.is_ok() {
                        room.send(Message::CancelGame { user: user.id }).await;
                        *guard = InternalRoomState::SelectChart;
                        drop(guard);
                        room.on_state_change().await;
                    } else {
                        room.send(Message::CancelReady { user: user.id }).await;
                    }
                }
                Ok(())
            }
            .await;
            Some(ServerCommand::CancelReady(err_to_str(res)))
        }
        ClientCommand::Played { id } => {
            let user_id = user.id;
            // 无官方 record id（-1）时没有可回源的成绩，直接返回成功，不请求 /record/-1。
            if id == -1 {
                return Some(ServerCommand::Played(Ok(())));
            }
            tokio::spawn(async move {
                let fetched: Result<Record> = async {
                    let res: Record = reqwest::get(format!("{HOST}/record/{id}"))
                        .await?
                        .error_for_status()?
                        .json()
                        .await?;
                    if res.player != user_id {
                        bail!("invalid record");
                    }
                    Ok(res)
                }
                .await;
                match fetched {
                    Ok(record) => {
                        let _ = handle_played_score(
                            user,
                            record.id,
                            record.score.max(0) as u32,
                            record.accuracy,
                            record.full_combo,
                            record.max_combo.max(0) as u32,
                            record.perfect.max(0) as u32,
                            record.good.max(0) as u32,
                            record.bad.max(0) as u32,
                            record.miss.max(0) as u32,
                        )
                        .await;
                    }
                    Err(err) => {
                        warn!(user = user_id, record = id, "failed to fetch official record: {err:?}");
                    }
                }
            });
            Some(ServerCommand::Played(Ok(())))
        }
        ClientCommand::PlayedWithScore { id, score, accuracy, full_combo, max_combo, perfect, good, bad, miss } => {
            let res = handle_played_score(
                user,
                id,
                score,
                accuracy,
                full_combo,
                max_combo,
                perfect,
                good,
                bad,
                miss,
            )
            .await;
            Some(ServerCommand::Played(err_to_str(res)))
        }
        ClientCommand::Abort => {
            let res: Result<()> = async move {
                get_room!(room);
                let mut guard = room.state.write().await;
                if let InternalRoomState::Playing { results, aborted } = guard.deref_mut() {
                    if results.contains_key(&user.id) {
                        bail!("already uploaded");
                    }
                    if !aborted.insert(user.id) {
                        bail!("aborted");
                    }
                    drop(guard);
                    room.send(Message::Abort { user: user.id }).await;
                    room.check_all_ready().await;
                }
                Ok(())
            }
            .await;
            Some(ServerCommand::Abort(err_to_str(res)))
        }
    }
}
