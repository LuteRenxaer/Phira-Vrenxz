# 局域网联机功能

## 功能概述

Phira-Firefly 的局域网联机功能允许玩家在同一局域网内进行多人游戏，无需连接到互联网服务器。该功能包含以下特性：

- **服务器发现**：自动扫描局域网内的游戏服务器
- **房间管理**：创建房间、加入房间、管理成员列表
- **文件传输**：房主作为文件服务器，提供谱面文件下载
- **等待机制**：支持等待其他玩家准备就绪
- **音频控制**：自定义发声设备设置

## 架构设计

### 核心组件

1. **协议层 (protocol.rs)**
   - 定义局域网通信协议
   - 使用 JSON 格式进行消息序列化
   - 支持多种消息类型

2. **发现层 (discovery.rs)**
   - 使用 UDP 广播进行服务器发现
   - 自动扫描局域网内的服务器
   - 维护服务器列表

3. **服务器层 (server.rs)**
   - 处理房间连接和通信
   - 管理房间成员
   - 广播消息给所有客户端

4. **客户端层 (client.rs)**
   - 与服务器建立 TCP 连接
   - 发送和接收消息
   - 管理本地状态

5. **UI 层 (ui.rs)**
   - 提供用户界面
   - 处理用户输入
   - 显示房间信息和状态

6. **文件服务器 (file_server.rs)**
   - 房主作为文件服务器
   - 提供谱面文件下载
   - 支持 ZIP 格式传输

7. **等待界面 (waiting_ui.rs)**
   - 显示等待其他玩家界面
   - 显示准备进度
   - 支持离开房间

## 使用方法

### 1. 创建房间

```rust
use phira::lan::*;

let mut manager = LanManager::new();
let config = LanConfig {
    server_name: "My Room".to_string(),
    room_name: "Test Room".to_string(),
    max_players: 4,
    waiting_for_players: false,
    local_ip: "0.0.0.0".to_string(),
    tcp_port: 27016,
};

manager.create_room(config)?;
```

### 2. 加入房间

```rust
let server_addr = "192.168.1.100:27016".to_string();
let player_name = "Player1".to_string();

manager.join_room(server_addr, player_name)?;
```

### 3. 准备就绪

```rust
// 准备
manager.ready(true)?;

// 取消准备
manager.ready(false)?;
```

### 4. 开始游戏

```rust
// 开始游戏，不等待其他玩家
manager.start_game(false)?;

// 开始游戏，等待其他玩家
manager.start_game(true)?;
```

### 5. 更新音频设备设置

```rust
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

let selected_index = 0;
manager.update_audio_devices(devices, selected_index)?;
```

### 6. 请求下载谱面

```rust
let chart_id = "12345".to_string();
let chart_name = "Test Chart".to_string();

manager.request_download_chart(chart_id, chart_name)?;
```

## UI 使用

### 1. 显示局域网联机面板

```rust
let mut panel = LanPanel::new(Arc::new(Mutex::new(manager)));
panel.show(0.0);
```

### 2. 显示等待面板

```rust
let mut waiting_panel = WaitingPanel::new(Arc::new(Mutex::new(manager)));
waiting_panel.show(0.0, 4); // 4个玩家
```

### 3. 显示音频设备选择面板

```rust
let mut audio_panel = AudioDevicePanel::new();
audio_panel.show(vec!["设备1".to_string(), "设备2".to_string()]);
```

## 消息类型

### 客户端 -> 服务器

- `Join`: 加入房间
- `Ready`: 准备状态
- `StartGame`: 开始游戏
- `AudioDevices`: 音频设备设置
- `DownloadChart`: 请求下载谱面

### 服务器 -> 客户端

- `JoinResponse`: 加入房间响应
- `RoomMembers`: 房间成员列表
- `StartGame`: 开始游戏
- `AudioDevices`: 音频设备设置
- `DownloadProgress`: 下载进度
- `DownloadComplete`: 下载完成
- `PlayerReady`: 玩家准备状态
- `WaitingStatus`: 等待状态

## 网络配置

### 端口配置

- UDP 广播端口: 27015
- TCP 服务器端口范围: 27016-27025

### 消息格式

所有消息使用 JSON 格式，包含 `type` 字段标识消息类型。

## 示例代码

完整的示例代码请参考 `example.rs` 文件。

## 注意事项

1. **网络要求**：所有设备必须在同一局域网内
2. **防火墙**：确保相关端口未被防火墙阻止
3. **设备发现**：如果无法发现服务器，检查网络设置
4. **文件传输**：确保房主有足够的带宽上传谱面文件

## 故障排除

### 常见问题

1. **无法发现服务器**
   - 检查设备是否在同一局域网
   - 确认防火墙设置
   - 尝试手动刷新服务器列表

2. **无法加入房间**
   - 检查服务器地址是否正确
   - 确认房间未满
   - 检查网络连接

3. **文件下载失败**
   - 检查房主设备是否在线
   - 确认网络带宽
   - 尝试重新下载

## 性能优化

1. **文件传输**：使用 ZIP 压缩减少传输数据量
2. **消息广播**：优化消息广播频率
3. **连接管理**：及时断开不活跃的连接

## 扩展功能

1. **自定义协议**：可以扩展协议支持更多功能
2. **加密传输**：添加加密保护数据安全
3. **中继服务器**：支持跨子网连接
4. **房间管理**：添加房间密码、权限控制等

## 贡献

欢迎提交 Issue 和 Pull Request 来改进这个功能。
