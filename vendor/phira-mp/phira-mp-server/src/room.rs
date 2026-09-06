use crate::{Chart, Record, User};
use anyhow::{bail, Result};
use phira_mp_common::{ClientRoomState, JudgeEvent, Judgement, Message, RoomId, RoomState, ServerCommand};
use rand::{seq::SliceRandom, thread_rng};
use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Weak,
    },
};
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tracing::{debug, info};
use serde::{Serialize, Deserialize};

// 玩家判定统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayerJudgementStats {
    pub user_id: i32,
    pub user_name: String,
    pub perfect: u32,
    pub good: u32,
    pub bad: u32,
    pub miss: u32,
    pub hold_perfect: u32,
    pub hold_good: u32,
    pub max_combo: u32,
    pub current_combo: u32,
    pub score: u32,
    pub accuracy: f32,
}

impl PlayerJudgementStats {
    pub fn new(user_id: i32, user_name: String) -> Self {
        Self {
            user_id,
            user_name,
            ..Default::default()
        }
    }
    
    pub fn add_judgement(&mut self, judgement: &Judgement) {
        match judgement {
            Judgement::Perfect => {
                self.perfect += 1;
                self.current_combo += 1;
                self.score += 100;
            }
            Judgement::Good => {
                self.good += 1;
                self.current_combo += 1;
                self.score += 50;
            }
            Judgement::Bad => {
                self.bad += 1;
                self.current_combo = 0;
                self.score += 10;
            }
            Judgement::Miss => {
                self.miss += 1;
                self.current_combo = 0;
            }
            Judgement::HoldPerfect => {
                self.hold_perfect += 1;
                self.score += 100;
            }
            Judgement::HoldGood => {
                self.hold_good += 1;
                self.score += 50;
            }
        }
        if self.current_combo > self.max_combo {
            self.max_combo = self.current_combo;
        }
        // 简化准确率计算
        let total = self.perfect + self.good + self.bad + self.miss;
        if total > 0 {
            self.accuracy = (self.perfect as f32 * 100.0 + self.good as f32 * 50.0 + self.bad as f32 * 10.0) 
                / (total as f32 * 100.0) * 100.0;
        }
    }
}

// 判定事件包装器，用于SSE传输（仿照PhiraRecord格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeEventWrapper {
    pub user_id: i32,
    pub user_name: String,
    pub time: f32,
    pub line_id: i32,
    pub note_id: i32,
    pub judgement: String,
}

impl JudgeEventWrapper {
    pub fn from_judge_event(user_id: i32, user_name: String, event: &JudgeEvent) -> Self {
        let judgement_str = match event.judgement {
            Judgement::Perfect => "Perfect",
            Judgement::Good => "Good",
            Judgement::Bad => "Bad",
            Judgement::Miss => "Miss",
            Judgement::HoldPerfect => "HoldPerfect",
            Judgement::HoldGood => "HoldGood",
        };
        Self {
            user_id,
            user_name,
            time: event.time,
            line_id: event.line_id,
            note_id: event.note_id,
            judgement: judgement_str.to_string(),
        }
    }
}

const ROOM_MAX_USERS: usize = 2147483647;

#[derive(Default, Debug)]
pub enum InternalRoomState {
    #[default]
    SelectChart,
    // 房主选择了本地谱面，处于本地分享阶段；started = 已同步完成的玩家 id 集合
    LocalChart {
        started: HashSet<i32>,
    },
    WaitForReady {
        started: HashSet<i32>,
    },
    Playing {
        results: HashMap<i32, Record>,
        aborted: HashSet<i32>,
    },
}

impl InternalRoomState {
    pub fn to_client(&self, chart: Option<i32>) -> RoomState {
        match self {
            Self::SelectChart => RoomState::SelectChart(chart),
            Self::LocalChart { .. } => RoomState::LocalChart,
            Self::WaitForReady { .. } => RoomState::WaitingForReady,
            Self::Playing { .. } => RoomState::Playing,
        }
    }
}

pub struct Room {
        pub id: RoomId,
        pub host: RwLock<Weak<User>>,
        pub state: RwLock<InternalRoomState>,
    pub log_tx: broadcast::Sender<String>,
    pub judgement_tx: broadcast::Sender<String>, // 判定统计广播
    
        pub live: AtomicBool,
        pub locked: AtomicBool,
        pub cycle: AtomicBool,
    
