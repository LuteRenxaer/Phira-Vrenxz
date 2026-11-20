//! 跨设备（局域网）数据包传输（移植自 Phira-Firefly）。
//!
//! 发送端：TCP 监听 + UDP 广播应答（携带设备名与端口，不含配对码）。
//! 接收端：UDP 广播发现 → 输入配对码连接 → 等待发送端点击“开始传输”→ 下载 → 导入。
use super::{apply_imported_data, prepare_import, run_import, Conflict, Progress};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Read, Write},
    net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

/// 局域网数据包发现的 UDP 广播端口（与联机房间的 27015 区分开）。
pub const DISCOVERY_PORT: u16 = 27100;
const DISCOVER_ADDR: &str = "255.255.255.255:27100";
const DISCOVER_INTERVAL: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum Msg {
    Discover,
    Offer { name: String, port: u16 },
    Pair { code: String, name: String },
    Paired { size: u64 },
    Error { error: String },
    Start { size: u64 },
}

fn write_msg(stream: &mut impl Write, msg: &Msg) -> Result<()> {
    serde_json::to_writer(&mut *stream, msg)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn read_msg(reader: &mut impl BufRead) -> Result<Msg> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        bail!("connection closed");
    }
    Ok(serde_json::from_str(line.trim())?)
}

/// 本机显示名称：优先使用电脑名，取不到时用固定名称。
pub fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| "Phira-Vrenxz".to_owned())
}

fn random_code() -> String {
    format!("{:06}", rand::random::<u32>() % 1_000_000)
}

fn temp_path() -> PathBuf {
    std::env::temp_dir().join(format!("phira-data-{}.pfd", uuid::Uuid::new_v4()))
}

/// 局域网内可接收的设备。
#[derive(Clone, Debug)]
pub struct Device {
    pub addr: SocketAddr,
    pub name: String,
}

impl Device {
    pub fn ip(&self) -> IpAddr {
        self.addr.ip()
    }
}

// ===== 发送端 =====
pub enum SenderEvent {
    Connected { name: String, peer: String },
    Progress { done: u64, total: u64 },
    Finished,
    Error(String),
}

enum SenderCmd {
    Start,
}

pub struct Sender {
    pub code: String,
    events: mpsc::Receiver<SenderEvent>,
    cmd: mpsc::Sender<SenderCmd>,
    stop: Arc<AtomicBool>,
}

impl Sender {
    /// 启动发送端：监听随机 TCP 端口并开始应答局域网发现广播。
    pub fn start(name: String, package: PathBuf, size: u64) -> Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", 0)).context("failed to bind transfer port")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let code = random_code();
        let stop = Arc::new(AtomicBool::new(false));

