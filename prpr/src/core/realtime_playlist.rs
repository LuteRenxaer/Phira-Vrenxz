//! 实时演算背景播放列表
//!
//! 谱面根目录存在 `.cfg` 时优先使用它（不再读取单独的 `.pbfx`）。
//! `.cfg` 是简单的 `key = value` 文本，`#` 或 `//` 开头为注释：
//!
//! ```ini
//! # 引用哪些 .pbfx（相对谱面根目录，可省略扩展名）
//! files = fx-a.pbfx, fx-b.pbfx
//! # 底色（#rrggbb 或 #rgb），指定后会覆盖各 .pbfx 自己的背景色
//! color = #221a33
//! # 切换时选下一个的顺序：sequence（顺序循环，默认） / random（随机）
//! order = sequence
//! # 切换模式：1 = 每隔 N 拍切换；2 = 在指定拍切换
//! mode = 1
//! # 模式 1 写间隔拍数；模式 2 写切换拍号，英文逗号分隔（可带 []）
//! beats = 8
//! ```
//!
//! 模式 2 示例：`beats = 16,32,48` 表示在第 16、32、48 拍切换。
//!
//! 拍号以「音频开始播放 + info.yml 的 offset + 谱面文件 offset」为 0 点换算：
//! 即谱面时间 0（第一拍）对应的音频时刻，而不是音频文件的第 0 秒，
//! 因此谱面前奏/空白段不会被计入拍点。

use super::realtime_background::EFFECT_EXTENSION;
use super::{RealtimeBackground, RealtimeFx};
use anyhow::{ensure, Context, Result};
use macroquad::prelude::get_time;
use std::path::Path;

/// 播放列表配置文件的扩展名（不带点）。
pub const CFG_EXTENSION: &str = "cfg";

/// 判断文件名是否为 `.cfg` 播放列表。
pub fn is_cfg_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|it| it.to_str())
        .is_some_and(|it| it.eq_ignore_ascii_case(CFG_EXTENSION))
}

/// 在谱面根目录文件列表中查找 `.cfg` 配置文件。
pub fn find_cfg_file(files: &[String]) -> Option<String> {
    files.iter().find(|it| is_cfg_file(it)).cloned()
}

/// `.cfg` 的解析结果。
#[derive(Clone, Debug, PartialEq)]
pub struct PlaylistCfg {
    /// 引用的 `.pbfx` 文件名（相对谱面根目录）
    pub files: Vec<String>,
    /// 底色（覆盖各效果的背景色），None 表示各效果用自己的背景
    pub color: Option<[u8; 3]>,
    /// 拍点微调（秒，正数表示延后）
    pub sync_offset: f32,
    /// 切换时是否随机挑选（false = 顺序循环）
    pub random: bool,
    /// 1 = 每隔固定拍数切换；2 = 在指定拍切换
    pub mode: u8,
    /// mode 1 为间隔拍数；mode 2 为切换拍号列表
    pub beats: Vec<f32>,
}

impl PlaylistCfg {
    pub fn parse(text: &str) -> Result<Self> {
        let mut files = Vec::new();
        let mut color = None;
        let mut sync_offset = 0f32;
        let mut random = false;
        let mut mode = 1u8;
        let mut beats = Vec::new();

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "files" | "file" | "effects" | "pbfx" => {
                    for item in value.split(|c: char| c == ',' || c.is_whitespace()) {
                        let item = item.trim().trim_matches('"').trim_matches('\'');
                        if item.is_empty() {
                            continue;
                        }
                        files.push(with_pbfx_extension(item));
                    }
                }
                "order" => {
                    random = matches!(value.to_ascii_lowercase().as_str(), "random" | "rand" | "shuffle" | "随机");
                }
                "color" | "colour" | "bgcolor" => color = parse_hex_color(value),
                "sync" | "syncoffset" | "beatsoffset" => {
                    if let Ok(v) = value.trim().trim_end_matches("ms").parse::<f32>() {
                        // 带 ms 后缀按毫秒处理，否则按秒
                        sync_offset = if value.trim().ends_with("ms") { v / 1000. } else { v };
                    }
                }
                "mode" => {
                    if let Ok(v) = value.parse::<u8>() {
                        mode = v;
                    }
                }
                "beats" => beats.extend(parse_beats(value)),
                _ => {}
            }
        }

        ensure!(!files.is_empty(), ".cfg 没有指定任何 .pbfx（files = ...）");
        ensure!(mode == 1 || mode == 2, ".cfg 的 mode 只能是 1 或 2，实际为 {mode}");
        if beats.is_empty() {
            beats.push(8.);
        }
        if mode == 1 && !(beats[0] > 0.) {
            beats[0] = 8.;
        }

        Ok(Self {
            files,
            color,
            sync_offset: sync_offset.clamp(-2., 2.),
            random,
            mode,
            beats,
        })
    }
}

