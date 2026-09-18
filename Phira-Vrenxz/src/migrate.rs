//! 旧版本（PhirLie / 旧版 Phira-Vrenxz）数据检测与同步。
//!
//! **与多人模式无关**：只在启动后、主菜单里跑一次。
//!
//! 1. 在若干「旧安装可能出现的位置」找旧版本的 data/。认两条：目录名像本作
//!    （PhirLie / Phira-Vrenxz，刻意不认上游 Phira、Phira-Firefly 这些同源分支），
//!    以及 data/data.json 里确实有本作那些字段（config + charts/me/tokens 之一），
//!    免得把别的程序的目录认成旧版本；
//! 2. 找到就弹窗问玩家要不要把旧数据同步过来；
//! 3. 选「同步旧数据」→ 把旧 data/ 合并复制到当前 data/（先备份当前 data.json，最后才
//!    覆盖 data.json，中途失败也不会留下「data.json 指向还没复制过来的谱面」），完成后
//!    提示玩家重新启动游戏；**本次运行不再写 data.json**（否则退出时会把同步结果盖回上
//!    一份数据），见 migrated()；
//! 4. 选「直接进入游戏」→ 记下这个目录，之后不再询问；
//! 5. 成功同步过一次之后**不再自动提示**（标记文件见 SYNCED_FILE）——需要再同步别的旧
//!    安装时，走「设置 → 存储与重置 → 同步旧版本数据」，也就是 [`manual_sync`]。

prpr_l10n::tl_file!("migrate" mtl);

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    time::SystemTime,
};

use anyhow::{Context, Result};
use prpr::{
    scene::{show_message, DIALOG},
    time::TimeManager,
    ui::Dialog,
};
use tracing::{info, warn};

use crate::dir;

/// 已经问过玩家的旧目录（选「直接进入游戏」的那些）：写在当前 data/ 里，一行一个绝对路径。
const ASKED_FILE: &str = "legacy-asked.txt";
/// 成功同步过一次的标记：存在它就不再自动提示（想恢复自动提示删掉这个文件即可）。
const SYNCED_FILE: &str = "legacy-synced.txt";
/// 用户配置目录下存放「本程序用过的安装根目录」的子目录 / 文件名。
const ROOTS_DIR: &str = "Phira-Vrenxz";
const ROOTS_FILE: &str = "roots.txt";

/// 启动后延迟多久再问（让开场动画先走完）。
const ASK_DELAY: f32 = 1.5;
/// 扫描时最多看多少个目录（防止在大目录上卡启动）。
const MAX_SCAN_DIRS: usize = 600;
/// 每个「基目录」最多看多少个直接子目录。
const MAX_CHILDREN: usize = 200;
/// 记住多少个用过的安装根目录。
const MAX_ROOTS: usize = 16;

/// 本次运行已经同步过旧数据：save_data 直接跳过，别把同步结果盖回去。
static MIGRATED: AtomicBool = AtomicBool::new(false);
/// 一次性检测是否已经跑过。
static STARTED: AtomicBool = AtomicBool::new(false);

/// 是否需要跳过写 data.json（crate::save_data 用）。
pub fn migrated() -> bool {
    MIGRATED.load(Ordering::Relaxed)
}

/// 找到的一处旧版本数据。
#[derive(Debug, Clone)]
struct Legacy {
    /// 旧安装的根目录（data/ 的上一层）
    root: PathBuf,
    /// 旧安装的 data/
    data_dir: PathBuf,
    /// 目录名，展示给玩家看
    name: String,
    /// 旧 data.json 的修改时间
    modified: SystemTime,
    /// 旧数据里的本地谱面数
    charts: usize,
    /// 旧数据里有没有登录信息
    logged_in: bool,
    /// 从哪里找到的（排查用）
    source: &'static str,
}

/// 同步统计。
#[derive(Debug, Default)]
struct Stats {
    files: u64,
    bytes: u64,
    /// 当前 data.json 的备份文件名
    backup: Option<String>,
}

/// 复制进度（后台线程写，主线程读）。
#[derive(Default)]
struct Progress {
    files: AtomicU64,
    bytes: AtomicU64,
}