        let udp_stop = Arc::clone(&stop);
        let offer_name = name.clone();
        thread::spawn(move || {
            let Ok(socket) = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)) else {
                tracing::warn!("failed to bind data transfer discovery port");
                return;
            };
            let _ = socket.set_read_timeout(Some(POLL_INTERVAL));
            let mut buf = [0u8; 1024];
            while !udp_stop.load(Ordering::Relaxed) {
                let Ok((n, addr)) = socket.recv_from(&mut buf) else {
                    continue;
                };
                if matches!(serde_json::from_slice(&buf[..n]), Ok(Msg::Discover)) {
                    let msg = Msg::Offer {
                        name: offer_name.clone(),
                        port,
                    };
                    if let Ok(text) = serde_json::to_string(&msg) {
                        let _ = socket.send_to(text.as_bytes(), addr);
                    }
                }
            }
        });

        let (evt_tx, evt_rx) = mpsc::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let tcp_stop = Arc::clone(&stop);
        let expected = code.clone();
        let events = evt_tx;
        thread::spawn(move || {
            while !tcp_stop.load(Ordering::Relaxed) {
                let (stream, peer) = match listener.accept() {
                    Ok(it) => it,
                    Err(_) => {
                        thread::sleep(POLL_INTERVAL);
                        continue;
                    }
                };
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
                let mut writer = match stream.try_clone() {
                    Ok(it) => it,
                    Err(_) => continue,
                };
                let mut reader = BufReader::new(stream);
                let paired = (|| -> Result<bool> {
                    match read_msg(&mut reader)? {
                        Msg::Pair { code: input, name } if input.trim() == expected => {
                            write_msg(&mut writer, &Msg::Paired { size })?;
                            let _ = events.send(SenderEvent::Connected {
                                name,
                                peer: peer.ip().to_string(),
                            });
                            Ok(true)
                        }
                        Msg::Pair { .. } => {
                            write_msg(&mut writer, &Msg::Error { error: "invalid code".to_owned() })?;
                            Ok(false)
                        }
                        _ => Ok(false),
                    }
                })()
                .unwrap_or(false);
                if !paired {
                    let _ = events.send(SenderEvent::Error("pair failed".to_owned()));
                    continue;
                }
                if cmd_rx.recv().is_ok() {
                    let res = send_package(&mut writer, &package, size, &events);
                    if let Err(err) = res {
                        let _ = events.send(SenderEvent::Error(err.to_string()));
                    }
                }
                break;
            }
        });

        Ok(Self {
            code,
            events: evt_rx,
            cmd: cmd_tx,
            stop,
        })
    }

    pub fn try_recv(&self) -> Option<SenderEvent> {
        self.events.try_recv().ok()
    }

    /// 通知后台线程开始把数据包发给已配对的接收端。
    pub fn begin(&self) {
        let _ = self.cmd.send(SenderCmd::Start);
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn send_package(stream: &mut TcpStream, path: &Path, size: u64, events: &mpsc::Sender<SenderEvent>) -> Result<()> {
    write_msg(stream, &Msg::Start { size })?;
    let mut file = BufReader::new(File::open(path)?);
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        stream.write_all(&buf[..n])?;
        done += n as u64;
        let _ = events.send(SenderEvent::Progress { done, total: size });
    }
    stream.flush()?;
    let _ = events.send(SenderEvent::Finished);
    let _ = stream.shutdown(Shutdown::Write);
    let _ = fs::remove_file(path);
    Ok(())
}

// ===== 接收端 =====
pub enum ReceiverEvent {
    Devices(Vec<Device>),
    Paired,
    Progress { done: u64, total: u64 },
    Received(PathBuf),
    Error(String),
}

enum ReceiverCmd {
    Pair { addr: SocketAddr, code: String },
}

pub struct Receiver {
    events: mpsc::Receiver<ReceiverEvent>,
    cmd: mpsc::Sender<ReceiverCmd>,
    stop: Arc<AtomicBool>,
}

impl Receiver {
    pub fn start(name: String) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let (evt_tx, evt_rx) = mpsc::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel();

