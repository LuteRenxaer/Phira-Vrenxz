//! 观战会话：旁观（monitor）状态 + 同步观战的数据喂送。
//!
//! # 两种观战
//! - **旁观**：以 `client.join_room(id, true)`（monitor）进入任意房间，含正在对局
//!   中的房间。只读、不占玩家位、不参与就绪与结算。服务端会在观战者进房时补发
//!   当前谱面与已暂停玩家的暂停状态（这些消息由 [`super::messages`] 提取后经
//!   [`Spectate::notice`] 转交进来）。
//! - **同步观战**：加载对方正在玩的谱面，把对方的判定事件流喂给引擎
//!   `prpr::scene::SpectateSource`，用 `SongScene::global_launch_spectate` 启动。
//!   后台任务持续搬运 `client.live_player(target).judge_events / touch_frames`
//!   并 `set_time_ref`，同时每轮 `set_remote_paused(client.blocking_player_paused(target))`。
//!
//! 观战的信息展示（谱面名、每人 P/G/B/M/连击/近似准确率与分数、选择目标）是
//! 多人场景里的一个**整屏页面**（见 [`super::page::spectate`]），本模块只提供数据。
//!
//! # 观战暂停画面
//! 暂停画面本身（变暗/模糊、暂停按钮、退出/重试/继续三键、屏幕正中的
//! 「玩家暂停中」）由引擎绘制，状态机与渲染函数都在引擎里：
//! - 状态机：[`prpr::scene::SpectatePauseUi`]
//!   （`Hidden` / `RemoteWaiting` / `RemoteControls` / `LocalPaused`）；
//! - 唯一渲染入口：`prpr::scene::game::GameScene` 的观战暂停提示函数
//!   （只画观战专属的居中文字，其余暂停画面由常规暂停界面负责）；
//! - 文案：`prpr/locales/<lang>/game.ftl` 的 `spectate-remote-paused`。
//!
//! 观战端只通过 [`prpr::scene::SpectateSource`] 与引擎交互，因此这里不需要
//! 镜像任何暂停状态；各状态的语义：
//! - `Hidden`：不在观战暂停画面（正常播放，或观战者自己的暂停已展开控制按钮）
//! - `RemoteWaiting`：被观战者暂停 → 只显示暂停按钮 + 正中「玩家暂停中」
//! - `RemoteControls`：被观战者暂停且观战者双击展开了三个控制按钮 → 隐藏文字
//! - `LocalPaused`：观战者自己暂停本机画面 → 显示控制按钮、无正中文字，
//!   且**绝不**通知服务器（观战场景不取用 `PAUSE_NOTIFY`）
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Result;
use phira_mp_client::Client;
use phira_mp_common::{ClientRoomState, Judgement, RoomState};
use prpr::{
    ext::LocalTask,
    scene::{NextScene, SpectateSource},
    task::Task,
};

use super::messages::RoomNotice;
use crate::scene::SongScene;

/// 同步观战的喂数据间隔。
const FEED_INTERVAL: Duration = Duration::from_millis(40);

/// 观战中的单个玩家实时统计（依据服务端广播的 Judges 事件累计，仅用于观战展示）。
#[derive(Clone, Copy, Default)]
pub struct SpectateStat {
    pub perfect: u32,
    pub good: u32,
    pub bad: u32,
    pub miss: u32,
    pub combo: u32,
    pub max_combo: u32,
}

impl SpectateStat {
    fn apply(&mut self, judgement: Judgement) {
        use Judgement as J;
        match judgement {
            J::Perfect | J::HoldPerfect => {
                self.perfect += 1;
                self.combo += 1;
                self.max_combo = self.max_combo.max(self.combo);
            }
            J::Good | J::HoldGood => {
                self.good += 1;
                self.combo += 1;
                self.max_combo = self.max_combo.max(self.combo);
            }
            J::Bad => {
                self.bad += 1;
                self.combo = 0;
            }
            J::Miss => {
                self.miss += 1;
                self.combo = 0;
            }
        }
    }

    pub fn total(&self) -> u32 {
        self.perfect + self.good + self.bad + self.miss
    }

