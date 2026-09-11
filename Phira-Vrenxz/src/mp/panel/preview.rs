//! 谱面预览会话：autoplay 试听当前谱面。
//!
//! 预览只在“回到面板”时才有意义，因此这里只保存两件事：
//! - **打断信号**：后台轮询房间状态，一旦房主开始（离开选谱阶段）就置位，
//!   `GameScene` 检测到后立即结束预览（不结算）；
//! - **停止信号**：预览场景结束回到面板后置位，结束后台轮询。
//!
//! 回到面板后按房间状态决定是否弹「房主要开始游戏啦」确认框；
//! 点「准备」只是置位一个标志，真正的下载→就绪流程由面板执行，
//! 与在房间里直接点「准备」走的是同一条路径。

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
    ui::Dialog,
};

use crate::{
    mp::L10N_LOCAL,
    scene::SongScene,
};

/// 预览场景结束回到面板后的处理结果。
pub enum PreviewReturn {
    /// 不是从预览返回，或无需处理
    Ignore,
    /// 房主已经开局：只给一条轻提示，不打断对局流程
    AlreadyStarted,
    /// 房主在等准备、而自己尚未就绪：弹确认框询问是否准备
    AskReady,
}

#[derive(Default)]
pub struct Preview {
    /// 后台轮询任务的停止信号（Some 表示预览场景正在运行）
    stop: Option<Arc<AtomicBool>>,
    /// 打断信号：房间离开选谱阶段（房主开始）时置位
    interrupt: Option<Arc<AtomicBool>>,
    /// 确认框「准备」按钮的置位（面板在 update 里消费）
    ready_confirm: Arc<AtomicBool>,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            stop: None,
            interrupt: None,
            ready_confirm: Arc::new(AtomicBool::new(false)),
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
            // 预览期间面板自身不更新，只能靠独立任务轮询房间状态
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

    /// 预览场景结束回到面板：清理运行态并给出后续动作。
    pub fn on_return(&mut self, room: Option<&RoomState>, is_ready: bool, spectating: bool) -> PreviewReturn {
        // 判据是“是否从预览画面返回”，而不依赖打断标志是否已置位——
        // 房主开始往往就发生在预览结束的同一瞬间，只看标志会漏掉提示。
        if self.stop.take().is_none() {
            return PreviewReturn::Ignore;
        }
        self.interrupt = None;
        // 观战者只旁观：观看结束不询问准备
        if spectating {
            return PreviewReturn::Ignore;
        }
        match room {
            Some(RoomState::Playing) => PreviewReturn::AlreadyStarted,
            // 房间停在等待就绪、而自己尚未就绪：房主已开始等自己准备
            Some(RoomState::WaitingForReady) if !is_ready => PreviewReturn::AskReady,
            _ => PreviewReturn::Ignore,
        }
    }

    /// 弹「房主要开始游戏啦」确认框；点「准备」后由 [`Self::take_ready_request`] 消费。
    pub fn ask_ready(&self) {
        let confirm = Arc::clone(&self.ready_confirm);
        Dialog::plain(mtl!("preview-interrupted-title"), mtl!("preview-interrupted-content"))
            .buttons(vec![mtl!("preview-not-now").into_owned(), mtl!("preview-ready").into_owned()])
            .listener(move |_dialog, id| {
                if id == 1 {
                    confirm.store(true, Ordering::Relaxed);
                }
                false
            })
            .show();
    }

    /// 确认框的「准备」是否被点过（一次性消费）。
    #[inline]
    pub fn take_ready_request(&self) -> bool {
        self.ready_confirm.swap(false, Ordering::Relaxed)
    }

    /// 房间状态轮询：开始时若还在选谱/本地谱阶段，则一旦离开该阶段即视为“房主开始”；
    /// 若是在等待准备阶段开始预览，则只有真正开局（Playing）才打断。
    async fn watch_loop(client: Arc<Client>, interrupt: Arc<AtomicBool>, stop: Arc<AtomicBool>) {
        let initial = client.blocking_room_state();
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Some(state) = client.blocking_room_state() else {
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
