//! Windows 平台 WebView2 桥接：仿 Phira-Mod-New launcher 的 Tauri+WebView2 方式，
//! 在游戏窗口内嵌入 wry WebView 子窗口承载 HTML 界面。
#![cfg(target_os = "windows")]

use std::{
    borrow::Cow,
    collections::VecDeque,
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use raw_window_handle::{
    HasWindowHandle, HandleError, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use windows_sys::Win32::{
    Foundation::{BOOL, HWND, LPARAM},
    UI::WindowsAndMessaging::{
        EnumChildWindows, FindWindowW, GetClassNameW, GetClientRect, SetForegroundWindow,
        ShowWindow, SW_HIDE,
    },
    UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus},
};
use wry::{
    dpi::{PhysicalPosition, PhysicalSize},
    http::{header, Request, Response, StatusCode},
    Rect, WebViewBuilder,
};

/// IPC 消息队列：JS -> Rust
pub type IpcQueue = Arc<Mutex<VecDeque<String>>>;

/// 注入到真实 HTML 页面的 phirc 桥接 shim。
/// 原页面期望 Tauri-like `window.phirc.*` Promise API，这里把它转发到 wry
/// 的 `window.ipc.postMessage`，Rust 侧处理后通过 `window.__phira_resolve` 回填。
const PHIRC_SHIM: &str = r#"(function () {
  if (window.__phira_shim_loaded) return;
  window.__phira_shim_loaded = true;
  var seq = 0;
  var pending = {};
  function invoke(cmd, args) {
    return new Promise(function (resolve, reject) {
      var id = ++seq;
      pending[id] = { resolve: resolve, reject: reject };
      window.ipc.postMessage(JSON.stringify({ t: 'phirc', id: id, cmd: cmd, args: args || {} }));
    });
  }
  window.__phira_resolve = function (id, err, data) {
    var p = pending[id];
    if (!p) return;
    delete pending[id];
    if (err) p.reject(new Error(err));
    else p.resolve(data);
  };
  window.phirc = {
    listCharts: function (o) { return invoke('listCharts', o || {}); },
    listLanguages: function () { return invoke('listLanguages'); },
    listOnlineCharts: function (o) { return invoke('listOnlineCharts', o || {}); },
    importChart: function () { return invoke('importChart'); },
    startGame: function (path) { return invoke('startGame', { path: path }); },
    startChallenge: function (paths, opts) { return invoke('startChallenge', { paths: paths, opts: opts || {} }); },
    openPage: function (name) { return invoke('openPage', { name: name }); },
    openUrl: function (url) { return invoke('openUrl', { url: url }); },
    getCustomRks: function () { return invoke('getCustomRks'); },
    getCustomSkin: function () { return invoke('getCustomSkin'); },
    listPlugins: function () { return Promise.resolve({ plugins: [], pending: [] }); },
    setPluginEnabled: function () { return Promise.resolve(true); },
    deletePlugin: function () { return Promise.resolve(false); },
    importPlugin: function () { return Promise.reject(new Error('unsupported')); },
    pluginEvent: function () { return Promise.resolve(); },
    log: function (m) { try { console.log('[phirc]', m); } catch (e) {} try { window.ipc.postMessage(JSON.stringify({ t: 'log', m: String(m) })); } catch (e) {} return Promise.resolve(); }
  };
  document.addEventListener('click', function (e) {
    var t = e.target;
    var btn = t && t.closest ? t.closest('#back-btn') : null;
    if (btn) {
      e.preventDefault();
      e.stopImmediatePropagation();
      window.phirc.openPage('back').catch(function () {});
      return;
    }
  }, true);
})();"#;

/// Windows 窗口句柄包装，实现 HasWindowHandle 供 wry build_as_child 使用
#[derive(Clone, Copy)]
pub struct HwndHandle(pub isize);

impl HasWindowHandle for HwndHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let hwnd = std::num::NonZeroIsize::new(self.0)
            .ok_or(HandleError::Unavailable)?;
        let handle = Win32WindowHandle::new(hwnd);
        #[allow(deprecated)]
        let raw = RawWindowHandle::Win32(handle);
        unsafe { Ok(WindowHandle::borrow_raw(raw)) }
    }
}

/// 桥接器：持有 WebView 与入队队列
pub struct WebBridge {
    pub webview: wry::WebView,
    queue: IpcQueue,
    pending_state: Mutex<String>,
    /// 上次 set_bounds 时的窗口客户区尺寸，用于 resize 去重
    last_size: Mutex<(i32, i32)>,
}