    /// 观战用近似准确率（与服务端结算一致的 P/G 权重口径）。
    pub fn accuracy(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            return 0.;
        }
        (self.perfect as f32 + self.good as f32 * 0.65) / total as f32
    }

    /// 观战用近似分数（与服务端结算同口径的粗算）。
    pub fn score(&self) -> u32 {
        (self.accuracy() as f64 * 900000. + self.max_combo as f64 * 100.) as u32
    }
}

pub struct Spectate {
    /// 是否以观战者身份（monitor）加入了房间
    joined: bool,
    /// 观战中各玩家的实时统计
    stats: HashMap<i32, SpectateStat>,
    /// 正在游玩的谱面名（服务端补发的选谱消息）
    chart_name: Option<String>,
    /// 同步观战的目标玩家
    target: Option<i32>,
    /// 当前观战场景的数据源（场景结束时读它的退出请求）
    source: Option<Arc<SpectateSource>>,
    /// 喂数据任务的停止信号（Some 表示同步观战场景正在运行）
    feed_stop: Option<Arc<AtomicBool>>,
}

impl Default for Spectate {
    fn default() -> Self {
        Self::new()
    }
}

impl Spectate {
    pub fn new() -> Self {
        Self {
            joined: false,
            stats: HashMap::new(),
            chart_name: None,
            target: None,
            source: None,
            feed_stop: None,
        }
    }

    #[inline]
    pub fn joined(&self) -> bool {
        self.joined
    }

    /// 视图状态：是否以观战者身份在房间里（决定房间操作条只给观战入口）。
    #[inline]
    pub fn is_spectating(&self) -> bool {
        self.joined
    }

    #[inline]
    pub fn target(&self) -> Option<i32> {
        self.target
    }

    #[inline]
    pub fn stats(&self) -> &HashMap<i32, SpectateStat> {
        &self.stats
    }

    #[inline]
    pub fn chart_name(&self) -> Option<&str> {
        self.chart_name.as_deref()
    }

    /// 以观战者身份进房：清空上一轮统计，准备围观。
    pub fn begin(&mut self) {
        self.joined = true;
        self.stats.clear();
        self.chart_name = None;
        self.target = None;
    }

    /// 以玩家身份进房（或建房 / 深链接进房）：清空观战意图。
    pub fn cancel(&mut self) {
        self.joined = false;
        self.stats.clear();
        self.chart_name = None;
        self.target = None;
    }

    /// 消费消息流里提取出的选谱信息（观战者进房时服务端补发）。
    pub fn notice(&mut self, notice: RoomNotice) {
        if !self.joined {
            return;
        }
        match notice {
            RoomNotice::OnlineChart { name, .. } => self.chart_name = Some(name),
            RoomNotice::LocalChart { name } => self.chart_name = Some(name),
        }
    }

    /// 累计服务端 live 广播的判定事件，供观战页展示实时进度。
    pub fn collect_stats(&mut self, client: &Client) {
        let Some(room) = client.blocking_state() else {
            self.stats.clear();
            return;
        };
        // 只统计真正的玩家（monitor 观战者没有判定事件）
        for (id, user) in room.users.iter() {
            if user.monitor {
                continue;
            }
            let live = client.live_player(*id);
            let events: Vec<_> = live.judge_events.blocking_lock().drain(..).collect();
            if events.is_empty() {
                continue;
            }
            let stat = self.stats.entry(*id).or_default();
            for ev in events {
                stat.apply(ev.judgement);
            }
        }
        // 房间已回到选谱阶段：清空上一局统计，准备下一局观战
        if matches!(room.state, RoomState::SelectChart(_)) {
            self.stats.clear();
        }
    }

    /// 每帧：已在房间外则自动结束观战状态（离开 / 被移出 / 房间解散）。
    pub fn poll_room(&mut self, client: Option<&Client>) {
        if !self.joined {
            return;
        }
        if client.and_then(|c| c.blocking_room_id()).is_none() {
            self.cancel();
        }
    }

    // ---------- 同步观战 ----------

