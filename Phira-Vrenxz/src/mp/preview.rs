//! 谱面预览会话：autoplay 试听当前谱面。
//!
//! 预览只在“回到多人场景”时才有意义，因此这里只保存三件事：
//! - **打断信号**：后台轮询房间状态，一旦房主开始（离开选谱阶段）就置位，
//!   `GameScene` 检测到后立即结束预览（不计成绩）；
//! - **停止信号**：预览场景结束回到多人场景后置位，结束后台轮询；
//! - **待确认标记**：回到房间页时若房主已在等准备而自己尚未就绪，则置位，
//!   由房间页渲染一条**内联提示条**（「准备 / 暂不」两个按钮）——
//!   不再使用引擎的居中 `Dialog`，多人模式内部没有任何浮窗。

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Result;
use phira_mp_client::Client;
use phira_mp_common::RoomState;
use prpr::{
    config::Mods,
    ext::LocalTask,
    scene::{GameMode, NextScene},
    task::Task,
};

use crate::scene::SongScene;

/// 预览场景结束回到多人场景后的处理结果。
pub enum PreviewReturn {
    /// 不是从预览返回，或无需处理
    Ignore,
    /// 房主已经开局：只给一条轻提示，不打断对局流程
    AlreadyStarted,
    /// 从预览回来了：房间页按实时状态决定是否显示内联确认条
    AskReady,
}

#[derive(Default)]
pub struct Preview {
    /// 后台轮询任务的停止信号（Some 表示预览场景正在运行）
    stop: Option<Arc<AtomicBool>>,
    /// 打断信号：房间离开选谱阶段（房主开始）时置位
    interrupt: Option<Arc<AtomicBool>>,
    /// 从预览回来后置位：房间页据此按"实时状态"决定要不要显示确认条
    ///
    /// 关键是这里存的是**意图**而不是"要不要显示"的快照：房主按下开始与预览结束
    /// 之间差几十毫秒都很正常，返回那一刻房间还停在选谱阶段的话，用快照判断就会漏掉
    /// 提示（这正是"房主开始后没有提示"的原因）。
    armed: bool,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            stop: None,
            interrupt: None,
            armed: false,
        }
    }

    /// 启动 autoplay 预览场景，并开一个后台任务监视房主是否开始。
    pub fn begin(&mut self, client: Option<Arc<Client>>, id: Option<i32>, path: &str) -> Result<LocalTask<Result<NextScene>>> {
        let interrupt = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let task = SongScene::global_launch_preview(
            id,
            path,
            Mods::AUTOPLAY,
            GameMode::NoRetry,
            None,
            None,
            None,
            false,
            false,
            true,
            Some(Arc::clone(&interrupt)),
        )?;
        if let Some(client) = client {
            let (interrupt, stop) = (Arc::clone(&interrupt), Arc::clone(&stop));
            // 预览期间会话自身不更新，只能靠独立任务轮询房间状态
            Task::new(async move { Self::watch_loop(client, interrupt, stop).await });
        }
        self.interrupt = Some(interrupt);
        self.stop = Some(stop);
        Ok(task)
    }

    /// 场景启动失败时清理运行态，避免残留任务/误判“从预览返回”。
    pub fn abort(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.interrupt = None;
    }

    /// 预览场景结束回到多人场景：清理运行态并给出后续动作。
    pub fn on_return(&mut self, room: Option<&RoomState>, is_ready: bool) -> PreviewReturn {
        // 判据是“是否从预览画面返回”，而不依赖打断标志是否已置位——
        // 房主开始往往就发生在预览结束的同一瞬间，只看标志会漏掉提示。
        if self.stop.take().is_none() {
            return PreviewReturn::Ignore;
        }
        self.interrupt = None;
        match room {
            // 房主已经开局：只提示，不需要再问准备
            Some(RoomState::Playing) => PreviewReturn::AlreadyStarted,
            // 房主在等准备、而自己已经准备过了：也不需要再问
            Some(RoomState::WaitingForReady) if is_ready => PreviewReturn::Ignore,
            // 其余情况一律先"武装"提示：房间页会在房主进入等待准备、
            // 而自己还没准备时把它显示出来（哪怕返回时还停在选谱阶段）
            _ => PreviewReturn::AskReady,
        }
    }

    /// 请求显示内联的「房主要开始游戏啦」确认条。
    #[inline]
    pub fn ask_ready(&mut self) {
        self.armed = true;
    }

    /// 当前是否该显示确认条：从预览回来后，房间正等着自己准备。
    ///
    /// 由房间页每帧按实时状态询问，因此房主开始得比预览结束晚一点也不会漏提示。
    #[inline]
    pub fn prompt(&self, room: Option<&RoomState>, is_ready: bool) -> bool {
        self.armed && !is_ready && matches!(room, Some(RoomState::WaitingForReady))
    }

    #[inline]
    pub fn dismiss_prompt(&mut self) {
        self.armed = false;
    }

    /// 房间状态轮询：开始时若还在选谱/本地谱阶段，则一旦离开该阶段即视为“房主开始”；
    /// 若是在等待准备阶段开始预览，则只有真正开局（Playing）才打断。
    async fn watch_loop(client: Arc<Client>, interrupt: Arc<AtomicBool>, stop: Arc<AtomicBool>) {
        // 注意：这里必须用异步版 room_state()。blocking_room_state() 走的是 tokio 的
        // RwLock::blocking_read，在 async 上下文里会 panic —— 那样这个打断任务会
        // 一启动就静默死掉，房主点开始也打断不了预览。
        let initial = client.room_state().await;
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Some(state) = client.room_state().await else {
                // 已离开房间/被移出：不再需要打断
                break;
            };
            let started = match initial {
                Some(RoomState::WaitingForReady) => matches!(state, RoomState::Playing),
                _ => !matches!(state, RoomState::SelectChart(_) | RoomState::LocalChart),
            };
            if started {
                interrupt.store(true, Ordering::Relaxed);
                break;
            }
        }
    }
}