/// 对话框里点了按钮之后要执行的动作。
///
/// **不能在对话框的 listener 里直接做**：那段代码跑在 DIALOG.borrow_mut() 的作用域里，
/// 再弹一个新对话框（Dialog::show 要 borrow_mut）就是 RefCell 双重借用 panic，
/// 所以 listener 只把动作记下来，由下一帧的 tick 执行。
enum Action {
    Sync(Legacy),
    Skip(PathBuf),
    Exit,
}

/// 同步任务状态。
enum State {
    Idle,
    Copying {
        rx: Receiver<std::result::Result<Stats, String>>,
        progress: Arc<Progress>,
    },
}

thread_local! {
    static PENDING: std::cell::RefCell<Option<Action>> = const { std::cell::RefCell::new(None) };
    static STATE: std::cell::RefCell<State> = const { std::cell::RefCell::new(State::Idle) };
}

/// 每帧调用（MainScene::update）：一次性检测 → 询问 → 同步进度 → 收尾。
pub fn tick(tm: &TimeManager) {
    // Android 的数据目录是应用私有的，不存在「隔壁还放着一个旧版本」这回事
    if cfg!(target_os = "android") {
        return;
    }
    if let Some(action) = PENDING.with(|it| it.borrow_mut().take()) {
        handle(action);
    }
    poll_copy();
    if !STARTED.load(Ordering::Relaxed) && tm.real_time() as f32 > ASK_DELAY {
        STARTED.store(true, Ordering::Relaxed);
        remember_root();
        // 同步过一次之后就不再自动打扰玩家了（要再同步别的旧安装请走设置里的入口）
        if !synced_before() {
            ask(false);
        }
    }
}

/// 设置页「同步旧版本数据」：手动找一次旧数据。
///
/// 和自动提示的区别：**不看「已经问过」的记录**（玩家可能之前选了「直接进入游戏」，
/// 现在又想同步了），找不到就弹个提示。
pub fn manual_sync() {
    if cfg!(target_os = "android") {
        show_message(mtl!("migrate-none")).warn();
        return;
    }
    // 已经有一个同步任务在跑（进度弹窗还开着）就别再开了
    let busy = STATE.with(|it| matches!(&*it.borrow(), State::Copying { .. }));
    if busy {
        return;
    }
    if !ask(true) {
        show_message(mtl!("migrate-none")).warn();
    }
}

// ---------- 询问 / 收尾 ----------

/// 找一处旧数据并询问玩家；返回是否真的问出来了。
///
/// `manual` = 来自设置页的手动同步：不看「已经问过」的记录，第二个按钮也改成「取消」。
fn ask(manual: bool) -> bool {
    let Some(legacy) = detect(manual) else { return false };
    info!(root = %legacy.root.display(), source = legacy.source, "found legacy data");
    // 这三个先算好：mtl! 的实参里塞闭包 / if 表达式会让宏解析炸掉
    let login = if legacy.logged_in {
        mtl!("migrate-login-yes")
    } else {
        mtl!("migrate-login-no")
    }
    .into_owned();
    let name = legacy.name.clone();
    let path = legacy.root.display().to_string();
    let charts = legacy.charts.to_string();
    let time = format_time(legacy.modified);
    let msg = mtl!(
        "migrate-message",
        "name" => name,
        "path" => path,
        "charts" => charts,
        "time" => time,
        "login" => login
    );
    let sync_label = mtl!("migrate-sync").into_owned();
    let skip_label = if manual {
        mtl!("migrate-cancel").into_owned()
    } else {
        mtl!("migrate-skip").into_owned()
    };
    let owned = legacy.clone();
    Dialog::plain(mtl!("migrate-title"), msg)
        .buttons(vec![sync_label, skip_label])
        .listener(move |_dialog, pos| {
            PENDING.with(|it| {
                *it.borrow_mut() = Some(if pos == 0 {
                    Action::Sync(owned.clone())
                } else {
                    Action::Skip(owned.root.clone())
                })
            });
            false
        })
        .show();
    true
}