        users: RwLock<Vec<Weak<User>>>,
        monitors: RwLock<Vec<Weak<User>>>,
        pub chart: RwLock<Option<Chart>>,
        // 正在分享的本地谱面 (UUID id, 名称)
        pub local_chart: RwLock<Option<(String, String)>>,
        // 房主是否已上传本地谱面包到服务端缓存（玩家须等上传完成后才能就绪/下载）
        pub chart_uploaded: AtomicBool,
        pub is_competition: AtomicBool, // 添加比赛房间标识
        pub judgement_stats: RwLock<HashMap<i32, PlayerJudgementStats>>, // 玩家判定统计
        pub judge_events: RwLock<Vec<JudgeEventWrapper>>, // 原始判定事件列表（仿照PhiraRecord）
        pub creator_id: RwLock<Option<i32>>, // 创建者ID
    }
impl Room {
    async fn get_user_name_by_id(&self, uid: i32) -> Option<String> {
        if uid == 0 {
            return Some("系统".to_string());
        }
        for u in self.users().await {
            if u.id == uid {
                return Some(u.name.clone());
            }
        }
        for u in self.monitors().await {
            if u.id == uid {
                return Some(u.name.clone());
            }
        }
        None
    }

    async fn format_message_for_log(&self, msg: &Message) -> String {
        use phira_mp_common::Message::*;
        match msg {
            Chat { user, content } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("聊天：{} — {}", name, content)
            }
            CreateRoom { user, name } => {
                format!("创建房间：{} ({})", name, user)
            }
            JoinRoom { user, name, .. } => format!("加入房间：{} ({})", name, user),
            LeaveRoom { user, name } => format!("离开房间：{} ({})", name, user),
            NewHost { user } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("新房主：{}", name)
            }
            SelectChart { user, name, id } => {
                let uname = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("选择谱面：{} 由 {} (id={})", name, uname, id)
            }
            GameStart { user } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("请求开始：{} 发起", name)
            }
            Ready { user } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("准备：{}", name)
            }
            CancelReady { user } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("取消准备：{}", name)
            }
            CancelGame { user } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("取消比赛：{}", name)
            }
            StartPlaying => "开始游戏".to_string(),
            Played { user, score, accuracy, full_combo } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("完成：{} 分数={} 准确率={:.2}% 全连={} ", name, score, accuracy * 100.0, full_combo)
            }
            GameEnd => "游戏结束".to_string(),
            Abort { user } => {
                let name = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("中止：{}", name)
            }
            LockRoom { lock } => format!("房间锁定：{}", lock),
            CycleRoom { cycle } => format!("循环模式：{}", cycle),
            SelectLocalChart { user, name, id } => {
                let uname = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("选择本地谱面：{} 由 {} (id={})", name, uname, id)
            }
            SendChart { user } => {
                let uname = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("开始分享谱面：{}", uname)
            }
            DownloadReady { user } => {
                let uname = self.get_user_name_by_id(*user).await.unwrap_or_else(|| format!("用户#{}", user));
                format!("谱面下载完成：{}", uname)
            }
        }
    }
    pub fn new(id: RoomId, host: Weak<User>, creator_id: Option<i32>) -> Self {
        Self {
            id,
            host: host.clone().into(),
            state: RwLock::default(),

            live: AtomicBool::new(true), // 默认启用live模式，用于回放录制
            locked: AtomicBool::new(false),
            cycle: AtomicBool::new(false),

            users: vec![host].into(),
            monitors: Vec::new().into(),
            chart: RwLock::default(),
            local_chart: RwLock::default(),
            chart_uploaded: AtomicBool::new(false),
            is_competition: AtomicBool::new(false), // 默认不是比赛房间
            log_tx: broadcast::channel(256).0,
            judgement_tx: broadcast::channel(4096).0,
            judgement_stats: RwLock::default(),
            judge_events: RwLock::default(),
            creator_id: RwLock::new(creator_id),
        }
    }
    
    // 订阅判定统计更新
    pub fn subscribe_judgements(&self) -> broadcast::Receiver<String> {
        self.judgement_tx.subscribe()
    }
    
    // 更新玩家判定统计（添加原始判定事件）
    pub async fn update_judgement_stats(&self, user_id: i32, user_name: String, event: &JudgeEvent) {
        // 更新统计
        let mut stats = self.judgement_stats.write().await;
        let player_stats = stats.entry(user_id).or_insert_with(|| PlayerJudgementStats::new(user_id, user_name.clone()));
        player_stats.add_judgement(&event.judgement);
        
        // 存储原始判定事件（仿照PhiraRecord格式）
        let wrapper = JudgeEventWrapper::from_judge_event(user_id, user_name, event);
        let mut events = self.judge_events.write().await;
        events.push(wrapper.clone());
        
        // 广播更新后的统计和最新事件
        let stats_vec: Vec<&PlayerJudgementStats> = stats.values().collect();
        let stats_json = serde_json::to_string(&stats_vec).unwrap_or_default();
        let _ = self.judgement_tx.send(stats_json);
    }
    
    // 批量更新玩家判定统计（只广播一次）
    pub async fn update_judgement_stats_batch(&self, user_id: i32, user_name: String, events: &[JudgeEvent]) {
        if events.is_empty() {
            return;
        }
        
        // 更新统计
        let mut stats = self.judgement_stats.write().await;
        let player_stats = stats.entry(user_id).or_insert_with(|| PlayerJudgementStats::new(user_id, user_name.clone()));
        
        // 存储原始判定事件
        let mut judge_events = self.judge_events.write().await;
        
        for event in events {
            player_stats.add_judgement(&event.judgement);
            let wrapper = JudgeEventWrapper::from_judge_event(user_id, user_name.clone(), event);
            judge_events.push(wrapper);
        }
        
        // 只广播一次更新后的统计
        let stats_vec: Vec<&PlayerJudgementStats> = stats.values().collect();
        let stats_json = serde_json::to_string(&stats_vec).unwrap_or_default();
        let _ = self.judgement_tx.send(stats_json);
        
        tracing::debug!("批量更新判定统计: 用户ID={}, 事件数={}", user_id, events.len());
    }
    
    // 重置判定统计
    pub async fn reset_judgement_stats(&self) {
        let mut stats = self.judgement_stats.write().await;
        stats.clear();
        let mut events = self.judge_events.write().await;
        events.clear();
        let _ = self.judgement_tx.send("[]".to_string());
    }

    // 玩家游玩结束后，用客户端上报的真实成绩回填判定统计，
    // 使管理端 Judge 显示真实分数/准确率/判定数/最大连击。
    pub async fn update_final_stats(&self, user_id: i32, user_name: String, record: &Record) {
        let mut stats = self.judgement_stats.write().await;
        let s = stats
            .entry(user_id)
            .or_insert_with(|| PlayerJudgementStats::new(user_id, user_name));
        s.perfect = record.perfect.max(0) as u32;
        s.good = record.good.max(0) as u32;
        s.bad = record.bad.max(0) as u32;
        s.miss = record.miss.max(0) as u32;
        s.max_combo = record.max_combo.max(0) as u32;
        s.current_combo = record.max_combo.max(0) as u32;
        s.score = record.score.max(0) as u32;
        s.accuracy = record.accuracy;
        let stats_vec: Vec<&PlayerJudgementStats> = stats.values().collect();
        let stats_json = serde_json::to_string(&stats_vec).unwrap_or_default();
        let _ = self.judgement_tx.send(stats_json);
    }

    pub fn is_live(&self) -> bool {
        self.live.load(Ordering::SeqCst)
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }

    pub fn is_cycle(&self) -> bool {
        self.cycle.load(Ordering::SeqCst)
    }

    pub async fn client_room_state(&self) -> RoomState {
        self.state
            .read()
            .await
            .to_client(self.chart.read().await.as_ref().map(|it| it.id))
    }

    pub async fn client_state(&self, user: &User) -> ClientRoomState {
        ClientRoomState {
            id: self.id.clone(),
            state: self.client_room_state().await,
            live: self.is_live(),
            locked: self.is_locked(),
            cycle: self.is_cycle(),
            is_host: self.check_host(user).await.is_ok(),
            is_ready: matches!(&*self.state.read().await, InternalRoomState::WaitForReady { started } if started.contains(&user.id)),
            users: self
                .users
                .read()
                .await
                .iter()
                .chain(self.monitors.read().await.iter())
                .filter_map(|it| it.upgrade().map(|it| (it.id, it.to_info())))
                .collect(),
        }
    }

    pub async fn on_state_change(&self) {
        self.broadcast(ServerCommand::ChangeState(self.client_room_state().await))
            .await;
    }

    pub async fn add_user(&self, user: Weak<User>, monitor: bool) -> bool {
        if monitor {
            let mut guard = self.monitors.write().await;
            guard.retain(|it| it.strong_count() > 0);
            guard.push(user);
            true
        } else {
            let mut guard = self.users.write().await;
            guard.retain(|it| it.strong_count() > 0);
            if guard.len() >= ROOM_MAX_USERS {
                false
            } else {
                guard.push(user);
                true
            }
        }
    }

    pub async fn users(&self) -> Vec<Arc<User>> {
        self.users
            .read()
            .await
            .iter()
            .filter_map(|it| it.upgrade())
            .collect()
    }

    pub async fn monitors(&self) -> Vec<Arc<User>> {
        self.monitors
            .read()
            .await
            .iter()
            .filter_map(|it| it.upgrade())
            .collect()
    }

    pub async fn check_host(&self, user: &User) -> Result<()> {
        if self.host.read().await.upgrade().map(|it| it.id) != Some(user.id) {
            bail!("only host can do this");
        }
        Ok(())
    }

    #[inline]
    pub async fn send(&self, msg: Message) {
        // 首先把消息格式化为管理界面可读的中文日志并发送到房间日志广播
        let log = self.format_message_for_log(&msg).await;
        let _ = self.log_tx.send(log);

        self.broadcast(ServerCommand::Message(msg)).await;
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<String> {
        self.log_tx.subscribe()
    }

    pub async fn broadcast(&self, cmd: ServerCommand) {
        debug!("broadcast {cmd:?}");
        for session in self
            .users()
            .await
            .into_iter()
            .chain(self.monitors().await.into_iter())
        {
            session.try_send(cmd.clone()).await;
        }
    }

    pub async fn broadcast_monitors(&self, cmd: ServerCommand) {
        for session in self.monitors().await {
            session.try_send(cmd.clone()).await;
        }
    }

    #[inline]
    pub async fn send_as(&self, user: &User, content: String) {
        self.send(Message::Chat {
            user: user.id,
            content,
        })
        .await;
    }

    /// Return: should the room be dropped
    #[must_use]
    pub async fn on_user_leave(&self, user: &User) -> bool {
        self.send(Message::LeaveRoom {
            user: user.id,
            name: user.name.clone(),
        })
        .await;
        *user.room.write().await = None;
        (if user.monitor.load(Ordering::SeqCst) {
            &self.monitors
        } else {
            &self.users
        })
        .write()
        .await
        .retain(|it| it.upgrade().map_or(false, |it| it.id != user.id));
                if self.check_host(user).await.is_ok() {
                    info!("host disconnected!");
                    // 若游戏进行中房主退出，直接结束本局，回到选谱界面，
                    // 避免因其他成员未完成而卡在 Playing 状态、无法进入下一轮。
                    if matches!(*self.state.read().await, InternalRoomState::Playing { .. }) {
                        info!(room = self.id.to_string(), "host left during playing, ending game");
                        *self.local_chart.write().await = None;
                        *self.state.write().await = InternalRoomState::SelectChart;
                        self.on_state_change().await;
                    }
                    // 对于比赛房间，保持ID为0的系统作为房主，不选择新用户
                    if self.is_competition.load(std::sync::atomic::Ordering::SeqCst) {
                        // 发送NewHost消息，将房主设为ID为0的系统账号
                        self.send(Message::NewHost { user: 0 }).await;
                    } else {
                        let users = self.users().await;
                        if users.is_empty() {
                            info!("room users all disconnected, dropping room");
                            return true;
                        } else {
                            let user = users.choose(&mut thread_rng()).unwrap();
                            debug!("selected {} as host", user.id);
                            *self.host.write().await = Arc::downgrade(user);
                            self.send(Message::NewHost { user: user.id }).await;
                            user.try_send(ServerCommand::ChangeHost(true)).await;
                        }
                    }
                }
        self.check_all_ready().await;
        false
    }

    pub async fn reset_game_time(&self) {
        for user in self.users().await {
            user.game_time
                .store(f32::NEG_INFINITY.to_bits(), Ordering::SeqCst);
        }
    }

    pub async fn check_all_ready(&self) {
        let guard = self.state.read().await;
        match guard.deref() {
            InternalRoomState::LocalChart { started } => {
                // 本地谱面分享阶段：所有需要下载的玩家（非房主、非回放录制器、非 monitor 观察者）
                // 就绪后，通知房主并开始游戏。若房间只有房主一人，无需下载，直接视为就绪。
                // 前提：房主必须先上传谱面包到服务端缓存（chart_uploaded）。
                let users = self.users().await;
                let recorder_id = crate::replay::RECORDER_BOT_USER_ID;
                let all_downloaded = self.chart_uploaded.load(std::sync::atomic::Ordering::SeqCst)
                    && match self.host.read().await.upgrade() {
                        Some(host) => {
                            let hid = host.id;
                            users
                                .iter()
                                .filter(|it| {
                                    it.id != hid
                                        && it.id != recorder_id
                                        && !it.monitor.load(std::sync::atomic::Ordering::SeqCst)
                                })
                                .all(|it| started.contains(&it.id))
                        }
                        None => false,
                    };
                if all_downloaded {
                    drop(guard);
                    info!(room = self.id.to_string(), "local chart all downloaded, game start");
                    // 通知房主所有玩家已下载完成，可以关闭下载连接
                    if let Some(host) = self.host.read().await.upgrade() {
                        // 游戏即将开始：删除服务端缓存的谱面包，避免占用磁盘空间
                        if let Some((chart_id, _)) = self.local_chart.read().await.clone() {
                            host.server
                                .chart_cache
                                .write()
                                .await
                                .remove(&chart_id);
                            info!(room = self.id.to_string(), chart = %chart_id, "cleared chart cache after game start");
                        }
                        host.try_send(ServerCommand::HostReady).await;
                    }
                    self.chart_uploaded.store(false, std::sync::atomic::Ordering::SeqCst);
                    // 重置判定统计
                    self.reset_judgement_stats().await;
                    self.send(Message::StartPlaying).await;
                    self.reset_game_time().await;
                    *self.state.write().await = InternalRoomState::Playing {
                        results: HashMap::new(),
                        aborted: HashSet::new(),
                    };
                    self.on_state_change().await;
                }
            }
            InternalRoomState::WaitForReady { started } => {
                if self
                    .users()
                    .await
                    .into_iter()
                    .chain(self.monitors().await.into_iter())
                    .all(|it| started.contains(&it.id))
                {
                    drop(guard);
                    info!(room = self.id.to_string(), "game start");
                    // 重置判定统计
                    self.reset_judgement_stats().await;
                    self.send(Message::StartPlaying).await;
                    self.reset_game_time().await;
                    *self.state.write().await = InternalRoomState::Playing {
                        results: HashMap::new(),
                        aborted: HashSet::new(),
                    };
                    self.on_state_change().await;
                }
            }
            InternalRoomState::Playing { results, aborted } => {
                if self
                    .users()
                    .await
                    .into_iter()
                    .all(|it| results.contains_key(&it.id) || aborted.contains(&it.id))
                {
                    drop(guard);
                    // TODO print results
                    self.send(Message::GameEnd).await;
                    // dbg!(2);
                    *self.state.write().await = InternalRoomState::SelectChart;
                    // dbg!(3);
                    if self.is_cycle() {
                        debug!(room = self.id.to_string(), "cycling");
                        // 对于比赛房间，保持ID为0的系统作为房主
                        if self.is_competition.load(std::sync::atomic::Ordering::SeqCst) {
                            // 发送NewHost消息，将房主设为ID为0的系统账号
                            self.send(Message::NewHost { user: 0 }).await;
                        } else {
                            let host = Weak::clone(&*self.host.read().await);
                            let new_host = {
                                let users = self.users().await;
                                let index = users
                                    .iter()
                                    .position(|it| host.ptr_eq(&Arc::downgrade(it)))
                                    .map(|it| (it + 1) % users.len())
                                    .unwrap_or_default();
                                users.into_iter().nth(index).unwrap()
                            };
                            *self.host.write().await = Arc::downgrade(&new_host);
                            self.send(Message::NewHost { user: new_host.id }).await;
                            if let Some(old) = host.upgrade() {
                                old.try_send(ServerCommand::ChangeHost(false)).await;
                            }
                            new_host.try_send(ServerCommand::ChangeHost(true)).await;
                        }
                    }
                    // 清空本地谱面残留状态并通知所有客户端退出本地谱面分享，
                    // 避免本地谱面游玩结束后切换到在线谱面时状态错乱（卡在转圈/无法开始）。
                    *self.local_chart.write().await = None;
                    self.chart_uploaded.store(false, Ordering::SeqCst);
                    self.broadcast(ServerCommand::ChangeLocalChart { local: false, chart_id: String::new() })
                        .await;
                    self.on_state_change().await;
                }
            }
            _ => {}
        }
    }
}
