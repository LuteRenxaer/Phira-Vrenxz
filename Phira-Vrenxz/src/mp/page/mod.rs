//! 多人场景的整屏分页界面。
//!
//! 多人模式从一个「从右侧滑出的面板 + 居中浮层」改成**独立整屏场景 + 整屏分页**：
//! 每个页面都占满整个屏幕，统一由 [`super::theme`] 的骨架布局
//! （页头：返回 + 标题；中部：内容；底部：操作条）组织，页面之间用整页淡入切换，
//! 不存在任何居中浮窗。
//!
//! 页面结构：
//! - [`connect`]：未连接（连接服务器）
//! - [`lobby`]：已连接未进房（创建 / 加入 / 公共房间 / 断开连接）
//! - [`room_list`]：公共房间列表（点行加入，行内「观战」旁观）
//! - [`room`]：房间主体（状态卡、谱面下载/同步状态行、玩家入口、聊天流、操作条）
//! - [`players`]：玩家列表 + 房主对某玩家的管理页
//! - [`results`]：对局结算排名
//! - [`spectate`]：观战（实时统计 + 同步观战 / 退出观战）

pub mod connect;
pub mod lobby;
pub mod players;
pub mod results;
pub mod room;
pub mod room_list;
pub mod spectate;

/// 多人场景内的页面。
///
/// `Manage` 带目标玩家 id：房主在玩家列表里点某一行后进入该页。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Connect,
    Lobby,
    RoomList,
    Room,
    Players,
    Manage(i32),
    Results,
    Spectate,
}

/// 所有页面的 UI 部件（按钮池、滚动区）与各自的渲染/触摸。
#[derive(Default)]
pub struct Pages {
    pub connect: connect::ConnectPage,
    pub lobby: lobby::LobbyPage,
    pub room_list: room_list::RoomListPage,
    pub room: room::RoomPage,
    pub players: players::PlayersPage,
    pub manage: players::ManagePage,
    pub results: results::ResultsPage,
    pub spectate: spectate::SpectatePage,
}

impl Pages {
    pub fn new() -> Self {
        Self::default()
    }

    /// 每帧渲染前让所有按钮失效：只有本帧真正绘制到的按钮才会重建命中区，
    /// 因此切页后不会残留上一页的命中区。
    pub fn invalidate(&mut self) {
        self.connect.invalidate();
        self.lobby.invalidate();
        self.room_list.invalidate();
        self.room.invalidate();
        self.players.invalidate();
        self.manage.invalidate();
        self.results.invalidate();
        self.spectate.invalidate();
    }

    /// 滚动惯性（所有页面的滚动区都更新，代价可忽略且不会漏掉惯性收尾）。
    pub fn update(&mut self, t: f32) {
        self.room_list.update(t);
        self.room.update(t);
        self.players.update(t);
        self.results.update(t);
        self.spectate.update(t);
    }
}
