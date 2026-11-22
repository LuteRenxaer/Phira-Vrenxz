//! ADOFAI (A Dance of Fire and Ice) chart importer.
//!
//! ADOFAI stores a chart as one JSON file: `settings`, `angleData` / `pathData`
//! and a list of floor actions.  Phira cannot load it directly, so this module
//! converts the path into a pure-config RPE chart: one judge line with one
//! tap note per floor, keeping the rhythm and BPM changes instead of drawing
//! ADOFAI's 2-D tile path.
//!
//! Timing uses ADOFAI's rule that a 180° tile angle is one beat.  `SetSpeed`
//! actions at a floor change the BPM from that beat on, which the generated
//! `BPMList` preserves.

use anyhow::{bail, Context, Result};
use prpr::info::{ChartFormat, ChartInfo};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::path::Path;

#[derive(Default)]
pub struct Converted {
    pub chart_json: String,
    pub info: ChartInfo,
    /// File name from `settings.song`, if it looks like a local path.
    pub song: Option<String>,
    /// File name from `settings.previewImage`, if it looks like a local path.
    pub background: Option<String>,
}

/// Pure-config key layout chosen at import time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyMode {
    K4,
    K5,
    K6,
}

impl KeyMode {
    pub const ALL: [Self; 3] = [Self::K4, Self::K5, Self::K6];