fn handle(action: Action) {
    match action {
        Action::Skip(root) => {
            info!(path = %root.display(), "legacy data skipped by player");
            if let Err(err) = mark_asked(&root) {
                warn!(?err, "failed to remember skipped legacy dir");
            }
        }
        Action::Sync(legacy) => {
            info!(root = %legacy.root.display(), "syncing legacy data");
            let progress = Arc::new(Progress::default());
            let rx = spawn_copy(legacy, Arc::clone(&progress));
            STATE.with(|it| *it.borrow_mut() = State::Copying { rx, progress });
            let msg = mtl!("migrate-syncing-message", "files" => "0".to_owned(), "size" => human_size(0));
            Dialog::plain(mtl!("migrate-syncing-title"), msg).show();
        }
        Action::Exit => exit_game(),
    }
}

fn exit_game() {
    prpr::ui::cleanup_audio();
    std::process::exit(0);
}

fn poll_copy() {
    enum Event {
        Progress(u64, u64),
        Done(std::result::Result<Stats, String>),
    }

    let ev = STATE.with(|it| {
        let st = it.borrow();
        let State::Copying { rx, progress } = &*st else { return None };
        match rx.try_recv() {
            Ok(res) => Some(Event::Done(res)),
            Err(TryRecvError::Empty) => Some(Event::Progress(
                progress.files.load(Ordering::Relaxed),
                progress.bytes.load(Ordering::Relaxed),
            )),
            Err(TryRecvError::Disconnected) => Some(Event::Done(Err("同步线程意外退出".to_owned()))),
        }
    });

    match ev {
        None => {}
        Some(Event::Progress(files, bytes)) => {
            // 文字每改一次都要重新排版，隔十几帧刷一次就够
            static N: AtomicU64 = AtomicU64::new(0);
            if N.fetch_add(1, Ordering::Relaxed) % 12 == 0 {
                let msg = mtl!("migrate-syncing-message", "files" => files.to_string(), "size" => human_size(bytes));
                DIALOG.with(|it| {
                    if let Some(dialog) = it.borrow_mut().as_mut() {
                        dialog.set_message(msg);
                    }
                });
            }
        }
        Some(Event::Done(res)) => {
            STATE.with(|it| *it.borrow_mut() = State::Idle);
            match res {
                Ok(stats) => {
                    MIGRATED.store(true, Ordering::Relaxed);
                    info!(files = stats.files, bytes = stats.bytes, "legacy data synced");
                    show_done(&stats);
                }
                Err(err) => {
                    warn!(%err, "failed to sync legacy data");
                    show_fail(&err);
                }
            }
        }
    }
}

fn show_done(stats: &Stats) {
    let files = stats.files.to_string();
    let size = human_size(stats.bytes);
    let backup = stats.backup.clone().unwrap_or_else(|| "-".to_owned());
    let msg = mtl!(
        "migrate-done-message",
        "files" => files,
        "size" => size,
        "backup" => backup
    );
    Dialog::plain(mtl!("migrate-done-title"), msg)
        .buttons(vec![mtl!("migrate-done-exit").into_owned(), mtl!("migrate-done-later").into_owned()])
        .listener(|_dialog, pos| {
            if pos == 0 {
                PENDING.with(|it| *it.borrow_mut() = Some(Action::Exit));
            }
            false
        })
        .show();
}

fn show_fail(err: &str) {
    let msg = mtl!("migrate-fail-message", "error" => err.to_owned());
    Dialog::plain(mtl!("migrate-fail-title"), msg)
        .buttons(vec![mtl!("migrate-ok").into_owned()])
        .show();
}

// ---------- 同步 ----------

fn spawn_copy(legacy: Legacy, progress: Arc<Progress>) -> Receiver<std::result::Result<Stats, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let res = apply(&legacy, &progress).map_err(|err| format!("{err:#}"));
        let _ = tx.send(res);
    });
    rx
}

