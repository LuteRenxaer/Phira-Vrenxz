prpr_l10n::tl_file!("multiplayer" mtl);

mod panel;
pub mod serve;
pub use panel::MPPanel;

use std::sync::Mutex;

/// 深链接（phira://）待处理的多人房间动作。
/// 在原生启动参数到达时暂存，等玩家进入多人面板并连上服务器后自动加入/创建房间。
#[derive(Debug, Clone, Default)]
pub struct PendingRoomLink {
    /// 加入的房间码（phira://room/join/<code>）
    pub join: Option<String>,
    /// 创建房间用的房间 id（phira://room/create/<id>）
    pub create: Option<String>,
    /// 房间所在服务器地址（可选；为空时使用当前配置的 mp_address）
    pub server: Option<String>,
}

static PENDING_ROOM_LINK: Mutex<Option<PendingRoomLink>> = Mutex::new(None);

/// 保存待处理的深链接动作（在游戏数据初始化前也可能被调用，只暂存）。
pub fn set_pending_room_link(link: PendingRoomLink) {
    *PENDING_ROOM_LINK.lock().unwrap() = Some(link);
}

/// 取出待处理的深链接动作（一次性消费）。
pub fn take_pending_room_link() -> Option<PendingRoomLink> {
    PENDING_ROOM_LINK.lock().unwrap().take()
}