/// 按窗口标题查找游戏主窗口 HWND
pub fn find_main_hwnd(title: &str) -> Option<isize> {
    let wide: Vec<u16> = OsStr::new(title).encode_wide().chain(Some(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), wide.as_ptr());
        if hwnd.is_null() {
            None
        } else {
            Some(hwnd as isize)
        }
    }
}

/// 把键盘焦点归还给游戏主窗口（销毁 WebView 后调用，避免系统提示音 Ding 且保证输入可达）
pub fn focus_main(hwnd: isize) {
    unsafe {
        SetForegroundWindow(hwnd as HWND);
        // 键盘焦点需显式归还主窗口：仅 SetForegroundWindow 不转移焦点，
        // 销毁 WebView 子窗口后按键会被系统当作无焦点处理，持续触发 Ding 提示音
        SetFocus(hwnd as HWND);
    }
}

/// EnumChildWindows 回调：找到 WebView2 子窗口（类名 Chrome_WidgetWin_*）后隐藏并禁用，
/// 使其不再抢键盘焦点。返回 TRUE 继续枚举。
unsafe extern "system" fn hide_webview_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut buf = [0u16; 80];
    let len = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if len > 0 {
        let cls = String::from_utf16_lossy(&buf[..len as usize]);
        if cls.starts_with("Chrome_WidgetWin") {
            ShowWindow(hwnd, SW_HIDE);
            EnableWindow(hwnd, 0);
            if lparam != 0 {
                *(lparam as *mut bool) = true;
            }
        }
    }
    1 // TRUE：继续枚举
}

impl WebBridge {
    /// 显式关闭 WebView2 子窗口（隐藏 + 禁用），保证它不再残留抢焦点。
    /// wry 的 WebView Drop 只是释放 Rust 包装，WebView2 子窗口销毁是异步的，
    /// 若不主动隐藏禁用，销毁期间子窗口仍会吞掉键盘并触发系统 Ding。
    pub fn shutdown(&self, hwnd: isize) {
        unsafe {
            let mut found = false;
            EnumChildWindows(
                hwnd as HWND,
                Some(hide_webview_child),
                (&mut found as *mut bool) as LPARAM,
            );
            if found {
                // 隐藏后 WebView 还可能短暂存活，再补一次焦点归还
                focus_main(hwnd);
            }
        }
    }
}

impl WebBridge {
    /// 创建桥接器。hwnd 为父窗口句柄；html 为完整 HTML 字符串
    pub fn new(hwnd: isize, html: &str, queue: IpcQueue) -> anyhow::Result<Self> {
        let queue2 = Arc::clone(&queue);
        let mut builder = WebViewBuilder::new()
            .with_html(html.to_string())
            .with_initialization_script(PHIRC_SHIM)
            .with_focused(false)
            .with_ipc_handler(move |req| {
                let body = req.body().to_string();
                if let Ok(mut q) = queue2.lock() {
                    q.push_back(body);
                }
            });
        // 铺满父窗口客户区，避免默认 200x200 只显示一个小角落
        let (cw, ch) = unsafe { client_rect(hwnd) };
        if cw > 0 && ch > 0 {
            builder = builder.with_bounds(Rect {
                position: PhysicalPosition::new(0, 0).into(),
                size: PhysicalSize::new(cw, ch).into(),
            });
        }
        let webview = builder.build_as_child(&HwndHandle(hwnd))?;
        Ok(Self {
            webview,
            queue,
            pending_state: Mutex::new(String::new()),
            last_size: Mutex::new((0, 0)),
        })
    }

    /// 从 assets 目录加载真实 HTML（`phirc://` 自定义协议 + url 模式），
    /// 注入 `window.phirc` shim，并在 ready 后向 Rust 请求初始列表。
    pub fn new_from_dir(hwnd: isize, dir: &Path, queue: IpcQueue) -> anyhow::Result<Self> {
        let dir = dir.to_path_buf();
        let queue2 = Arc::clone(&queue);
        let mut builder = WebViewBuilder::new()
            .with_custom_protocol("phirc".to_string(), move |_id, req| {
                serve_dir(&dir, &req)
            })
            .with_url("phirc://localhost/challenge_ui.html")
            .with_initialization_script(PHIRC_SHIM)
            .with_focused(false)
            .with_ipc_handler(move |req| {
                let body = req.body().to_string();
                if let Ok(mut q) = queue2.lock() {
                    q.push_back(body);
                }
            });
        // 铺满父窗口客户区，避免默认 200x200 只显示一个小角落
        let (cw, ch) = unsafe { client_rect(hwnd) };
        if cw > 0 && ch > 0 {
            builder = builder.with_bounds(Rect {
                position: PhysicalPosition::new(0, 0).into(),
                size: PhysicalSize::new(cw, ch).into(),
            });
        }
        let webview = builder.build_as_child(&HwndHandle(hwnd))?;
        let (cw, ch) = unsafe { client_rect(hwnd) };
        Ok(Self {
            webview,
            queue,
            pending_state: Mutex::new(String::new()),
            last_size: Mutex::new((cw, ch)),
        })
    }