    pub fn keys(self) -> usize {
        match self {
            Self::K4 => 4,
            Self::K5 => 5,
            Self::K6 => 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::K4 => "4K",
            Self::K5 => "5K",
            Self::K6 => "6K",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "4k" | "4K" => Some(Self::K4),
            "5k" | "5K" => Some(Self::K5),
            "6k" | "6K" => Some(Self::K6),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdofaiChart {
    #[serde(default)]
    settings: AdofaiSettings,
    #[serde(default)]
    angle_data: Vec<Option<f64>>,
    #[serde(default)]
    path_data: String,
    #[serde(default, alias = "actions")]
    action: Vec<AdofaiAction>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdofaiSettings {
    #[serde(default)]
    song: String,
    #[serde(default)]
    song_filename: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    author: String,
    #[serde(default = "default_bpm")]
    bpm: f64,
    #[serde(default = "default_speed")]
    speed_multiplier: f64,
    #[serde(default)]
    preview_image: String,
    #[serde(default)]
    song_length: f64,
    #[serde(default)]
    offset: f64,
    #[serde(default)]
    difficulty: Value,
    #[serde(default)]
    level_desc: String,
}

fn default_bpm() -> f64 {
    120.0
}

fn default_speed() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
struct AdofaiAction {
    #[serde(default)]
    floor: usize,
    #[serde(rename = "eventType", default)]
    event_type: String,
    #[serde(flatten)]
    data: Map<String, Value>,
}

fn number(map: &Map<String, Value>, key: &str) -> Option<f64> {
    map.get(key).and_then(Value::as_f64)
}

fn text<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str)
}

fn file_component(path: &str) -> Option<String> {
    if path.trim().is_empty() || path.contains("://") {
        return None;
    }
    Path::new(path)
        .file_name()
        .map(|it| it.to_string_lossy().into_owned())
        .filter(|it| !it.is_empty())
}

fn difficulty_value(value: &Value) -> f32 {
    if let Some(s) = value.as_str() {
        return match s.trim().to_ascii_lowercase().as_str() {
            "easy" => 3.0,
            "normal" => 6.0,
            "hard" => 9.0,
            "harder" => 11.0,
            "insane" => 13.0,
            "turbo" => 15.0,
            _ => 9.0,
        };
    }
    match value.as_f64() {
        Some(1.0) => 3.0,
        Some(2.0) => 6.0,
        Some(3.0) => 9.0,
        Some(4.0) => 11.0,
        Some(5.0) => 13.0,
        Some(6.0) => 15.0,
        _ => 9.0,
    }
}

/// ADOFAI stores an absolute tile direction.  The travel time to the next
/// tile is determined by the relative direction change.
fn relative_beats(current: f64, next: f64, twirled: bool) -> f64 {
    let mut delta = (next - current) % 360.0;
    if delta < 0.0 {
        delta += 360.0;
    }
    if delta.abs() < 1e-6 {
        delta = 360.0;
    }
    if twirled {
        delta = 360.0 - delta;
        if delta.abs() < 1e-6 {
            delta = 360.0;
        }
    }
    delta / 180.0
}

fn is_midspin(angle: f64) -> bool {
    angle == 999.0 || angle == -1.0
}

fn beat_triple(beat: f64) -> Value {
    const DEN: u64 = 180;
    let units = (beat.max(0.0) * DEN as f64).round() as u64;
    json!([units / DEN, units % DEN, DEN])
}
/// `!` (999°) is a midspin and does not count as a separate hit tile.
fn decode_path_data(source: &str) -> Vec<f64> {
    source
        .chars()
        .filter_map(|c| {
            Some(match c {
                'R' => 0.0,
                'p' => 15.0,
                'J' => 30.0,
                'E' => 45.0,
                'T' => 60.0,
                'o' => 75.0,
                'U' => 90.0,
                'q' => 105.0,
                'G' => 120.0,
                'Q' => 135.0,
                'H' => 150.0,
                'W' => 165.0,
                'L' => 180.0,
                'x' => 195.0,
                'N' => 210.0,
                'Z' => 225.0,
                'F' => 240.0,
                'V' => 255.0,
                'D' => 270.0,
                'Y' => 285.0,
                'B' => 300.0,
                'C' => 315.0,
                'M' => 330.0,
                'A' => 345.0,
                '5' => 555.0,
                '6' => 666.0,
                '7' => 777.0,
                '8' => 888.0,
                '!' => 999.0,
                _ => return None,
            })
        })
        .collect()
}

fn angles(chart: &AdofaiChart) -> Vec<f64> {
    // `null` still marks a floor; `999` is ADOFAI's midspin marker.  Both are
    // kept here so action floor indices stay aligned; `convert` then removes
    // midspins from the note list (they are part of the previous tile).
    if chart.angle_data.is_empty() {
        decode_path_data(&chart.path_data)
    } else {
        chart.angle_data.iter().map(|it| it.unwrap_or(999.0)).collect()
    }
}

/// Places every floor at its 2-D center on the ADOFAI tile path.  ADOFAI
/// angles are counter-clockwise with `0` = right and `90` = up.
fn floor_centers(angles: &[f64]) -> Vec<(f64, f64)> {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut out = Vec::with_capacity(angles.len());
    for angle in angles {
        let rad = angle.to_radians();
        let (sx, cx) = rad.sin_cos();
        out.push((x + cx * 0.5, y + sx * 0.5));
        x += cx;
        y += sx;
    }
    fit_centers(out)
}

/// Fit the path into the visible Phira world.  Uses one uniform scale so the
/// ADOFAI path keeps its shape instead of being stretched to 16:9.
fn fit_centers(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if points.is_empty() {
        return points;
    }
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in &points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let w = (max_x - min_x).max(1e-6);
    let h = (max_y - min_y).max(1e-6);
    // World x is [-1, 1]; world y is roughly [-0.5625, 0.5625] at 16:9.
    let scale = (1.85 / w).min(1.05 / h);
    let cx = (min_x + max_x) / 2.0;
    let cy = (min_y + max_y) / 2.0;
    for (x, y) in &mut points {
        *x = (*x - cx) * scale;
        *y = (*y - cy) * scale;
    }
    points
}

/// Build the BPM timeline.  Every floor is one beat; `SetSpeed` at floor N
/// changes the BPM starting with the tile at floor N.
fn bpm_timeline(
    settings: &AdofaiSettings,
    actions: &[AdofaiAction],
    floor_to_beat: &[f64],
    end_beat: f64,
) -> Vec<(f64, f64)> {
    let base = if settings.bpm.is_finite() && settings.bpm > 0.0 {
        settings.bpm
    } else {
        120.0
    };
    let speed = if settings.speed_multiplier.is_finite() && settings.speed_multiplier > 0.0 {
        settings.speed_multiplier
    } else {
        1.0
    };
    let mut bpm = base * speed;
    let mut changes: Vec<(f64, f64)> = vec![(0.0, bpm)];
    let mut sorted: Vec<&AdofaiAction> = actions.iter().collect();
    sorted.sort_by_key(|it| it.floor);
    for action in sorted {
        if action.event_type != "SetSpeed" {
            continue;
        }
        let mult = number(&action.data, "bpmMultiplier")
            .filter(|it| it.is_finite() && *it > 0.0)
            .unwrap_or(1.0);
        let speed_type = text(&action.data, "speedType").unwrap_or_default();
        let new_bpm = match speed_type {
            // `bpmMultiplier` is an editor artifact for BPM-type events (it
            // can be 1e-7); the target BPM itself is what matters.
            "Bpm" | "BPM" => number(&action.data, "beatsPerMinute")
                .filter(|it| it.is_finite() && *it > 0.0)
                .map(|it| it * speed),
            _ => number(&action.data, "beatsPerMinute")
                .filter(|it| it.is_finite() && *it > 0.0)
                .map(|it| it * mult * speed)
                .or_else(|| {
                    number(&action.data, "speed")
                        .filter(|it| it.is_finite() && *it > 0.0)
                        .map(|it| bpm * it)
                })
                .or_else(|| Some(bpm * mult)),
        };
        if let Some(new_bpm) = new_bpm {
            bpm = new_bpm.clamp(1.0, 100_000.0);
            let beat = floor_to_beat.get(action.floor).copied().unwrap_or(end_beat);
            changes.push((beat, bpm));
        }
    }
    // Last change at a beat wins.
    changes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut deduped: Vec<(f64, f64)> = Vec::with_capacity(changes.len());
    for (beat, bpm) in changes {
        if let Some(last) = deduped.last_mut() {
            if (last.0 - beat).abs() < 1e-9 {
                last.1 = bpm;
                continue;
            }
        }
        deduped.push((beat, bpm));
    }
    deduped
}

pub fn convert(source: &str, fallback_name: &str) -> Result<Converted> {
    convert_with_key_mode(source, fallback_name, KeyMode::K4)
}

pub fn convert_with_key_mode(source: &str, fallback_name: &str, key_mode: KeyMode) -> Result<Converted> {
    let source = source.trim_start_matches('\u{feff}');
    let chart: AdofaiChart = serde_json::from_str(source).context("invalid ADOFAI JSON")?;
    let raw_angles = angles(&chart);
    if raw_angles.is_empty() {
        bail!("ADOFAI chart has no angleData/pathData floors");
    }
    let mut tile_angles = Vec::new();
    let mut tile_twirl = Vec::new();
    let mut twirl_floors: Vec<usize> = chart
        .action
        .iter()
        .filter(|it| it.event_type == "Twirl")
        .map(|it| it.floor)
        .collect();
    twirl_floors.sort_unstable();
    let mut twirl = false;
    let mut twirl_cursor = 0usize;
    for (floor, angle) in raw_angles.iter().enumerate() {
        while twirl_cursor < twirl_floors.len() && twirl_floors[twirl_cursor] <= floor {
            twirl = !twirl;
            twirl_cursor += 1;
        }
        if !is_midspin(*angle) {
            tile_angles.push(*angle);
            tile_twirl.push(twirl);
        }
    }
    if tile_angles.is_empty() {
        bail!("ADOFAI chart has no playable tiles");
    }

    // `angleData` holds absolute tile directions, not the rotation to the
    // next tile.  The beat at tile i is the sum of the relative angles of the
    // intervals before it.
    let mut tile_beats = Vec::with_capacity(tile_angles.len());
    let mut beat = 0.0f64;
    for i in 0..tile_angles.len() {
        tile_beats.push(beat);
        if i + 1 < tile_angles.len() {
            beat += relative_beats(tile_angles[i], tile_angles[i + 1], tile_twirl[i]);
        }
    }
    let mut floor_to_beat = vec![0.0f64; raw_angles.len() + 1];
    let mut actual_floor = 0usize;
    for (floor, angle) in raw_angles.iter().enumerate() {
        if is_midspin(*angle) {
            // A midspin marker belongs to the tile before it, so events at
            // this raw index happen at that tile's time.
            floor_to_beat[floor] = actual_floor
                .checked_sub(1)
                .and_then(|i| tile_beats.get(i))
                .copied()
                .unwrap_or(0.0);
        } else {
            floor_to_beat[floor] = tile_beats[actual_floor];
            actual_floor += 1;
        }
    }
    floor_to_beat[raw_angles.len()] = beat;
    let centers = floor_centers(&tile_angles);
    // Pure-config chart: one judge line, `key_mode.keys()` fixed lanes.
    // The horizontal component of the ADOFAI path picks the lane when the
    // path actually moves sideways; a vertical path falls back to a
    // round-robin lane pattern so it does not collapse into one column.
    let key_count = key_mode.keys();
    let x_min = centers.iter().map(|&(x, _)| x).fold(f64::INFINITY, f64::min);
    let x_max = centers.iter().map(|&(x, _)| x).fold(f64::NEG_INFINITY, f64::max);
    let x_span = x_max - x_min;
    let lane_pos = |lane: usize| -0.9 + 1.8 * lane as f64 / (key_count - 1) as f64;
    let mut lanes = Vec::with_capacity(centers.len());
    let mut last_lane: Option<usize> = None;
    for (i, &(x, _)) in centers.iter().enumerate() {
        let nx = if x_span > 1e-9 { ((x - x_min) / x_span).clamp(0.0, 1.0) } else { 0.0 };
        let mut lane = if x_span > 0.02 {
            ((nx * key_count as f64).floor() as usize).min(key_count - 1)
        } else {
            i % key_count
        };
        // Avoid two consecutive notes in the same lane; a pure-config chart
        // still needs readable separation between taps.
        if last_lane == Some(lane) {
            lane = if lane == 0 {
                1
            } else if lane == key_count - 1 {
                key_count - 2
            } else if i % 2 == 0 {
                lane - 1
            } else {
                lane + 1
            };
        }
        lanes.push(lane);
        last_lane = Some(lane);
    }
    let bpms = bpm_timeline(&chart.settings, &chart.action, &floor_to_beat, beat);

    let song = file_component(&chart.settings.song_filename)
        .or_else(|| file_component(&chart.settings.song));
    let background = file_component(&chart.settings.preview_image);
    let title = song
        .as_deref()
        .and_then(|it| Path::new(it).file_stem())
        .and_then(|it| it.to_str())
        .filter(|it| !it.is_empty())
        .unwrap_or(fallback_name)
        .to_owned();

    let diff = difficulty_value(&chart.settings.difficulty);
    let mut info = ChartInfo {
        name: title.clone(),
        difficulty: diff,
        level: if chart.settings.level_desc.trim().is_empty() {
            format!("ADOFAI Lv.{diff:.0}")
        } else {
            chart.settings.level_desc.clone()
        },
        charter: if chart.settings.author.trim().is_empty() {
            "ADOFAI".to_owned()
        } else {
            chart.settings.author.clone()
        },
        composer: chart.settings.artist.clone(),
        illustrator: chart.settings.artist.clone(),
        chart: "chart.json".to_owned(),
        format: Some(ChartFormat::Rpe),
        music: song.clone().unwrap_or_else(|| "song.mp3".to_owned()),
        illustration: background.clone().unwrap_or_else(|| "background.png".to_owned()),
        preview_start: 0.0,
        preview_end: (chart.settings.song_length > 0.0).then_some(chart.settings.song_length as f32),
        offset: if chart.settings.offset.is_finite() { chart.settings.offset as f32 / 1000.0 } else { 0.0 },
        tags: vec!["ADOFAI".to_owned(), format!("{}K", key_count)],
        ..Default::default()
    };
    // Keep metadata consistent even when the caller writes info.yml directly.
    info.name = title;

    let notes: Vec<Value> = centers
        .iter()
        .enumerate()
        .map(|(i, &_center)| {
            let time = beat_triple(tile_beats[i]);
            json!({
                "type": 1,
                "above": 1,
                "startTime": time.clone(),
                "endTime": time,
                // Pure-config chart: one judge line, one fixed lane per note.
                "positionX": lane_pos(lanes[i]) * 675.0,
                "yOffset": 0.0,
                "alpha": 255,
                "hitsound": null,
                "size": 1.0,
                "speed": 1.0,
                "isFake": 0,
                "visibleTime": 2.0,
                "tint": null,
                "tintHitEffects": null,
                "judgeArea": null
            })
        })
        .collect();

    let bpm_list: Vec<Value> = bpms
        .into_iter()
        .map(|(beat, bpm)| {
            json!({
                "bpm": bpm,
                "startTime": beat_triple(beat)
            })
        })
        .collect();
    let chart_json = serde_json::to_string_pretty(&json!({
        "META": {
            "offset": 0,
            "RPEVersion": 170
        },
        "BPMList": bpm_list,
        "judgeLineList": [{
            "Name": "ADOFAI",
            "Texture": "line.png",
            "father": null,
            "rotateWithFather": false,
            "eventLayers": [{}],
            "extended": null,
            "notes": notes,
            "isCover": 0,
            "zOrder": 0,
            "attachUI": null,
            "posControl": [],
            "sizeControl": [],
            "alphaControl": [{ "x": 0.0, "easing": 1, "alpha": 1.0 }],
            "yControl": [],
            "bpmfactor": 1.0,
            "anchor": null
        }]
    }))?;

    Ok(Converted {
        chart_json,
        info,
        song,
        background,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "settings": {
            "song": "song.mp3",
            "artist": "Artist",
            "author": "Charter",
            "bpm": 120,
            "speedMultiplier": 1.0,
            "previewImage": "bg.png",
            "songLength": 42.0,
            "difficulty": "Hard"
        },
        "angleData": [0, 90, 180, 270, 0, 90, 180, 270],
        "actions": [
            {"floor": 3, "eventType": "SetSpeed", "speedType": "Multiplier", "bpmMultiplier": 2.0}
        ]
    }"#;

    #[test]
    fn converts_floors_and_bpm_changes() {
        let out = convert(SAMPLE, "fallback").unwrap();
        assert_eq!(out.info.name, "song");
        assert_eq!(out.info.music, "song.mp3");
        assert_eq!(out.info.illustration, "bg.png");
        assert_eq!(out.song.as_deref(), Some("song.mp3"));

        let value: Value = serde_json::from_str(&out.chart_json).unwrap();
        let notes = value["judgeLineList"][0]["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 8);
        for note in notes {
            assert!(note["positionX"].as_f64().unwrap().is_finite());
            assert!(note["yOffset"].as_f64().unwrap().is_finite());
        }
        let bpm = value["BPMList"].as_array().unwrap();
        assert_eq!(bpm.len(), 2);
        assert_eq!(bpm[0]["bpm"].as_f64(), Some(120.0));
        assert_eq!(bpm[1]["bpm"].as_f64(), Some(240.0));
        assert_eq!(bpm[1]["startTime"][0].as_u64(), Some(1));
        assert_eq!(bpm[1]["startTime"][1].as_u64(), Some(90));
    }

    #[test]
    fn decodes_path_data() {
        let source = r#"{
            "settings": {"song": "x.mp3"},
            "pathData": "RD"
        }"#;
        let out = convert(source, "x").unwrap();
        let value: Value = serde_json::from_str(&out.chart_json).unwrap();
        assert_eq!(value["judgeLineList"][0]["notes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn handles_midspins_and_numeric_difficulty() {
        let source = r#"{
            "settings": {"songFilename": "Remember Me.ogg", "difficulty": 1, "bpm": 131},
            "angleData": [0, 0, 90, 180, 999, 0, 0, 0, 90, 180, 999, 0, 135, 999, 0, 135, 999, 0, 90, 180, 999, 0],
            "actions": [
                {"floor": 1, "eventType": "SetSpeed", "speedType": "Bpm", "beatsPerMinute": 32.75, "bpmMultiplier": 1e-7},
                {"floor": 2, "eventType": "SetSpeed", "speedType": "Multiplier", "bpmMultiplier": 4}
            ]
        }"#;
        let out = convert(source, "fallback").unwrap();
        assert_eq!(out.info.music, "Remember Me.ogg");
        let value: Value = serde_json::from_str(&out.chart_json).unwrap();
        assert_eq!(value["judgeLineList"][0]["notes"].as_array().unwrap().len(), 17);
        assert_eq!(value["BPMList"].as_array().unwrap().len(), 3);
    }
}