    /// 选定同步观战目标：优先已选定的目标，否则取第一个正在游玩的玩家。
    pub fn pick_target(&self, room: &ClientRoomState) -> Option<i32> {
        self.target.filter(|id| room.users.contains_key(id)).or_else(|| {
            let mut ids: Vec<i32> = room
                .users
                .iter()
                .filter(|(_, u)| !u.monitor)
                .map(|(id, _)| *id)
                .collect();
            ids.sort_unstable();
            ids.first().copied()
        })
    }

    #[inline]
    pub fn set_target(&mut self, id: i32) {
        self.target = Some(id);
    }

    /// 启动同步观战场景，并开一个后台任务把对方的事件流喂给引擎。
    pub fn begin_watch(&mut self, client: Option<Arc<Client>>, id: Option<i32>, path: &str, target: i32) -> Result<LocalTask<Result<NextScene>>> {
        let source = SpectateSource::new();
        let stop = Arc::new(AtomicBool::new(false));
        // 场景结束时会话需要读它的“退出观战”请求，因此在这里留一份
        self.source = Some(Arc::clone(&source));
        let task = match SongScene::global_launch_spectate(id, path, Arc::clone(&source)) {
            Ok(task) => task,
            Err(err) => {
                // 场景启动失败：清掉数据源，避免后续误判“观战画面还在跑”
                self.source = None;
                return Err(err);
            }
        };
        if let Some(client) = client {
            let feed_source = Arc::clone(&source);
            let feed_stop = Arc::clone(&stop);
            Task::new(async move { Self::feed_loop(client, target, feed_source, feed_stop).await });
        }
        self.feed_stop = Some(stop);
        self.set_target(target);
        Ok(task)
    }

    /// 观战场景结束回到多人场景：停止喂数据任务。
    pub fn on_scene_return(&mut self) {
        if let Some(stop) = self.feed_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// 观战者在暂停面板里点了「退出」→ 一次性消费退出请求。
    pub fn take_quit_request(&self) -> bool {
        self.source.as_ref().is_some_and(|it| it.take_quit_request())
    }

    /// 真正退出观战：离开房间并清理观战状态。
    pub fn finish(&mut self) {
        self.on_scene_return();
        self.source = None;
        self.joined = false;
        self.stats.clear();
        self.chart_name = None;
        self.target = None;
    }

    /// 持续把目标玩家的判定事件与时间参考喂给观战场景，
    /// 使观战画面的音符命中、分数、连击与音乐进度都与对方一致。
    async fn feed_loop(client: Arc<Client>, target: i32, source: Arc<SpectateSource>, stop: Arc<AtomicBool>) {
        use phira_mp_common::Judgement as MJ;
        use prpr::judge::{SpectateEvent, SpectateJudgement as SJ};
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(FEED_INTERVAL).await;
            let live = client.live_player(target);
            // 判定事件：驱动对方的命中/失误与分数
            let events: Vec<phira_mp_common::JudgeEvent> = live.judge_events.lock().await.drain(..).collect();
            if !events.is_empty() {
                let converted: Vec<SpectateEvent> = events
                    .iter()
                    .map(|it| SpectateEvent {
                        time: it.time as f64,
                        line_id: it.line_id,
                        note_id: it.note_id,
                        judgement: match it.judgement {
                            MJ::Perfect => SJ::Perfect,
                            MJ::Good => SJ::Good,
                            MJ::Bad => SJ::Bad,
                            MJ::Miss => SJ::Miss,
                            MJ::HoldPerfect => SJ::HoldPerfect,
                            MJ::HoldGood => SJ::HoldGood,
                        },
                    })
                    .collect();
                source.push_events(converted);
            }
            // 触控帧：用作时间参考（对方当前谱面时间），保证画面进度同步
            let frames: Vec<phira_mp_common::TouchFrame> = live.touch_frames.lock().await.drain(..).collect();
            if let Some(last) = frames.last() {
                source.set_time_ref(last.time as f64);
            }
            // 被观战者的暂停状态：远端暂停时观战端进入暂停画面（对方继续后自动恢复）
            source.set_remote_paused(client.blocking_player_paused(target));
        }
    }
}