    /// 回复一次 JS Promise 调用（回调 window.__phira_resolve）。data 为已序列化的 JSON 文本。
    pub fn resolve(&mut self, id: u64, err: Option<&str>, data: &str) {
        let err_js = match err {
            Some(e) => format!("\"{}\"", e.replace('"', "\\\"")),
            None => "null".to_string(),
        };
        let script = format!(
            "window.__phira_resolve && window.__phira_resolve({}, {}, {});",
            id, err_js, data
        );
        let _ = self.webview.evaluate_script(&script);
    }

    /// 是否空闲（无待处理 JS 消息）
    pub fn idle(&self) -> bool {
        self.queue.lock().map(|q| q.is_empty()).unwrap_or(true)
    }

    /// 取出下一条 JS 消息（JSON 字符串）
    pub fn pop(&self) -> Option<String> {
        self.queue.lock().ok()?.pop_front()
    }

    /// 推送状态到 JS（调用 window.__phira_push）
    pub fn push_state(&mut self, json: &str) {
        let script = format!("window.__phira_push && window.__phira_push({json})");
        // 若上次推送尚未执行完则覆盖为最新
        let mut pending = self.pending_state.lock().unwrap();
        *pending = script.clone();
        let _ = self.webview.evaluate_script(&script);
    }

    /// 推送一条 toast 提示（带消息字段即可由 JS 侧渲染）
    pub fn toast(&mut self, text: &str) {
        let msg_json = format!(
            "{{\"msg\":{}}}",
            serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
        );
        self.push_state(&msg_json);
    }

    /// 按父窗口当前客户区尺寸同步 WebView 边界；仅在尺寸变化时执行 set_bounds。
    pub fn sync_bounds(&self, hwnd: isize) {
        let (cw, ch) = unsafe { client_rect(hwnd) };
        if cw <= 0 || ch <= 0 {
            return;
        }
        let mut last = self.last_size.lock().unwrap();
        if *last == (cw, ch) {
            return;
        }
        *last = (cw, ch);
        let _ = self.webview.set_bounds(Rect {
            position: PhysicalPosition::new(0, 0).into(),
            size: PhysicalSize::new(cw as u32, ch as u32).into(),
        });
    }
}

/// 获取窗口客户区尺寸（像素）
unsafe fn client_rect(hwnd: isize) -> (i32, i32) {
    let mut rect: windows_sys::Win32::Foundation::RECT = std::mem::zeroed();
    let ok = GetClientRect(hwnd as HWND, &mut rect);
    if ok == 0 {
        (0, 0)
    } else {
        (rect.right - rect.left, rect.bottom - rect.top)
    }
}

/// 获取游戏主窗口句柄（优先使用启动时 native_window_handle 缓存的真实 HWND，
/// 不依赖标题匹配，最可靠；失败退回标题查找/活动窗口）
pub fn lookup_main_hwnd(title: &str) -> Option<isize> {
    use std::sync::atomic::Ordering;
    let cached = crate::MAIN_HWND.load(Ordering::Relaxed);
    if cached != 0 {
        return Some(cached);
    }
    if let Some(h) = find_main_hwnd(title) {
        return Some(h);
    }
    None
}

/// 尝试创建桥接器；失败返回 Err 时调用方可降级回原生绘制
pub fn try_bridge(hwnd: isize, html: &str, queue: IpcQueue) -> anyhow::Result<WebBridge> {
    WebBridge::new(hwnd, html, queue)
}

/// 检查 HWND 是否有效
pub fn hwnd_valid(hwnd: isize) -> bool {
    hwnd != 0
}

/// phirc:// 自定义协议处理器：从 assets 目录按 URI 路径返回文件
fn serve_dir(
    dir: &Path,
    req: &Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let raw = req.uri().path().trim_start_matches('/');
    eprintln!("[serve_dir] uri={:?} raw={:?}", req.uri(), raw);
    // cover_cache/xx 前缀：从本地曲绘缓存目录读取缩略图（JPEG，无扩展名）
    if let Some(rest) = raw.strip_prefix("cover_cache/") {
        eprintln!("[serve_dir] -> cover_cache rest={:?}", rest);
        return serve_cover_cache(rest);
    }
    let path = percent_decode(raw);
    // 路径穿越防护
    if path.split('/').any(|seg| seg == "..") {
        return http_404();
    }
    let file_path = dir.join(path);
    match std::fs::read(&file_path) {
        Ok(data) => {
            let body: Cow<'static, [u8]> = Cow::Owned(data);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_of(&file_path))
                .header("Access-Control-Allow-Origin", "*")
                .body(body)
                .unwrap()
        }
        Err(_) => http_404(),
    }
}