        // 发现广播
        let udp_stop = Arc::clone(&stop);
        let events = evt_tx.clone();
        thread::spawn(move || {
            let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) else {
                return;
            };
            let _ = socket.set_broadcast(true);
            let _ = socket.set_read_timeout(Some(POLL_INTERVAL));
            let mut buf = [0u8; 1024];
            let mut last_broadcast = None::<std::time::Instant>;
            while !udp_stop.load(Ordering::Relaxed) {
                let now = std::time::Instant::now();
                if last_broadcast.map(|it| now.duration_since(it) >= DISCOVER_INTERVAL).unwrap_or(true) {
                    last_broadcast = Some(now);
                    let text = serde_json::to_string(&Msg::Discover).unwrap_or_default();
                    let _ = socket.send_to(text.as_bytes(), DISCOVER_ADDR);
                }
                let mut devices = Vec::new();
                let deadline = std::time::Instant::now() + POLL_INTERVAL;
                loop {
                    if std::time::Instant::now() >= deadline || udp_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok((n, addr)) = socket.recv_from(&mut buf) else {
                        continue;
                    };
                    if let Ok(Msg::Offer { name, port }) = serde_json::from_slice(&buf[..n]) {
                        let device = Device {
                            addr: SocketAddr::new(addr.ip(), port),
                            name,
                        };
                        if !devices.iter().any(|it: &Device| it.addr == device.addr) {
                            devices.push(device);
                        }
                    }
                }
                if !devices.is_empty() {
                    let _ = events.send(ReceiverEvent::Devices(devices));
                }
            }
        });

        // 配对与下载
        let tcp_stop = Arc::clone(&stop);
        let events = evt_tx;
        thread::spawn(move || {
            while !tcp_stop.load(Ordering::Relaxed) {
                let Ok(ReceiverCmd::Pair { addr, code }) = cmd_rx.recv() else {
                    break;
                };
                let _ = events.send(ReceiverEvent::Devices(Vec::new()));
                if let Err(err) = pair_and_download(addr, &code, &name, &events) {
                    let _ = events.send(ReceiverEvent::Error(err.to_string()));
                }
            }
        });

        Ok(Self {
            events: evt_rx,
            cmd: cmd_tx,
            stop,
        })
    }

    pub fn try_recv(&self) -> Option<ReceiverEvent> {
        self.events.try_recv().ok()
    }

    pub fn pair(&self, addr: SocketAddr, code: String) -> Result<()> {
        self.cmd.send(ReceiverCmd::Pair { addr, code }).map_err(|_| anyhow!("transfer thread stopped"))
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn pair_and_download(
    addr: SocketAddr,
    code: &str,
    name: &str,
    events: &mpsc::Sender<ReceiverEvent>,
) -> Result<PathBuf> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).context("failed to connect")?;
    stream.set_read_timeout(Some(Duration::from_secs(20)))?;
    stream.set_write_timeout(Some(Duration::from_secs(20)))?;
    write_msg(
        &mut stream,
        &Msg::Pair {
            code: code.trim().to_owned(),
            name: name.to_owned(),
        },
    )?;
    let mut reader = BufReader::new(stream.try_clone()?);
    match read_msg(&mut reader)? {
        Msg::Paired { .. } => {
            let _ = events.send(ReceiverEvent::Paired);
        }
        Msg::Error { error } => bail!("{error}"),
        _ => bail!("unexpected response"),
    }
    // 等待发送端点击“开始传输”
    stream.set_read_timeout(None)?;
    let size = match read_msg(&mut reader)? {
        Msg::Start { size } => size,
        Msg::Error { error } => bail!("{error}"),
        _ => bail!("unexpected response"),
    };
    let path = temp_path();
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);
    let mut buf = vec![0u8; 64 * 1024];
    let mut remaining = size;
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let n = reader.read(&mut buf[..want])?;
        if n == 0 {
            bail!("connection closed during transfer");
        }
        writer.write_all(&buf[..n])?;
        remaining -= n as u64;
        let _ = events.send(ReceiverEvent::Progress {
            done: size - remaining,
            total: size,
        });
    }
    writer.flush()?;
    let _ = events.send(ReceiverEvent::Received(path.clone()));
    Ok(path)
}

// ===== 发送会话（UI 侧状态机） =====
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SendPhase {
    /// 正在打包
    Preparing,
    /// 等待接收端配对
    Waiting,
    /// 正在传输
    Transferring,
    /// 传输完成
    Finished,
}

pub struct SendSession {
    pub phase: SendPhase,
    pub code: Option<String>,
    pub peer: Option<String>,
    pub progress: Arc<Progress>,
    pub error: Option<String>,
    sender: Option<Sender>,
    preparing: Option<mpsc::Receiver<Result<(Sender, String)>>>,
    started: bool,
}

impl SendSession {
    /// 在 UI 线程收集条目后，后台打包并启动发送端。
    pub fn new(selected: &[bool; super::CATEGORIES.len()], conflict: Conflict) -> Result<Self> {
        let entries = super::collect_entries(selected, conflict)?;
        let total = entries.iter().map(|it| it.size()).sum::<u64>();
        let progress = Arc::new(Progress::new(total));
        let thread_progress = Arc::clone(&progress);
        let (tx, rx) = mpsc::channel();
        let name = device_name();
        thread::spawn(move || {
            let res = (|| -> Result<(Sender, String)> {
                let path = temp_path();
                let file = File::create(&path)?;
                super::write_zip(entries, file, &thread_progress)?;
                let size = fs::metadata(&path)?.len();
                let sender = Sender::start(name, path, size)?;
                let code = sender.code.clone();
                Ok((sender, code))
            })();
            let _ = tx.send(res);
        });
        Ok(Self {
            phase: SendPhase::Preparing,
            code: None,
            peer: None,
            progress,
            error: None,
            sender: None,
            preparing: Some(rx),
            started: false,
        })
    }

