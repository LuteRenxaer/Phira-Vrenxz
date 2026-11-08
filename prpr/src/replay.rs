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
#[serde(default)]
pub struct Replay {
    pub version: u32,
    pub name: String,
    pub chart: String,
    #[serde(default)]
    pub local_path: Option<String>,
    pub speed: f32,
    pub recorded_at: i64,
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
    serde_json::from_slice(&data).ok()
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
        version: 1,
        name: name.to_owned(),
        chart: chart.to_owned(),
        local_path: NEXT_LOCAL_PATH.lock().unwrap().clone(),
        speed,
        recorded_at: chrono::Utc::now().timestamp(),
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
pub fn finish() -> Option<PathBuf> {
    let replay = ACTIVE.lock().unwrap().take()?;
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

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .take(40)
        .collect()
}