/// 把旧 data/ 合并复制到当前 data/。
///
/// 顺序很重要：先复制除 data.json 之外的所有东西，**最后**才覆盖 data.json。这样中途出错
/// 或被强杀时，当前版本读到的还是自己那份完整数据，只是多了些旧谱面文件，不会出现
/// 「data.json 记着一堆还没复制过来的谱面」。
fn apply(legacy: &Legacy, progress: &Progress) -> Result<Stats> {
    let dst = PathBuf::from(dir::root()?);
    let mut stats = Stats::default();

    // 1) 先把当前 data.json 备份一份
    let cur_json = dst.join("data.json");
    if cur_json.is_file() {
        let backup = dst.join(format!("data.json.bak-{}", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        fs::copy(&cur_json, &backup).with_context(|| format!("备份 {} 失败", cur_json.display()))?;
        stats.backup = backup.file_name().map(|it| it.to_string_lossy().into_owned());
    }

    // 2) 旧 data/ 里除 data.json 之外的东西
    let entries = fs::read_dir(&legacy.data_dir).with_context(|| format!("读取 {} 失败", legacy.data_dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        // 软链接 / 目录联接不跟：可能指到别处，也可能成环
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        // 本作自己的记帐文件不参与覆盖（legacy-asked.txt 走下面的合并）
        if name == "data.json" || name.starts_with("data.json.bak-") || name == SYNCED_FILE {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if file_type.is_dir() {
            copy_dir(&from, &to, progress, &mut stats)?;
        } else if file_type.is_file() {
            if name == ASKED_FILE {
                // 「已经问过」的记录要并起来，别把新版本这边的覆盖掉
                merge_lines(&from, &to)?;
                continue;
            }
            copy_file(&from, &to, progress, &mut stats)?;
        }
    }

    // 3) 最后覆盖 data.json（旧账号 / 设置 / 成绩都在这里）
    copy_file(&legacy.data_dir.join("data.json"), &cur_json, progress, &mut stats)?;

    // 4) 记下这个旧目录；顺便打下「已经同步过」的标记 —— 之后不再自动提示（要再同步就
    //    走设置页的手动入口）
    mark_asked(&legacy.root)?;
    mark_synced(&legacy.root);
    Ok(stats)
}

fn copy_dir(src: &Path, dst: &Path, progress: &Progress, stats: &mut Stats) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&from, &to, progress, stats)?;
        } else if file_type.is_file() {
            copy_file(&from, &to, progress, stats)?;
        }
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path, progress: &Progress, stats: &mut Stats) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst).with_context(|| format!("复制 {} 失败", src.display()))?;
    stats.files += 1;
    stats.bytes += fs::metadata(dst).map(|it| it.len()).unwrap_or(0);
    progress.files.store(stats.files, Ordering::Relaxed);
    progress.bytes.store(stats.bytes, Ordering::Relaxed);
    Ok(())
}

// ---------- 检测 ----------

/// 找一处最值得同步的旧版本数据（最近改过的那个）。
fn detect(ignore_asked: bool) -> Option<Legacy> {
    let cur = current_root()?;
    let cur_data = cur.join("data");
    let ours = fs::read(cur_data.join("data.json")).unwrap_or_default();
    let asked = if ignore_asked { HashSet::new() } else { asked_set() };

    let mut found: Vec<Legacy> = Vec::new();
    for (dir, source) in candidate_dirs(&cur) {
        if let Some(legacy) = inspect(&dir, source, &cur, &cur_data, &asked, &ours) {
            found.push(legacy);
        }
    }
    // 同时有多份时挑「内容最多」的那份：先比谱面数，再比有没有登录，最后比新旧。
    // 只比修改时间不行 —— 玩家机器上常有好几个版本的残留，最近动过的那份未必是他的主力档。
    found.into_iter().max_by_key(|it| (it.charts, it.logged_in, it.modified))
}

/// 当前安装根目录（init_assets 已经把工作目录切到含 assets/ 的那一层）。
fn current_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    if cwd.join("data").is_dir() {
        return Some(cwd);
    }
    let root = PathBuf::from(dir::root().ok()?);
    root.parent().map(|it| it.to_path_buf())
}