fn with_pbfx_extension(name: &str) -> String {
    if Path::new(name).extension().is_some() {
        name.to_owned()
    } else {
        format!("{name}.{EFFECT_EXTENSION}")
    }
}

fn parse_beats(value: &str) -> Vec<f32> {
    let cleaned = value.trim().trim_start_matches('[').trim_end_matches(']');
    cleaned
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|it| it.trim().parse::<f32>().ok())
        .collect()
}

/// 解析 `#rrggbb` / `#rgb`（可省略 #）。
fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let text = value.trim().trim_matches('"').trim_matches('\'').trim_start_matches('#');
    if !text.is_ascii() {
        return None;
    }
    let byte = |s: &str| u8::from_str_radix(s, 16).ok();
    match text.len() {
        6 => Some([byte(&text[0..2])?, byte(&text[2..4])?, byte(&text[4..6])?]),
        3 => {
            let expand = |s: &str| byte(s).map(|v| v * 17);
            Some([expand(&text[0..1])?, expand(&text[1..2])?, expand(&text[2..3])?])
        }
        _ => None,
    }
}

/// 用 cfg 里的底色重建背景渐变（中心最亮，向边缘压暗），并关闭色相漂移。
fn apply_base_color(fx: &mut RealtimeFx, rgb: [u8; 3]) {
    let r = rgb[0] as f32 / 255.;
    let g = rgb[1] as f32 / 255.;
    let b = rgb[2] as f32 / 255.;
    fx.background.inner = hex_color(r, g, b);
    fx.background.mid = hex_color(r * 0.5, g * 0.5, b * 0.5);
    fx.background.outer = hex_color(r * 0.18, g * 0.18, b * 0.18);
    fx.background.hue_drift = 0.;
}

fn hex_color(r: f32, g: f32, b: f32) -> String {
    let to = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", to(r), to(g), to(b))
}

/// 拍点切换器（纯逻辑，便于单测）。
///
/// `advance` 返回本次需要执行的切换次数，由播放列表负责真正换效果。
#[derive(Clone, Debug)]
pub struct BeatSwitch {
    mode: u8,
    interval: f64,
    points: Vec<f64>,
    next_switch: f64,
    point_idx: usize,
    last_beat: f64,
    initialized: bool,
}

impl BeatSwitch {
    pub fn new(cfg: &PlaylistCfg) -> Self {
        let interval = cfg.beats.first().copied().unwrap_or(8.).max(0.25) as f64;
        let points = if cfg.mode == 2 {
            let mut pts: Vec<f64> = cfg.beats.iter().map(|&b| b.max(0.) as f64).collect();
            pts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            pts.dedup();
            pts
        } else {
            Vec::new()
        };
        Self {
            mode: cfg.mode,
            interval,
            points,
            next_switch: f64::INFINITY,
            point_idx: 0,
            last_beat: 0.,
            initialized: false,
        }
    }

    /// 推进到给定拍点，返回应执行的切换次数。
    pub fn advance(&mut self, beat: f64) -> usize {
        if !self.initialized {
            self.initialized = true;
            self.reset_at(beat);
            return 0;
        }
        if beat < self.last_beat - 1e-4 {
            // 回退（重开 / 跳转）时重新对齐，不再补发切换
            self.reset_at(beat);
            return 0;
        }
        self.last_beat = beat;

        let mut count = 0;
        if self.mode == 1 {
            let mut guard = 0;
            while beat >= self.next_switch && guard < 64 {
                self.next_switch += self.interval;
                count += 1;
                guard += 1;
            }
            if guard >= 64 {
                self.next_switch = beat + self.interval;
            }
        } else {
            while self.point_idx < self.points.len() && beat >= self.points[self.point_idx] {
                self.point_idx += 1;
                count += 1;
            }
        }
        count
    }

