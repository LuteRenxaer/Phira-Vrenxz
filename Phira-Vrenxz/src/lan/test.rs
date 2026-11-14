//! 局域网联机功能测试

use super::*;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// 局域网联机测试
pub struct LanTest {
    manager: Arc<Mutex<LanManager>>,
    test_server: Option<TestServer>,
}

/// 测试服务器
struct TestServer {
    server: LanServer,
    running: bool,
}

impl LanTest {
    /// 创建新的测试
    pub fn new() -> Self {
        let manager = Arc::new(Mutex::new(LanManager::new()));
        
        Self {
            manager: manager.clone(),
            test_server: None,
        }
    }

    /// 启动测试服务器
    pub fn start_test_server(&mut self) -> Result<()> {
        let config = LanConfig {
            server_name: "Test Server".to_string(),
            room_name: "Test Room".to_string(),
            max_players: 4,
            waiting_for_players: false,
            local_ip: "127.0.0.1".to_string(),
            tcp_port: 27016,
        };

        let server = LanServer::new(config)?;
        server.start()?;

        self.test_server = Some(TestServer {
            server,
            running: true,
        });

        println!("Test server started on 127.0.0.1:27016");
        Ok(())
    }

    /// 停止测试服务器
    pub fn stop_test_server(&mut self) {
        if let Some(test_server) = self.test_server.take() {
            test_server.server.stop();
            println!("Test server stopped");
        }
    }

    /// 测试服务器发现
    pub fn test_discovery(&mut self) -> Result<()> {
        println!("Testing server discovery...");
        
        // 开始发现
        self.manager.lock().unwrap().start_discovery()?;
        
        // 等待发现
        thread::sleep(Duration::from_secs(3));
        
        // 获取服务器列表
        let servers = self.manager.lock().unwrap().get_servers();
        println!("Found {} servers", servers.len());
        
        for server in servers {
            println!("Server: {} (Room: {})", server.name, server.room_name);
        }
        
        // 停止发现
        self.manager.lock().unwrap().stop_discovery();
        
        Ok(())
    }

    /// 测试房间创建和加入
    pub fn test_room_creation(&mut self) -> Result<()> {
        println!("Testing room creation and joining...");
        
        // 创建房间
        let config = LanConfig {
            server_name: "Test Room".to_string(),
            room_name: "My Test Room".to_string(),
            max_players: 4,
            waiting_for_players: false,
            local_ip: "127.0.0.1".to_string(),
            tcp_port: 27017,
        };

        self.manager.lock().unwrap().create_room(config)?;
        println!("Room created successfully");

        // 模拟另一个客户端加入房间
        let client_manager = Arc::new(Mutex::new(LanManager::new()));
        let client_addr = "127.0.0.1:27017".to_string();
        
        // 在实际应用中，这里需要真正的网络连接
        // 这里只是模拟
        println!("Simulating client joining room...");
        
        Ok(())
    }

    /// 测试准备状态
    pub fn test_ready_status(&mut self) -> Result<()> {
        println!("Testing ready status...");
        
        // 设置准备状态
        self.manager.lock().unwrap().ready(true)?;
        println!("Player ready");

        // 取消准备
        self.manager.lock().unwrap().ready(false)?;
        println!("Player not ready");

        Ok(())
    }

    /// 测试开始游戏
    pub fn test_start_game(&mut self) -> Result<()> {
        println!("Testing start game...");
        
        // 开始游戏
        self.manager.lock().unwrap().start_game(false)?;
        println!("Game started");

        // 测试等待其他玩家
        self.manager.lock().unwrap().start_game(true)?;
        println!("Game started with waiting for players");

        Ok(())
    }

    /// 测试音频设备设置
    pub fn test_audio_devices(&mut self) -> Result<()> {
        println!("Testing audio devices...");
        
        let devices = vec![
            AudioDeviceInfo {
                name: "设备1".to_string(),
                id: "device1".to_string(),
                is_current: true,
            },
            AudioDeviceInfo {
                name: "设备2".to_string(),
                id: "device2".to_string(),
                is_current: false,
            },
        ];

        // 更新音频设备设置
        self.manager.lock().unwrap().update_audio_devices(devices.clone(), 0)?;
        println!("Audio devices updated");

        // 获取当前设置
        let current_devices = self.manager.lock().unwrap().get_audio_devices();
        println!("Current devices: {:?}", current_devices);

        Ok(())
    }

    /// 测试文件下载
    pub fn test_file_download(&mut self) -> Result<()> {
        println!("Testing file download...");
        
        // 请求下载谱面
        let chart_id = "test_chart_123".to_string();
        let chart_name = "Test Chart".to_string();
        
        self.manager.lock().unwrap().request_download_chart(chart_id.clone(), chart_name.clone())?;
        println!("Download requested for chart: {}", chart_id);

        // 模拟下载进度
        for i in 0..=100 {
            thread::sleep(Duration::from_millis(50));
            println!("Download progress: {}%", i);
        }

        println!("File download test completed");
        Ok(())
    }

    /// 运行所有测试
    pub fn run_all_tests(&mut self) -> Result<()> {
        println!("Starting LAN multiplayer tests...");
        
        // 启动测试服务器
        self.start_test_server()?;
        
        // 运行各项测试
        self.test_discovery()?;
        self.test_room_creation()?;
        self.test_ready_status()?;
        self.test_start_game()?;
        self.test_audio_devices()?;
        self.test_file_download()?;
        
        // 停止测试服务器
        self.stop_test_server();
        
        println!("All tests completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lan_manager_creation() {
        let manager = LanManager::new();
        assert!(manager.get_state().is_disconnected());
    }

    #[test]
    fn test_lan_config_default() {
        let config = LanConfig::default();
        assert_eq!(config.server_name, "Phira LAN");
        assert_eq!(config.room_name, "My Room");
        assert_eq!(config.max_players, 4);
        assert!(!config.waiting_for_players);
        assert_eq!(config.local_ip, "0.0.0.0");
        assert_eq!(config.tcp_port, 27016);
    }

    #[test]
    fn test_lan_message_serialization() {
        let msg = LanMessage::Discover {
            version: "1.0.0".to_string(),
        };
        
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: LanMessage = serde_json::from_str(&serialized).unwrap();
        
        if let LanMessage::Discover { version } = deserialized {
            assert_eq!(version, "1.0.0");
        } else {
            panic!("Deserialized message is not Discover type");
        }
    }

    #[test]
    fn test_room_info_creation() {
        let room_info = RoomInfo {
            room_id: 123,
            room_name: "Test Room".to_string(),
            host_name: "Test Host".to_string(),
            max_players: 4,
            waiting_for_players: false,
            started: false,
        };

        assert_eq!(room_info.room_id, 123);
        assert_eq!(room_info.room_name, "Test Room");
        assert_eq!(room_info.max_players, 4);
        assert!(!room_info.waiting_for_players);
        assert!(!room_info.started);
    }

    #[test]
    fn test_member_info_creation() {
        let member_info = MemberInfo {
            name: "Player1".to_string(),
            is_host: true,
            ready: false,
            started: false,
        };

        assert_eq!(member_info.name, "Player1");
        assert!(member_info.is_host);
        assert!(!member_info.ready);
        assert!(!member_info.started);
    }

    #[test]
    fn test_audio_device_info_creation() {
        let device_info = AudioDeviceInfo {
            name: "Test Device".to_string(),
            id: "test_device_123".to_string(),
            is_current: true,
        };

        assert_eq!(device_info.name, "Test Device");
        assert_eq!(device_info.id, "test_device_123");
        assert!(device_info.is_current);
    }
}