/// 从本地曲绘缓存目录读取封面临时图。优先直接读谱面文件夹内的曲绘原图
/// （info.yml 的 illustration 字段，画质不被压缩）；原图不可用（如 builtin 谱面在 zip 内）
/// 时退回 cache/image 缩略图缓存（文件名 = 谱面路径中的 `/` 替换为 `_`），仍无则 404。
fn serve_cover_cache(rest: &str) -> Response<Cow<'static, [u8]>> {
    let local_path = percent_decode(rest);
    eprintln!("[cover] rest={:?} local_path={:?}", rest, local_path);
    // 路径穿越防护：不允许 `..` 段；Windows 下绝对路径/盘符前缀也拒绝
    if local_path.split('/').any(|seg| seg == "..") || local_path.contains(":\\") {
        eprintln!("[cover] blocked path traversal");
        return http_404();
    }
    // 1) 优先读本地谱面文件夹曲绘原图（保证封面画质；读不到时落回缩略图缓存）
    if !local_path.contains(':') {
        if let Some((data, mime)) = read_local_illustration(&local_path) {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header("Access-Control-Allow-Origin", "*")
                .body(Cow::Owned(data))
                .unwrap();
        }
    }
    // 2) 原图不可用（builtin 谱面等）退回缩略图缓存（key = local_path 归一化，`/` 与 `:` 替换为 `_`）
    let cache_key = local_path.replace('/', "_").replace(':', "_");
    let cache_file = match crate::dir::cache_image_local() {
        Ok(dir) => PathBuf::from(dir).join(&cache_key),
        Err(_) => return http_404(),
    };
    eprintln!("[cover] cache_key={:?} cache_file={:?} exists={}", cache_key, cache_file, cache_file.exists());
    if let Ok(data) = std::fs::read(&cache_file) {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header("Access-Control-Allow-Origin", "*")
            .body(Cow::Owned(data))
            .unwrap();
    }
    http_404()
}

/// 从本地谱面文件夹读取曲绘原图：解析 `<charts>/<local_path>/info.yml` 的 illustration 字段，
/// 读取对应图片文件并返回字节与 MIME。仅支持磁盘上的 download/custom 谱面。
fn read_local_illustration(local_path: &str) -> Option<(Vec<u8>, &'static str)> {
    let dir = crate::dir::charts().ok()?;
    let chart_dir = Path::new(&dir).join(local_path);
    let info_path = chart_dir.join("info.yml");
    let text = std::fs::read_to_string(&info_path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&text).ok()?;
    let illu = yaml.get("illustration").and_then(|v| v.as_str())?;
    let img_path = chart_dir.join(illu);
    let data = std::fs::read(&img_path).ok()?;
    let mime = mime_of(&img_path);
    Some((data, mime))
}

/// 简单 URL 百分号解码（中文文件名为 UTF-8 字节）
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = hex_val(bytes[i + 1]);
            let l = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (h, l) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 生成 cover_cache URL（文件名 = cache_image_local 下的图片名）
pub fn cover_cache_url(local_path: &str) -> Option<String> {
    if local_path.is_empty() {
        return None;
    }
    // 保留 '/' 作为路径分隔（服务端可原样还原谱面目录），其余字节百分号编码
    let mut enc = String::with_capacity(local_path.len() * 3);
    for b in local_path.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                enc.push(b as char)
            }
            _ => {
                enc.push('%');
                let hex = format!("{:02X}", b);
                enc.push_str(&hex);
            }
        }
    }
    Some(format!("phirc://localhost/cover_cache/{}", enc))
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn http_404() -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Cow::Borrowed(b"404 Not Found".as_slice()))
        .unwrap()
}

/// 简单 MIME 推断
fn mime_of(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "jfif" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// 空实现占位（非 Windows 时保持 API 形态一致）
#[cfg(not(target_os = "windows"))]
pub fn try_bridge(
    _hwnd: isize,
    _html: &str,
    _queue: IpcQueue,
) -> anyhow::Result<WebBridge> {
    anyhow::bail!("webview 仅支持 Windows")
}

#[allow(dead_code)]
type _Hwnd = HWND;