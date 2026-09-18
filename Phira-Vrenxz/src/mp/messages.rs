//! 房间消息流：把服务端下发的 `Message` 渲染成滚动列表，并把聊天输入
//! 送出去。房间页要显示的“当前谱面名”也顺带从这里取出（服务端只把谱面名放在
//! 选谱消息里，房间状态里没有），因此 [`MessageLog::ingest`] 会返回 [`RoomNotice`]。

use macroquad::prelude::*;
use phira_mp_client::Client;
use phira_mp_common::Message as MpMessage;
use prpr::{
    ext::semi_white,
    ui::{DrawText, Scroll, Ui},
};

use super::theme::{FS_BODY, FS_SMALL};
use crate::mp::L10N_LOCAL;

/// 消息流里的一条消息（坐标由渲染时测量写入）。
struct Message {
    content: String,
    y: f32,
    bottom: f32,
    color: Color,
}

impl Message {
    fn text<'a, 's, 'ui>(&'s self, ui: &'ui mut Ui<'a>, max_width: f32) -> DrawText<'a, 's, 'ui> {
        ui.text(&self.content)
            .pos(0., self.y)
            .size(FS_BODY)
            .color(self.color)
            .max_width(max_width)
            .multiline()
    }
}

/// 从消息流中提取的选谱信息（房间页左面板要显示「谱面:xxx」）。
#[derive(Debug, Clone)]
pub enum RoomNotice {
    /// 在线谱面的名字
    OnlineChart { name: String },
    /// 本地谱面的名字
    LocalChart { name: String },
}

/// 消息列表（含滚动与增量布局）。
pub struct MessageLog {
    scroll: Scroll,
    msgs: Vec<Message>,
    /// 从这一条开始布局失效（屏幕尺寸变化时归零）
    dirty_from: usize,
}

impl Default for MessageLog {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageLog {
    pub fn new() -> Self {
        Self {
            scroll: Scroll::new(),
            msgs: Vec::new(),
            dirty_from: 0,
        }
    }

    pub fn clear(&mut self) {
        self.msgs.clear();
        self.dirty_from = 0;
    }

    /// 屏幕尺寸变化时使全部消息重新测量。
    pub fn invalidate_layout(&mut self) {
        self.dirty_from = 0;
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    /// 滚动区的触摸处理；返回 true 表示本次触摸已被消息列表消费。
    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        self.scroll.contains(touch) && self.scroll.touch(touch, t)
    }

    /// 消费服务端消息：转成消息流文本，并返回其中带下来的谱面名。
    pub fn ingest(&mut self, client: &Client, incoming: Vec<MpMessage>) -> Vec<RoomNotice> {
        let mut notices = Vec::new();
        self.msgs.extend(incoming.into_iter().map(|msg| match msg {
            MpMessage::Chat { user, content } => Message {
                // user == 0 为服务器系统消息（如进房欢迎提示），不显示发送者前缀
                content: if user == 0 { content } else { format!("{}：{content}", client.user_name(user)) },
                y: 0.,
                bottom: 0.,
                color: if user == 0 { semi_white(0.7) } else { WHITE },
            },
            msg => {
                // 谱面名只有选谱消息里带（房间状态里没有），顺手记下来给房间页用
                match &msg {
                    MpMessage::SelectChart { id, name, .. } if *id > 0 => {
                        notices.push(RoomNotice::OnlineChart { name: name.clone() })
                    }
                    MpMessage::SelectLocalChart { name, .. } => notices.push(RoomNotice::LocalChart { name: name.clone() }),
                    _ => {}
                }
                Message {
                    content: system_text(client, msg),
                    y: 0.,
                    bottom: 0.,
                    color: semi_white(0.7),
                }
            }
        }));
        notices
    }

