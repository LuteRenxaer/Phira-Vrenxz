//! 轻量本地回放录制：保存触点时间轴，供后续回放/PRCA 使用。
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TouchFrame {
    pub t: f32,
    pub id: u64,
    pub phase: u8,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HitFrame {
    pub t: f32,
    pub kind: u8,
    pub diff: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReplayResult {
    pub score: u32,
    pub accuracy: f64,
    pub max_combo: u32,
    pub counts: [u32; 4],
    pub full_combo: bool,
    pub early: u32,
    pub late: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Replay {
    pub header: String,
    pub version: u32,
    pub name: String,
    pub player: Option<String>,
    pub chart: String,
    #[serde(default)]
    pub local_path: Option<String>,
    pub speed: f32,
    pub offset: f32,
    pub mods: u32,
    pub duration: f32,
    pub recorded_at: i64,
    pub result: Option<ReplayResult>,
    pub touches: Vec<TouchFrame>,
    pub hits: Vec<HitFrame>,
    #[serde(skip)]
    pub seen_hits: HashSet<(u64, u8, u32)>,
}

static ACTIVE: Mutex<Option<Replay>> = Mutex::new(None);
static PENDING: Mutex<Option<Replay>> = Mutex::new(None);
static NEXT_LOCAL_PATH: Mutex<Option<String>> = Mutex::new(None);

pub fn set_chart_path(path: &str) {
    *NEXT_LOCAL_PATH.lock().unwrap() = Some(path.to_owned());
}

pub fn chart_path() -> Option<String> {
    NEXT_LOCAL_PATH.lock().unwrap().clone()
}

pub fn set_pending(replay: Replay) {
    *PENDING.lock().unwrap() = Some(replay);
}

pub fn take_pending() -> Option<Replay> {
    PENDING.lock().unwrap().take()
}

pub fn load(path: &std::path::Path) -> Option<Replay> {
    let data = std::fs::read(path).ok()?;
    let mut replay: Replay = serde_json::from_slice(&data).ok()?;
    if replay.header.is_empty() {
        replay.header = "PhiraVrenxzReplay".into();
    }
    if replay.version < 2 {
        replay.version = 2;
    }
    if replay.duration <= 0. {
        replay.duration = replay.touches.iter().map(|t| t.t).fold(0., f32::max);
    }
    Some(replay)
}

/// Find the newest replay recorded for a chart display name.
pub fn latest_for_name(name: &str) -> Option<Replay> {
    latest_path_for_name(name).and_then(|path| load(&path))
}

pub fn latest_path_for_name(name: &str) -> Option<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir("replays")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    for path in files.into_iter().rev() {
        if let Some(replay) = load(&path) {
            if replay.name == name && !replay.touches.is_empty() {
                return Some(path);
            }
        }
    }
    None
}

pub fn delete_latest(name: &str) -> bool {
    latest_path_for_name(name)
        .is_some_and(|path| std::fs::remove_file(path).is_ok())
}

impl Replay {
    pub fn stats(&self) -> (u32, u32, u32, u32, f32) {
        let mut counts = [0u32; 4];
        for h in &self.hits {
            counts[(h.kind as usize).min(3)] += 1;
        }
        let total = counts.iter().sum::<u32>().max(1);
        let acc = (counts[0] as f32 + 0.65 * counts[1] as f32) / total as f32;
        (counts[0], counts[1], counts[2], counts[3], acc)
    }

    pub fn early_late(&self) -> (u32, u32) {
        let early = self.hits.iter().filter(|h| h.diff < 0.).count() as u32;
        (early, self.hits.len() as u32 - early)
    }

    pub fn recommended_offset_ms(&self) -> f32 {
        if self.hits.is_empty() {
            return 0.;
        }
        self.hits.iter().map(|h| h.diff).sum::<f32>() / self.hits.len() as f32 * 1000.
    }

    pub fn active_touches(&self, time: f32, window: f32) -> Vec<TouchFrame> {
        // Keep the latest frame per finger near the current time.
        let mut map = std::collections::HashMap::new();
        for f in &self.touches {
            if f.t > time + 0.03 {
                break;
            }
            if f.t >= time - window && f.phase != 2 && f.phase != 3 {
                map.insert(f.id, *f);
            }
        }
        map.into_values().collect()
    }
}

pub fn begin(name: &str, chart: &str, speed: f32) {
    *ACTIVE.lock().unwrap() = Some(Replay {
        header: "PhiraVrenxzReplay".into(),
        version: 2,
        name: name.to_owned(),
        player: None,
        chart: chart.to_owned(),
        local_path: NEXT_LOCAL_PATH.lock().unwrap().clone(),
        speed,
        offset: 0.,
        mods: 0,
        duration: 0.,
        recorded_at: chrono::Utc::now().timestamp(),
        result: None,
        touches: Vec::new(),
        hits: Vec::new(),
        seen_hits: HashSet::new(),
    });
}

pub fn capture_hits(hits: &[(f32, u8, f32)]) {
    if let Some(replay) = ACTIVE.lock().unwrap().as_mut() {
        for &(t, kind, diff) in hits {
            let key = (t.to_bits() as u64, kind, diff.to_bits());
            if replay.seen_hits.insert(key) {
                replay.hits.push(HitFrame { t, kind, diff });
            }
        }
    }
}

pub fn capture(touches: &[(u64, u8, f32, f32)], t: f32) {
    if let Some(replay) = ACTIVE.lock().unwrap().as_mut() {
        replay.touches.extend(touches.iter().map(|&(id, phase, x, y)| TouchFrame {
            t,
            id,
            phase,
            x,
            y,
        }));
    }
}

pub fn is_recording() -> bool {
    ACTIVE.lock().unwrap().is_some()
}

/// 结束并写入 `replays/`；返回保存路径。
pub fn finish(result: Option<ReplayResult>) -> Option<PathBuf> {
    let mut replay = ACTIVE.lock().unwrap().take()?;
    replay.result = result;
    replay.duration = replay
        .touches
        .iter()
        .map(|t| t.t)
        .chain(replay.hits.iter().map(|h| h.t))
        .fold(0., f32::max);
    if replay.touches.is_empty() {
        return None;
    }
    let dir = PathBuf::from("replays");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!(
        "{}_{}.json",
        chrono::Local::now().format("%Y%m%d_%H%M%S"),
        sanitize(&replay.name)
    ));
    let json = serde_json::to_vec_pretty(&replay).ok()?;
    std::fs::write(&path, json).ok()?;
    Some(path)
}

pub fn cancel() {
    *ACTIVE.lock().unwrap() = None;
}

impl Replay {
    pub fn save_as(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    pub fn trim(&mut self, start: f32, end: f32) {
        let start = start.max(0.);
        let end = end.max(start);
        self.touches.retain(|t| t.t >= start && t.t <= end);
        self.hits.retain(|h| h.t >= start && h.t <= end);
        for t in &mut self.touches {
            t.t -= start;
        }
        for h in &mut self.hits {
            h.t -= start;
        }
        self.duration = (end - start).max(0.);
    }

    pub fn shift(&mut self, delta: f32) {
        for t in &mut self.touches {
            t.t = (t.t + delta).max(0.);
        }
        for h in &mut self.hits {
            h.t = (h.t + delta).max(0.);
        }
        self.duration = (self.duration + delta).max(0.);
    }

    pub fn delete_range(&mut self, start: f32, end: f32) {
        self.touches.retain(|t| t.t < start || t.t > end);
        self.hits.retain(|h| h.t < start || h.t > end);
    }

    pub fn rename(&mut self, name: String) {
        if !name.trim().is_empty() {
            self.name = name;
        }
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .take(40)
        .collect()
}