/// 可能放着旧安装的目录（每个基目录本身 + 它的一层子目录）。
fn candidate_dirs(cur: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut bases: Vec<(PathBuf, &'static str)> = Vec::new();
    // 记录过的安装目录：同一个程序换过位置时最准
    for root in roots_file().map(|it| read_lines(&it)).unwrap_or_default() {
        bases.push((PathBuf::from(root), "recorded"));
    }
    // 当前目录自己 + 旁边 + 上一两级：新版解压到旧版隔壁是最常见的情况
    bases.push((cur.to_path_buf(), "current"));
    if let Some(parent) = cur.parent() {
        bases.push((parent.to_path_buf(), "sibling"));
        if let Some(grand) = parent.parent() {
            bases.push((grand.to_path_buf(), "nearby"));
        }
    }
    // 桌面 / 下载 / 文档（玩家经常直接从下载目录里解压）
    if let Some(home) = home_dir() {
        for name in ["Desktop", "Downloads", "Documents"] {
            bases.push((home.join(name), "user-dir"));
        }
    }
    // 各盘符根目录
    for drive in drive_roots() {
        bases.push((drive, "drive-root"));
    }

    let mut out: Vec<(PathBuf, &'static str)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (base, source) in bases {
        if out.len() >= MAX_SCAN_DIRS {
            break;
        }
        if !base.is_dir() {
            continue;
        }
        push_candidate(&mut out, &mut seen, base.clone(), source);
        let Ok(entries) = fs::read_dir(&base) else { continue };
        for entry in entries.flatten().take(MAX_CHILDREN) {
            if out.len() >= MAX_SCAN_DIRS {
                break;
            }
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // 子目录按名字先筛一道：既快，也避免把别的程序的 data/ 认成旧版本
            let name = entry.file_name();
            if !name_looks_like_ours(&name.to_string_lossy()) {
                continue;
            }
            push_candidate(&mut out, &mut seen, path, source);
        }
    }
    out
}

fn push_candidate(out: &mut Vec<(PathBuf, &'static str)>, seen: &mut HashSet<String>, path: PathBuf, source: &'static str) {
    if seen.insert(norm_path(&path)) {
        out.push((path, source));
    }
}

/// 目录名像不像**本作**（PhirLie / Phira-Vrenxz）。
///
/// 刻意不认「Phira」「Phira-Firefly」这类同源分支：数据结构和本作不完全一样，
/// 把它们的数据同步过来只会把配置 / 谱面记录搞乱，也会平白打扰玩家。
fn name_looks_like_ours(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["phirlie", "phir_lie", "phir-lie", "vrenxz"].iter().any(|key| name.contains(key))
}

/// 检查一个目录是不是「旧版本安装」：有 data/data.json，而且内容确实像本作的数据。
fn inspect(dir: &Path, source: &'static str, cur: &Path, cur_data: &Path, asked: &HashSet<String>, ours: &[u8]) -> Option<Legacy> {
    let norm = norm_path(dir);
    if norm == norm_path(cur) || dir.starts_with(cur_data) {
        return None;
    }
    // 问过（或者已经同步过）就不再问
    if asked.contains(&norm) {
        return None;
    }
    let data_dir = dir.join("data");
    let data_json = data_dir.join("data.json");
    if !data_json.is_file() {
        return None;
    }
    let bytes = fs::read(&data_json).ok()?;
    // 和当前那份一模一样 = 就是同一份数据（或早就同步过了）
    if bytes.is_empty() || bytes == ours {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let object = value.as_object()?;
    // 认一下「这确实是本作的 data.json」：config 必有，另外至少要有下面之一
    if !object.contains_key("config") {
        return None;
    }
    if !["charts", "me", "tokens", "localRecords"].iter().any(|key| object.contains_key(*key)) {
        return None;
    }
    let charts = object.get("charts").and_then(|it| it.as_array()).map_or(0, |it| it.len());
    let logged_in = object.get("me").is_some_and(|it| !it.is_null()) || object.get("tokens").is_some_and(|it| !it.is_null());
    let modified = fs::metadata(&data_json).and_then(|it| it.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    let name = dir
        .file_name()
        .map(|it| it.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string());
    Some(Legacy {
        root: dir.to_path_buf(),
        data_dir,
        name,
        modified,
        charts,
        logged_in,
        source,
    })
}

// ---------- 「已经问过」的记录 ----------

fn asked_file() -> Option<PathBuf> {
    Some(PathBuf::from(dir::root().ok()?).join(ASKED_FILE))
}

fn mark_asked(root: &Path) -> Result<()> {
    let Some(file) = asked_file() else { return Ok(()) };
    let norm = norm_path(root);
    let mut list = read_lines(&file);
    // 文件里存的是「给人看的绝对路径」，比较一律走 norm_path（Windows 下忽略大小写）
    if list.iter().any(|it| norm_path(Path::new(it)) == norm) {
        return Ok(());
    }
    list.push(root.display().to_string());
    write_lines(&file, &list)
}

/// 是否已经成功同步过一次（同步过就不再自动提示）。
fn synced_before() -> bool {
    synced_file().map(|it| it.is_file()).unwrap_or(false)
}

fn synced_file() -> Option<PathBuf> {
    Some(PathBuf::from(dir::root().ok()?).join(SYNCED_FILE))
}

/// 记下「已经同步过一次」：文件里写一行时间 + 来源，方便玩家自己看。
fn mark_synced(root: &Path) {
    let Some(file) = synced_file() else { return };
    let line = format!("{} {}", format_time(SystemTime::now()), root.display());
    if let Err(err) = fs::write(&file, format!("{line}\n")) {
        warn!(?err, "failed to write synced marker");
    }
}

/// 已经问过的旧目录（规范化后，便于比较）。
fn asked_set() -> HashSet<String> {
    asked_file()
        .map(|it| read_lines(&it).iter().map(|it| norm_path(Path::new(it))).collect())
        .unwrap_or_default()
}

// ---------- 用过的安装根目录 ----------

fn roots_file() -> Option<PathBuf> {
    Some(config_dir()?.join(ROOTS_DIR).join(ROOTS_FILE))
}

/// 记住当前安装根目录：下次换了位置，新版本还能认出「这就是以前那个安装」。
fn remember_root() {
    let (Some(file), Some(root)) = (roots_file(), current_root()) else { return };
    let norm = norm_path(&root);
    let mut list = read_lines(&file);
    if list.iter().any(|it| norm_path(Path::new(it)) == norm) {
        return;
    }
    list.push(root.display().to_string());
    let extra = list.len().saturating_sub(MAX_ROOTS);
    if extra > 0 {
        list.drain(..extra);
    }
    if let Err(err) = write_lines(&file, &list) {
        warn!(?err, "failed to remember install root");
    }
}

// ---------- 小工具 ----------

/// 规范化路径（用于比较 / 记录）：尽量转成绝对路径，Windows 下忽略大小写。
fn norm_path(path: &Path) -> String {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/').to_owned();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|it| it.lines().map(|it| it.trim().to_owned()).filter(|it| !it.is_empty()).collect())
        .unwrap_or_default()
}

fn write_lines(path: &Path, lines: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    fs::write(path, content).with_context(|| format!("写入 {} 失败", path.display()))?;
    Ok(())
}

/// 把 src 里的行并进 dst（去重）。
fn merge_lines(src: &Path, dst: &Path) -> Result<()> {
    let mut list = read_lines(dst);
    for line in read_lines(src) {
        if !list.iter().any(|it| *it == line) {
            list.push(line);
        }
    }
    write_lines(dst, &list)
}

fn config_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(path));
    }
    home_dir().map(|it| it.join(".config"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|it| it.is_dir())
}

fn drive_roots() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return Vec::new();
    }
    (b'A'..=b'Z')
        .map(|it| PathBuf::from(format!("{}:\\", it as char)))
        .filter(|it| it.is_dir())
        .collect()
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    const GB: f64 = MB * 1024.;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else {
        format!("{:.0} KB", bytes / KB)
    }
}

