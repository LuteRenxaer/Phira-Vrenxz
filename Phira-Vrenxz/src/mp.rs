
prpr_l10n::tl_file!("multiplayer" mtl);

mod actions;
mod messages;
mod page;
mod preview;
mod scene;
pub(crate) mod serve;
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

    pub static MP_SESSION: RefCell<Option<MpSession>> = RefCell::default();

    static MP_ENTER_REQUEST: Cell<bool> = const { Cell::new(false) };

    static MP_OPEN_LIBRARY: Cell<bool> = const { Cell::new(false) };
}

pub fn request_enter() {
    MP_ENTER_REQUEST.with(|it| it.set(true));
}

pub fn take_enter_request() -> bool {
    MP_ENTER_REQUEST.with(|it| it.replace(false))
}

pub fn request_open_library() {
    MP_OPEN_LIBRARY.with(|it| it.set(true));
}

pub fn take_open_library_request() -> bool {
    MP_OPEN_LIBRARY.with(|it| it.replace(false))
}

pub fn session_ready() -> bool {
    MP_SESSION.with(|it| it.borrow().is_some())
}

#[derive(Debug, Clone, Default)]
pub struct PendingRoomLink {

    pub join: Option<String>,

    pub create: Option<String>,

    pub server: Option<String>,
}

static PENDING_ROOM_LINK: Mutex<Option<PendingRoomLink>> = Mutex::new(None);

pub fn set_pending_room_link(link: PendingRoomLink) {
    *PENDING_ROOM_LINK.lock().unwrap() = Some(link);
}

pub fn take_pending_room_link() -> Option<PendingRoomLink> {
    PENDING_ROOM_LINK.lock().unwrap().take()
}

#[cfg(test)]
mod l10n_tests {
    const LOCALES: [&str; 3] = ["zh-CN", "en-US", "zh-TW"];
    const SOURCES: [&str; 3] = [
        include_str!("../locales/zh-CN/multiplayer.ftl"),
        include_str!("../locales/en-US/multiplayer.ftl"),
        include_str!("../locales/zh-TW/multiplayer.ftl"),
    ];

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