    fn reset_at(&mut self, beat: f64) {
        self.last_beat = beat;
        self.point_idx = 0;
        if self.mode == 1 {
            // 对齐到谱面绝对拍网格（0、interval、2*interval…），
            // 这样初始化发生在第几帧都不影响切换点，也不会出现差一两拍的现象。
            let steps = (beat.max(0.) / self.interval).floor() + 1.;
            self.next_switch = steps * self.interval;
        } else {
            while self.point_idx < self.points.len() && beat > self.points[self.point_idx] {
                self.point_idx += 1;
            }
        }
    }
}

/// 实时背景播放列表：持有多个效果参数，按 `.cfg` 规则切换。
pub struct RealtimePlaylist {
    bg: RealtimeBackground,
    effects: Vec<RealtimeFx>,
    cfg: PlaylistCfg,
    switch: BeatSwitch,
    current: usize,
    rng: u64,
}

impl RealtimePlaylist {
    /// 从已解析的配置与效果创建播放列表。
    pub fn new(cfg: PlaylistCfg, mut effects: Vec<RealtimeFx>, viewport: (i32, i32, i32, i32)) -> Result<Self> {
        ensure!(!effects.is_empty(), "播放列表中没有可用的 .pbfx");
        // cfg 指定了底色时，覆盖所有效果的背景渐变
        if let Some(rgb) = cfg.color {
            for fx in &mut effects {
                apply_base_color(fx, rgb);
            }
        }
        let bg = RealtimeBackground::from_fx(effects[0].clone(), viewport);
        let switch = BeatSwitch::new(&cfg);
        Ok(Self {
            bg,
            effects,
            cfg,
            switch,
            current: 0,
            rng: seed(),
        })
    }

    /// 单效果（谱面只有 `.pbfx` 没有 `.cfg` 时）。
    pub fn single(fx: RealtimeFx, viewport: (i32, i32, i32, i32)) -> Self {
        let cfg = PlaylistCfg {
            files: Vec::new(),
            color: None,
            sync_offset: 0.,
            random: false,
            mode: 1,
            beats: vec![f32::INFINITY],
        };
        let bg = RealtimeBackground::from_fx(fx.clone(), viewport);
        let switch = BeatSwitch::new(&cfg);
        Self {
            bg,
            effects: vec![fx],
            cfg,
            switch,
            current: 0,
            rng: seed(),
        }
    }

    /// 从 `.pbfx` 内容创建单效果播放列表。
    pub fn load_single(data: &[u8], viewport: (i32, i32, i32, i32)) -> Result<Self> {
        let fx: RealtimeFx = serde_json::from_slice(data).context("failed to parse realtime effect file")?;
        Ok(Self::single(fx, viewport))
    }

    pub fn texture(&self) -> crate::ext::SafeTexture {
        self.bg.texture()
    }

    /// 拍点微调（秒），用于个别设备上的音频延迟补偿。
    pub fn sync_offset(&self) -> f64 {
        self.cfg.sync_offset as f64
    }

    /// 当前使用的效果下标（调试用）。
    pub fn current(&self) -> usize {
        self.current
    }

    /// 每帧更新：先按拍点决定是否切换，再渲染背景。
    ///
    /// `time` 为谱面时间（负值表示尚未到拍点起点），`beat` 为对应的谱面拍号（负值表示未开始）。
    pub fn update(&mut self, time: f32, viewport: (i32, i32, i32, i32), beat: f64) {
        if beat < 0. {
            // 音频尚未开始 / 前置空白：只渲染，不推进切换计时
            self.bg.update(time, viewport);
            return;
        }
        let switches = self.switch.advance(beat);
        for _ in 0..switches {
            self.switch_next();
        }
        self.bg.update(time, viewport);
    }

    fn switch_next(&mut self) {
        let n = self.effects.len();
        if n <= 1 {
            return;
        }
        self.current = if self.cfg.random {
            let mut idx = (self.next_u32() as usize) % n;
            if idx == self.current {
                idx = (idx + 1) % n;
            }
            idx
        } else {
            (self.current + 1) % n
        };
        self.bg.set_fx(self.effects[self.current].clone());
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 32) as u32
    }
}

fn seed() -> u64 {
    (get_time() * 1_000_000.) as u64 | 1
}
