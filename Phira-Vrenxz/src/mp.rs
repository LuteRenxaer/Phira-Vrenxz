//! 多人游戏（Phira-MP）。
//!
//! 结构：
//! - [`session::MpSession`]：唯一持有连接/房间状态与页面导航的对象，活在
//!   [`MP_SESSION`] 静态里，跨场景进出存活；
//! - [`scene::MultiplayerScene`]：与主菜单平级的整屏场景，只是会话的一层视图壳；
//! - [`page`]：整屏分页界面（连接 / 主页=房间大厅 / 房间 / 房主管理），
//!   所有页面共用 [`theme`] 的尺寸、字号与颜色体系；
//! - [`state`] / [`actions`]：协议状态与协议调用（无 UI）；
//! - [`messages`]：房间消息流与聊天；
//! - [`preview`]：谱面预览会话（后台任务、打断信号）；
//! - [`serve`]：本地谱面分享的打包 / 上传 / 下载。
//!
//! 多人模式内部**没有任何悬浮窗**：所有信息都是整屏页面。

prpr_l10n::tl_file!("multiplayer" mtl);

mod actions;
mod messages;
mod page;
mod preview;
mod scene;
mod serve;
mod session;
mod state;
pub mod theme;

pub use scene::MultiplayerScene;
pub use session::MpSession;

use std::{
    cell::{Cell, RefCell},
    sync::Mutex,
};

thread_local! {
    /// 多人会话：连接 / 房间 / 页面导航的唯一持有者。
    /// 场景可以反复进出，会话一直保留，因此“进游玩场景再回来”不会丢状态。
    pub static MP_SESSION: RefCell<Option<MpSession>> = RefCell::default();

    /// 一次性「进入多人场景」请求（主菜单按钮 / 谱面库选谱 / 深链接）。
    /// 由主场景在 `next_scene` 里消费成 `NextScene::Overlay(MultiplayerScene)`。
    static MP_ENTER_REQUEST: Cell<bool> = const { Cell::new(false) };

    /// 一次性「回到主菜单后打开谱面库」请求。
    ///
    /// 谱面库是主场景里的一个页面（`LibraryPage`），多人场景推不动它，
    /// 所以房间页点「谱面库」时的流程是：退出多人场景（房间与会话保留）→ 主场景收到这个请求
    /// → 直接把谱面库页面压上去。选完谱面后谱面库那边会再发一次进入多人场景的请求。
    static MP_OPEN_LIBRARY: Cell<bool> = const { Cell::new(false) };
}

/// 请求进入多人场景（主菜单点击多人按钮、谱面库中选谱、深链接）。
pub fn request_enter() {
    MP_ENTER_REQUEST.with(|it| it.set(true));
}

/// 取出「进入多人场景」请求（一次性消费）。
pub fn take_enter_request() -> bool {
    MP_ENTER_REQUEST.with(|it| it.replace(false))
}

/// 请求主场景打开谱面库页面（房间页的「谱面库」按钮用；见 [`MP_OPEN_LIBRARY`]）。
pub fn request_open_library() {
    MP_OPEN_LIBRARY.with(|it| it.set(true));
}

/// 取出「打开谱面库」请求（一次性消费）。
pub fn take_open_library_request() -> bool {
    MP_OPEN_LIBRARY.with(|it| it.replace(false))
}

/// 会话是否已创建（未创建时无法进入多人场景）。
pub fn session_ready() -> bool {
    MP_SESSION.with(|it| it.borrow().is_some())
}

/// 深链接（phira://）待处理的多人房间动作。
/// 在原生启动参数到达时暂存，等玩家进入多人场景并连上服务器后自动加入/创建房间。
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

/// 多人文案的自检：三语（zh-CN / en-US / zh-TW）必须都能被 Fluent 解析，
/// 且 key 集合完全一致（新增或删除文案时三语同步）。
#[cfg(test)]
mod l10n_tests {
    const LOCALES: [&str; 3] = ["zh-CN", "en-US", "zh-TW"];
    const SOURCES: [&str; 3] = [
        include_str!("../locales/zh-CN/multiplayer.ftl"),
        include_str!("../locales/en-US/multiplayer.ftl"),
        include_str!("../locales/zh-TW/multiplayer.ftl"),
    ];

    /// 取出顶层 `key =` 定义的 key（跳过注释、空行与 select 表达式的续行）。
    fn keys(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        for line in src.lines() {
            let line = line.trim_start();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, _)) = line.split_once('=') else { continue };
            let key = key.trim();
            if key.is_empty() || key.split_whitespace().count() != 1 || key.contains(['{', '}']) {
                continue;
            }
            out.push(key.to_owned());
        }
        out.sort();
        out
    }

    #[test]
    fn locales_parse_and_share_keys() {
        let mut sets = Vec::new();
        for (i, src) in SOURCES.iter().enumerate() {
            assert!(
                prpr_l10n::FluentResource::try_new((*src).to_owned()).is_ok(),
                "{} 的 multiplayer.ftl 无法被 Fluent 解析",
                LOCALES[i]
            );
            sets.push(keys(src));
        }
        for i in 1..sets.len() {
            assert_eq!(
                sets[0], sets[i],
                "{} 与 {} 的 multiplayer.ftl key 集合不一致",
                LOCALES[0], LOCALES[i]
            );
        }
    }
}