fn format_time(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(time).format("%Y-%m-%d %H:%M").to_string()
}

/// 文案自检：15 种语言都必须能解析，且 key 集合完全一致（和 multiplayer.ftl 的检查同一套规矩）。
#[cfg(test)]
mod l10n_tests {
    use prpr_l10n::FluentResource;

    const SOURCES: [(&str, &str); 15] = [
        ("de-DE", include_str!("../locales/de-DE/migrate.ftl")),
        ("en-US", include_str!("../locales/en-US/migrate.ftl")),
        ("fr-FR", include_str!("../locales/fr-FR/migrate.ftl")),
        ("id-ID", include_str!("../locales/id-ID/migrate.ftl")),
        ("ja-JP", include_str!("../locales/ja-JP/migrate.ftl")),
        ("ko-KR", include_str!("../locales/ko-KR/migrate.ftl")),
        ("mn-MN", include_str!("../locales/mn-MN/migrate.ftl")),
        ("pl-PL", include_str!("../locales/pl-PL/migrate.ftl")),
        ("pt-BR", include_str!("../locales/pt-BR/migrate.ftl")),
        ("ru-RU", include_str!("../locales/ru-RU/migrate.ftl")),
        ("th-TH", include_str!("../locales/th-TH/migrate.ftl")),
        ("tr-TR", include_str!("../locales/tr-TR/migrate.ftl")),
        ("vi-VN", include_str!("../locales/vi-VN/migrate.ftl")),
        ("zh-CN", include_str!("../locales/zh-CN/migrate.ftl")),
        ("zh-TW", include_str!("../locales/zh-TW/migrate.ftl")),
    ];