    pub fn update(&mut self) {
        if let Some(rx) = &self.preparing {
            match rx.try_recv() {
                Ok(Ok((sender, code))) => {
                    self.code = Some(code);
                    self.sender = Some(sender);
                    self.preparing = None;
                    self.phase = SendPhase::Waiting;
                }
                Ok(Err(err)) => {
                    self.error = Some(err.to_string());
                    self.preparing = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("prepare thread panicked".to_owned());
                    self.preparing = None;
                }
            }
        }
        if let Some(sender) = &self.sender {
            while let Some(ev) = sender.try_recv() {
                match ev {
                    SenderEvent::Connected { name, peer } => {
                        self.peer = Some(format!("{name} ({peer})"));
                    }
                    SenderEvent::Progress { done, total } => {
                        self.progress.set(done, total);
                        self.phase = SendPhase::Transferring;
                    }
                    SenderEvent::Finished => {
                        self.phase = SendPhase::Finished;
                    }
                    SenderEvent::Error(err) => self.error = Some(err),
                }
            }
        }
    }

    pub fn begin(&mut self) {
        if self.started {
            return;
        }
        if let Some(sender) = &self.sender {
            self.started = true;
            sender.begin();
        }
    }

    pub fn can_begin(&self) -> bool {
        self.phase == SendPhase::Waiting && self.peer.is_some() && !self.started
    }
}

// ===== 接收会话（UI 侧状态机） =====
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RecvPhase {
    /// 搜索局域网内的发送设备
    Discovering,
    /// 已配对，等待发送端开始传输
    WaitingStart,
    /// 正在下载
    Downloading,
    /// 正在导入
    Importing,
    /// 导入完成（需要重启）
    Done,
}

pub struct ReceiveSession {
    receiver: Receiver,
    pub devices: Vec<Device>,
    pub phase: RecvPhase,
    pub progress: Arc<Progress>,
    pub error: Option<String>,
    pending_data: Option<Vec<u8>>,
    job: Option<mpsc::Receiver<Result<()>>>,
}

impl ReceiveSession {
    pub fn new() -> Result<Self> {
        Ok(Self {
            receiver: Receiver::start(device_name())?,
            devices: Vec::new(),
            phase: RecvPhase::Discovering,
            progress: Arc::new(Progress::default()),
            error: None,
            pending_data: None,
            job: None,
        })
    }

    pub fn pair(&mut self, addr: SocketAddr, code: String) -> Result<()> {
        self.error = None;
        self.phase = RecvPhase::WaitingStart;
        self.receiver.pair(addr, code)
    }

    pub fn update(&mut self) {
        while let Some(ev) = self.receiver.try_recv() {
            match ev {
                ReceiverEvent::Devices(devices) => {
                    if self.phase == RecvPhase::Discovering {
                        self.devices = devices;
                    }
                }
                ReceiverEvent::Paired => {
                    self.phase = RecvPhase::WaitingStart;
                }
                ReceiverEvent::Progress { done, total } => {
                    self.phase = RecvPhase::Downloading;
                    self.progress.set(done, total);
                }
                ReceiverEvent::Received(path) => self.start_import(path),
                ReceiverEvent::Error(err) => {
                    if self.phase != RecvPhase::Importing && self.phase != RecvPhase::Done {
                        self.phase = RecvPhase::Discovering;
                        self.error = Some(err);
                    }
                }
            }
        }
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok(res) => {
                    self.job = None;
                    match res {
                        Ok(()) => {
                            if let Some(bytes) = self.pending_data.take() {
                                if let Err(err) = apply_imported_data(&bytes) {
                                    self.error = Some(err.to_string());
                                }
                            }
                            self.phase = RecvPhase::Done;
                        }
                        Err(err) => {
                            self.error = Some(err.to_string());
                            self.phase = RecvPhase::Discovering;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                    self.error = Some("import thread panicked".to_owned());
                    self.phase = RecvPhase::Discovering;
                }
            }
        }
    }

    fn start_import(&mut self, path: PathBuf) {
        match prepare_import(path.to_string_lossy().as_ref()) {
            Ok(plan) => {
                let progress = Arc::new(Progress::new(plan.total));
                let thread_progress = Arc::clone(&progress);
                let (tx, rx) = mpsc::channel();
                self.pending_data = plan.data_json.clone();
                self.progress = progress;
                self.phase = RecvPhase::Importing;
                self.job = Some(rx);
                thread::spawn(move || {
                    let res = run_import(path.to_string_lossy().to_string(), plan, &thread_progress);
                    let _ = fs::remove_file(&path);
                    let _ = tx.send(res);
                });
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.phase = RecvPhase::Discovering;
                let _ = fs::remove_file(&path);
            }
        }
    }

    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }
}