    /// 消息列表（含布局测量与滚动），`r` 为面板局部坐标下的消息区矩形。
    pub fn render(&mut self, ui: &mut Ui, r: Rect) {
        ui.scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            let mut y = if self.dirty_from == 0 {
                0.
            } else {
                self.msgs.get(self.dirty_from - 1).map_or(0., |it| it.bottom)
            };
            let old_dirty = self.dirty_from != self.msgs.len();
            for msg in &mut self.msgs[self.dirty_from..] {
                msg.y = y + 0.02;
                msg.bottom = msg.text(ui, r.w).measure().bottom();
                y = msg.bottom;
            }
            if old_dirty {
                let o = y - r.h;
                if o >= 0. {
                    self.scroll.y_scroller.goto = Some(o);
                }
            }
            self.dirty_from = self.msgs.len();
            self.scroll.size((r.w, r.h));
            let offset = self.scroll.y_scroller.offset;
            self.scroll.render(ui, |ui| {
                if self.msgs.is_empty() {
                    ui.text(mtl!("mp-msg-none"))
                        .pos(r.w / 2., 0.04)
                        .anchor(0.5, 0.)
                        .size(FS_SMALL)
                        .color(semi_white(0.4))
                        .draw();
                    return (r.w, r.h);
                }
                for msg in &self.msgs {
                    if msg.bottom < offset {
                        continue;
                    }
                    if msg.y > offset + r.h {
                        break;
                    }
                    msg.text(ui, r.w).draw();
                }
                (r.w, self.msgs.last().map(|it| it.bottom).unwrap_or_default() + 0.03)
            });
        });
    }
}

/// 把服务端的房间事件翻译成一条系统消息文本。
fn system_text(client: &Client, msg: MpMessage) -> String {
    use phira_mp_common::Message as M;
    match msg {
        M::Chat { .. } => unreachable!("chat handled separately"),
        M::CreateRoom { user, .. } => mtl!("msg-create-room", "user" => client.user_name(user)),
        M::JoinRoom { name, .. } => mtl!("msg-join-room", "user" => name),
        M::LeaveRoom { name, .. } => mtl!("msg-leave-room", "user" => name),
        M::NewHost { user, .. } => mtl!("msg-new-host", "user" => client.user_name(user)),
        M::SelectChart { user, name, id } => {
            mtl!("msg-select-chart", "user" => client.user_name(user), "chart" => name, "id" => id)
        }
        M::GameStart { user, .. } => mtl!("msg-game-start", "user" => client.user_name(user)),
        M::Ready { user, .. } => mtl!("msg-ready", "user" => client.user_name(user)),
        M::CancelReady { user, .. } => mtl!("msg-cancel-ready", "user" => client.user_name(user)),
        M::CancelGame { user, .. } => mtl!("msg-cancel-game", "user" => client.user_name(user)),
        M::StartPlaying => mtl!("msg-start-playing").into_owned(),
        M::Played {
            user,
            score,
            accuracy,
            full_combo,
        } => mtl!(
            "msg-played",
            "user" => client.user_name(user),
            "score" => format!("{score:07}"),
            "accuracy" => format!("{:.2}%", accuracy * 100.),
            "full-combo" => full_combo.to_string()
        ),
        M::GameEnd => mtl!("msg-game-end").into_owned(),
        M::Abort { user, .. } => mtl!("msg-abort", "user" => client.user_name(user)),
        M::LockRoom { lock } => mtl!("msg-room-lock", "lock" => lock.to_string()),
        M::CycleRoom { cycle } => mtl!("msg-room-cycle", "cycle" => cycle.to_string()),
        M::SelectLocalChart { user, name, .. } => {
            mtl!("msg-select-local-chart", "user" => client.user_name(user), "chart" => name)
        }
        M::SendChart { user, .. } => mtl!("msg-send-chart", "user" => client.user_name(user)),
        M::DownloadReady { user, .. } => mtl!("msg-download-ready", "user" => client.user_name(user)),
        M::Kicked { user, name, .. } => {
            if Some(user) == client.me().as_ref().map(|it| it.id) {
                mtl!("msg-kicked-me").into_owned()
            } else {
                mtl!("msg-kicked", "user" => name.as_str())
            }
        }
        // 服务端在本局结束时发来的玩家成绩汇总：独立的展示页面已经删掉，
        // 所以这里只在消息流里留一行「几名玩家完成」，让房间里的人知道上一局结束了
        // （对应的文案 key 仍在 locales 里，由文案那边决定怎么改写或删除）
        M::RoomResults { results } => mtl!("msg-room-results", "n" => results.len() as u64),
        // 多人对局中某玩家暂停/继续
        M::PlayerPaused { user, paused } => {
            let key = if paused { "msg-player-paused" } else { "msg-player-resumed" };
            mtl!(key, "user" => client.user_name(user))
        }
    }
}