    /// 取出顶层 `key =` 定义的 key（跳过注释、空行与续行）。
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
        for (lang, src) in SOURCES {
            assert!(FluentResource::try_new(src.to_owned()).is_ok(), "{lang} 的 migrate.ftl 无法被 Fluent 解析");
            sets.push(keys(src));
        }
        for (i, (lang, _)) in SOURCES.iter().enumerate().skip(1) {
            assert_eq!(sets[0], sets[i], "{lang} 与 {} 的 migrate.ftl key 集合不一致", SOURCES[0].0);
        }
    }
}

/// 同步逻辑本身（复制 / 备份 / 打标记）不依赖引擎，单独一组测试。
#[cfg(test)]
mod sync_tests {
    use super::*;

    /// 注意：`apply` 走的是 `dir::root()`，也就是「当前工作目录 /data」——
    /// 所以这里把工作目录切到临时目录里造的一套假环境，跑完切回去。
    #[test]
    fn apply_copies_legacy_data() {
        use std::time::SystemTime;

        let tmp = tempfile::tempdir().unwrap();
        let old_cwd = std::env::current_dir().unwrap();

        // 假的「旧版本安装」
        let old_data = tmp.path().join("PhirLie-old/data");
        fs::create_dir_all(old_data.join("charts/custom/demo")).unwrap();
        fs::create_dir_all(old_data.join("collections")).unwrap();
        fs::write(old_data.join("data.json"), b"{\"config\":{},\"charts\":[1,2,3]}").unwrap();
        fs::write(old_data.join("charts/custom/demo/x.json"), b"{}").unwrap();
        fs::write(old_data.join("collections/c.json"), b"{}").unwrap();
        fs::write(old_data.join(ASKED_FILE), b"F:\\somewhere\\else\n").unwrap();

        // 假的「当前版本」目录（apply 会往这里写）
        let cur_root = tmp.path().join("cur");
        let cur_data = cur_root.join("data");
        fs::create_dir_all(cur_data.join("charts/custom/keep")).unwrap();
        fs::write(cur_data.join("data.json"), b"{\"me\":null}").unwrap();
        fs::write(cur_data.join("charts/custom/keep/y.json"), b"{}").unwrap();

        std::env::set_current_dir(&cur_root).unwrap();
        let legacy = Legacy {
            root: old_data.parent().unwrap().to_path_buf(),
            data_dir: old_data.clone(),
            name: "PhirLie-old".to_owned(),
            modified: SystemTime::now(),
            charts: 3,
            logged_in: false,
            source: "test",
        };
        let res = apply(&legacy, &Progress::default());
        std::env::set_current_dir(&old_cwd).unwrap();
        let stats = res.unwrap();

        // 旧 data.json 覆盖过来了（最后一步，且在内存里就是旧那份）
        assert_eq!(fs::read(cur_data.join("data.json")).unwrap(), b"{\"config\":{},\"charts\":[1,2,3]}");
        // 目录是合并的：旧的进来了，新版本自己的东西没被删
        assert!(cur_data.join("charts/custom/demo/x.json").is_file());
        assert!(cur_data.join("charts/custom/keep/y.json").is_file());
        assert!(cur_data.join("collections/c.json").is_file());
        // 备份、两个标记文件
        let backup = stats.backup.clone().expect("should back up data.json");
        assert_eq!(fs::read(cur_data.join(&backup)).unwrap(), b"{\"me\":null}");
        assert!(cur_data.join(SYNCED_FILE).is_file());
        let asked = fs::read_to_string(cur_data.join(ASKED_FILE)).unwrap();
        assert!(asked.contains("somewhere"), "旧安装里的 asked 记录应该被并进来: {asked}");
        assert!(stats.files >= 3, "至少要复制 data.json + 2 个文件: {stats:?}");
    }
}
