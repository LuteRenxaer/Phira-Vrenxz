
pub mod connect;
pub mod lobby;
pub mod players;
pub mod room;
pub mod servers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Connect,
    /// 服务器管理（地址簿），挂在连接页之上
    Servers,
    Lobby,
    Room,
    Manage(i32),
}

#[derive(Default)]
pub struct Pages {
    pub connect: connect::ConnectPage,
    pub servers: servers::ServersPage,
    pub lobby: lobby::LobbyPage,
    pub room: room::RoomPage,
    pub manage: players::ManagePage,
}

impl Pages {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate(&mut self) {
        self.connect.invalidate();
        self.servers.invalidate();
        self.lobby.invalidate();
        self.room.invalidate();
        self.manage.invalidate();
    }

    pub fn update(&mut self, t: f32) {
        self.lobby.update(t);
        self.room.update(t);
    }
}
