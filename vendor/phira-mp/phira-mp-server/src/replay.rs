//! Replay recording module for phira-mp-server
//! 
//! Implements JPhiraRec V1 format (.phirarec files)
//! Reference: JPHIRA_RECORD_FORMAT_V2.md

use anyhow::{Context, Result};
use bytes::{BufMut, BytesMut};
use half::f16;
use phira_mp_common::{JudgeEvent, TouchFrame};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

/// Upload response from replay server
#[derive(Debug, serde::Deserialize)]
pub struct UploadResponse {
    pub success: bool,
    pub message: String,
    #[serde(deserialize_with = "deserialize_replay_id")]
    pub replay_id: Option<String>,
}

/// Custom deserializer for replay_id that handles both String and i64
fn deserialize_replay_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct ReplayIdVisitor;

    impl<'de> serde::de::Visitor<'de> for ReplayIdVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or integer replay_id")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(ReplayIdVisitor)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(value.to_string()))
        }
    }

    deserializer.deserialize_option(ReplayIdVisitor)
}

/// Upload a phirarec file to the replay server
pub async fn upload_replay(
    file_path: &PathBuf,
    api_url: &str,
    api_token: &str,
    chart_name: Option<&str>,
    username: Option<&str>,
) -> Result<UploadResponse> {
    // Read file content
    let file_content = fs::read(file_path).await
        .with_context(|| format!("Failed to read replay file: {}", file_path.display()))?;
    
    // Build multipart form
    let boundary = format!("----WebKitFormBoundary{}", rand::random::<u64>());
    
    let mut form_data = Vec::new();
    
    // Add file field
    form_data.extend_from_slice(format!("--{}\r
", boundary).as_bytes());
    form_data.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"replay.phirarec\"\r
");
    form_data.extend_from_slice(b"Content-Type: application/octet-stream\r
\r
");
    form_data.extend_from_slice(&file_content);
    form_data.extend_from_slice(b"\r
");
    
    // Add chart_name field if provided
    if let Some(name) = chart_name {
        form_data.extend_from_slice(format!("--{}\r
", boundary).as_bytes());
        form_data.extend_from_slice(format!("Content-Disposition: form-data; name=\"chart_name\"\r
\r
{}\r
", name).as_bytes());
    }
    
    // Add username field if provided
    if let Some(name) = username {
        form_data.extend_from_slice(format!("--{}\r
", boundary).as_bytes());
        form_data.extend_from_slice(format!("Content-Disposition: form-data; name=\"username\"\r
\r
{}\r
", name).as_bytes());
    }
    
    // End boundary
    form_data.extend_from_slice(format!("--{}--\r
", boundary).as_bytes());
    
    // Send HTTP POST request
    let client = reqwest::Client::new();
    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_token))
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .body(form_data)
        .send()
        .await
        .with_context(|| "Failed to send upload request")?;
    
    let status = response.status();
    let response_text = response.text().await
        .with_context(|| "Failed to read response body")?;
    
    if !status.is_success() {
        anyhow::bail!("Upload failed with status {}: {}", status, response_text);
    }
    
    // Parse response
    let upload_response: UploadResponse = serde_json::from_str(&response_text)
        .with_context(|| format!("Failed to parse upload response: {}", response_text))?;
    
    Ok(upload_response)
}

/// Magic header for JPhiraRec V1 format
const PHIRAREC_MAGIC: &[u8] = b"PHIRAREC";

/// Format version for JPhiraRec V1
const PHIRAREC_VERSION: i32 = 1;

/// Compression type: ZSTD
const COMPRESSION_ZSTD: u8 = 0x01;

/// Judgement enum mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum JudgementType {
    Perfect = 0x00,
    Good = 0x01,
    Bad = 0x02,
    Miss = 0x03,
    HoldPerfect = 0x04,
    HoldGood = 0x05,
}

impl JudgementType {
    pub fn from_phira(judgement: &phira_mp_common::Judgement) -> Self {
        use phira_mp_common::Judgement;
        match judgement {
            Judgement::Perfect => JudgementType::Perfect,
            Judgement::Good => JudgementType::Good,
            Judgement::Bad => JudgementType::Bad,
            Judgement::Miss => JudgementType::Miss,
            Judgement::HoldPerfect => JudgementType::HoldPerfect,
            Judgement::HoldGood => JudgementType::HoldGood,
        }
    }
}

/// Touch point for replay
#[derive(Debug, Clone)]
pub struct ReplayTouchPoint {
    pub id: u8,
    pub x: f32, // 0.0 ~ 1.0
    pub y: f32, // 0.0 ~ 1.0
}

/// Touch frame for replay
#[derive(Debug, Clone)]
pub struct ReplayTouchFrame {
    pub time: f32, // seconds
    pub points: Vec<ReplayTouchPoint>,
}

impl From<&TouchFrame> for ReplayTouchFrame {
    fn from(frame: &TouchFrame) -> Self {
        Self {
            time: frame.time,
            points: frame
                .points
                .iter()
                .map(|(id, pos)| ReplayTouchPoint {
                    id: *id as u8,
                    x: pos.x(),
                    y: pos.y(),
                })
                .collect(),
        }
    }
}

/// Judge event for replay
#[derive(Debug, Clone)]
pub struct ReplayJudgeEvent {
    pub time: f32,    // seconds
    pub line_id: i32,
    pub note_id: i32,
    pub judgement: JudgementType,
}

impl From<&JudgeEvent> for ReplayJudgeEvent {
    fn from(event: &JudgeEvent) -> Self {
        Self {
            time: event.time,
            line_id: event.line_id,
            note_id: event.note_id,
            judgement: JudgementType::from_phira(&event.judgement),
        }
    }
}

/// Per-player replay cache
#[derive(Debug, Default)]
pub struct PlayerReplayCache {
    pub user_id: i32,
    pub user_name: String,
    pub touch_frames: Vec<ReplayTouchFrame>,
    pub judge_events: Vec<ReplayJudgeEvent>,
}

impl PlayerReplayCache {
    pub fn new(user_id: i32, user_name: String) -> Self {
        Self {
            user_id,
            user_name,
            touch_frames: Vec::new(),
            judge_events: Vec::new(),
        }
    }

    pub fn add_touch_frame(&mut self, frame: ReplayTouchFrame) {
        self.touch_frames.push(frame);
    }

    pub fn add_judge_event(&mut self, event: ReplayJudgeEvent) {
        self.judge_events.push(event);
    }
}

/// Room replay manager - manages replay data for all players in a room
#[derive(Debug)]
pub struct RoomReplayManager {
    pub room_id: String,
    pub chart_id: i32,
    pub chart_name: String,
    pub player_caches: HashMap<i32, PlayerReplayCache>,
    pub is_recording: bool,
}

impl RoomReplayManager {
    pub fn new(room_id: String, chart_id: i32, chart_name: String) -> Self {
        Self {
            room_id,
            chart_id,
            chart_name,
            player_caches: HashMap::new(),
            is_recording: false,
        }
    }

    pub fn start_recording(&mut self) {
        self.is_recording = true;
        info!(room_id = %self.room_id, "Started replay recording");
    }

    pub fn stop_recording(&mut self) {
        self.is_recording = false;
        info!(room_id = %self.room_id, "Stopped replay recording");
    }

    pub fn init_player(&mut self, user_id: i32, user_name: String) {
        self.player_caches
            .insert(user_id, PlayerReplayCache::new(user_id, user_name));
        debug!(user_id, "Initialized player replay cache");
    }

    pub fn record_touch_frames(&mut self, user_id: i32, frames: &[TouchFrame]) {
        // 自动开始录制（当收到第一个事件时）
        if !self.is_recording && !frames.is_empty() {
            self.start_recording();
        }

        if !self.is_recording {
            return;
        }

        // 跳过录制器bot本身的事件
        if user_id == RECORDER_BOT_USER_ID {
            return;
        }

        if let Some(cache) = self.player_caches.get_mut(&user_id) {
            for frame in frames {
                cache.add_touch_frame(ReplayTouchFrame::from(frame));
            }
        }
    }

    pub fn record_judge_events(&mut self, user_id: i32, events: &[JudgeEvent]) {
        // 自动开始录制（当收到第一个事件时）
        if !self.is_recording && !events.is_empty() {
            self.start_recording();
        }

        if !self.is_recording {
            return;
        }

        // 跳过录制器bot本身的事件
        if user_id == RECORDER_BOT_USER_ID {
            return;
        }

        if let Some(cache) = self.player_caches.get_mut(&user_id) {
            for event in events {
                cache.add_judge_event(ReplayJudgeEvent::from(event));
            }
        }
    }

    /// Generate and save phirarec files for all players
    pub async fn save_replays(&self) -> Result<Vec<PathBuf>> {
        let mut saved_files = Vec::new();
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        info!(
            room_id = %self.room_id,
            chart_id = self.chart_id,
            chart_name = %self.chart_name,
            player_count = self.player_caches.len(),
            "Starting to save replays"
        );

        for (user_id, cache) in &self.player_caches {
            info!(user_id = *user_id, touch_count = cache.touch_frames.len(), judge_count = cache.judge_events.len(), "Processing player cache");
            
            // Skip recorder bot
            if *user_id < 0 {
                info!(user_id = *user_id, "Skipping recorder bot");
                continue;
            }

            let record_id = rand::random::<i32>().abs();
            
            let phira_record = PhiraRecord {
                id: record_id,
                time: timestamp,
                chart: self.chart_id,
                chart_name: self.chart_name.clone(),
                user: *user_id,
                user_name: cache.user_name.clone(),
                touch_frames: cache.touch_frames.clone(),
                judge_events: cache.judge_events.clone(),
            };

            let file_path = Self::get_record_path(*user_id, self.chart_id, timestamp);
            
            // Ensure directory exists
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Encode and save
            let encoded = phira_record.encode()?;
            let mut file = fs::File::create(&file_path).await?;
            file.write_all(&encoded).await?;
            file.flush().await?;

            info!(
                user_id = *user_id,
                path = %file_path.display(),
                touch_count = cache.touch_frames.len(),
                judge_count = cache.judge_events.len(),
                "Saved replay recording"
            );

            saved_files.push(file_path);
        }

        Ok(saved_files)
    }

    /// Generate, save and upload phirarec files for all players
    pub async fn save_and_upload_replays(
        &self,
        api_url: &str,
        api_token: &str,
    ) -> Result<Vec<(PathBuf, Option<UploadResponse>)>> {
        let mut results = Vec::new();
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        info!(
            room_id = %self.room_id,
            chart_id = self.chart_id,
            chart_name = %self.chart_name,
            player_count = self.player_caches.len(),
            "Starting to save and upload replays"
        );

        for (user_id, cache) in &self.player_caches {
            info!(user_id = *user_id, touch_count = cache.touch_frames.len(), judge_count = cache.judge_events.len(), "Processing player cache");
            
            // Skip recorder bot
            if *user_id < 0 {
                info!(user_id = *user_id, "Skipping recorder bot");
                continue;
            }

            let record_id = rand::random::<i32>().abs();
            
            let phira_record = PhiraRecord {
                id: record_id,
                time: timestamp,
                chart: self.chart_id,
                chart_name: self.chart_name.clone(),
                user: *user_id,
                user_name: cache.user_name.clone(),
                touch_frames: cache.touch_frames.clone(),
                judge_events: cache.judge_events.clone(),
            };

            let file_path = Self::get_record_path(*user_id, self.chart_id, timestamp);
            
            // Ensure directory exists
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Encode and save
            let encoded = phira_record.encode()?;
            let mut file = fs::File::create(&file_path).await?;
            file.write_all(&encoded).await?;
            file.flush().await?;

            info!(
                user_id = *user_id,
                path = %file_path.display(),
                touch_count = cache.touch_frames.len(),
                judge_count = cache.judge_events.len(),
                "Saved replay recording"
            );

            // Upload the replay file
            let upload_result = upload_replay(
                &file_path,
                api_url,
                api_token,
                Some(&self.chart_name),
                Some(&cache.user_name),
            ).await;

            match upload_result {
                Ok(response) => {
                    if response.success {
                        info!(
                            user_id = *user_id,
                            replay_id = ?response.replay_id,
                            "Successfully uploaded replay"
                        );
                    } else {
                        warn!(
                            user_id = *user_id,
                            message = %response.message,
                            "Upload returned failure"
                        );
                    }
                    results.push((file_path, Some(response)));
                }
                Err(e) => {
                    error!(
                        user_id = *user_id,
                        error = %e,
                        "Failed to upload replay"
                    );
                    results.push((file_path, None));
                }
            }
        }

        Ok(results)
    }

    fn get_record_path(user_id: i32, chart_id: i32, timestamp: u64) -> PathBuf {
        PathBuf::from(format!("record/{}/{}/{}.phirarec", user_id, chart_id, timestamp))
    }
}

/// PhiraRecord structure matching JPhiraRec V1 format
#[derive(Debug)]
pub struct PhiraRecord {
    pub id: i32,
    pub time: u64, // Unix timestamp in milliseconds
    pub chart: i32,
    pub chart_name: String,
    pub user: i32,
    pub user_name: String,
    pub touch_frames: Vec<ReplayTouchFrame>,
    pub judge_events: Vec<ReplayJudgeEvent>,
}

impl PhiraRecord {
    /// Encode to JPhiraRec V1 binary format
    pub fn encode(&self) -> Result<Vec<u8>> {
        // Encode payload first
        let payload = self.encode_payload()?;
        let payload_size = payload.len() as u64;

        // Compress payload with zstd, including content size in frame header
        let compressed = Self::compress_with_content_size(&payload)?;

        // Build final file
        let mut buf = BytesMut::with_capacity(8 + 4 + 1 + compressed.len());

        // Magic header
        buf.put_slice(PHIRAREC_MAGIC);

        // Version (little endian int32)
        buf.put_i32_le(PHIRAREC_VERSION);

        // Compression type
        buf.put_u8(COMPRESSION_ZSTD);

        // Compressed data
        buf.put_slice(&compressed);

        Ok(buf.freeze().to_vec())
    }

    /// Compress data with zstd, including content size in frame header
    fn compress_with_content_size(data: &[u8]) -> Result<Vec<u8>> {
        use zstd::stream::write::Encoder;
        use std::io::Write;

        // Create encoder with content size included in frame header
        let mut encoder = Encoder::new(Vec::new(), 3)
            .context("Failed to create zstd encoder")?;
        
        // Set content size in frame header - this is crucial for python-zstandard
        encoder.set_pledged_src_size(Some(data.len() as u64))
            .context("Failed to set pledged source size")?;
        
        // Write and compress data
        encoder.write_all(data)
            .context("Failed to write data to zstd encoder")?;
        
        // Finish encoding and get compressed data
        let compressed = encoder.finish()
            .context("Failed to finish zstd encoding")?;
        
        Ok(compressed)
    }

    /// Encode payload (uncompressed content)
    fn encode_payload(&self) -> Result<Vec<u8>> {
        let mut buf = BytesMut::new();

        // Record ID (int32 LE)
        buf.put_i32_le(self.id);

        // Timestamp (int64 LE)
        buf.put_i64_le(self.time as i64);

        // Chart ID (int32 LE)
        buf.put_i32_le(self.chart);

        // Chart name (varint string)
        Self::write_string(&mut buf, &self.chart_name)?;

        // User ID (int32 LE)
        buf.put_i32_le(self.user);

        // User name (varint string)
        Self::write_string(&mut buf, &self.user_name)?;

        // Touch frames list
        Self::write_touch_frames(&mut buf, &self.touch_frames)?;

        // Judge events list
        Self::write_judge_events(&mut buf, &self.judge_events)?;

        Ok(buf.freeze().to_vec())
    }

    /// Write varint32 length-prefixed string
    fn write_string(buf: &mut BytesMut, s: &str) -> Result<()> {
        let bytes = s.as_bytes();
        Self::write_varint32(buf, bytes.len() as i32)?;
        buf.put_slice(bytes);
        Ok(())
    }

    /// Write varint32 (LEB128 encoding)
    fn write_varint32(buf: &mut BytesMut, mut value: i32) -> Result<()> {
        // Convert to unsigned for encoding
        let mut value = value as u32;

        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            if value != 0 {
                byte |= 0x80;
            }

            buf.put_u8(byte);

            if value == 0 {
                break;
            }
        }

        Ok(())
    }

    /// Write list of touch frames
    fn write_touch_frames(buf: &mut BytesMut, frames: &[ReplayTouchFrame]) -> Result<()> {
        // List length (varint32)
        Self::write_varint32(buf, frames.len() as i32)?;

        for frame in frames {
            // Time (float32 LE)
            buf.put_f32_le(frame.time);

            // Points list
            Self::write_touch_points(buf, &frame.points)?;
        }

        Ok(())
    }

    /// Write list of touch points
    fn write_touch_points(buf: &mut BytesMut, points: &[ReplayTouchPoint]) -> Result<()> {
        // List length (varint32)
        Self::write_varint32(buf, points.len() as i32)?;

        for point in points {
            // Point ID (byte)
            buf.put_u8(point.id);

            // X coordinate (float16 LE)
            let x_f16 = f16::from_f32(point.x.clamp(0.0, 1.0));
            buf.put_u16_le(x_f16.to_bits());

            // Y coordinate (float16 LE)
            let y_f16 = f16::from_f32(point.y.clamp(0.0, 1.0));
            buf.put_u16_le(y_f16.to_bits());
        }

        Ok(())
    }

    /// Write list of judge events
    fn write_judge_events(buf: &mut BytesMut, events: &[ReplayJudgeEvent]) -> Result<()> {
        // List length (varint32)
        Self::write_varint32(buf, events.len() as i32)?;

        for event in events {
            // Time (float32 LE)
            buf.put_f32_le(event.time);

            // Line ID (int32 LE)
            buf.put_i32_le(event.line_id);

            // Note ID (int32 LE)
            buf.put_i32_le(event.note_id);

            // Judgement (byte)
            buf.put_u8(event.judgement as u8);
        }

        Ok(())
    }
}

/// Global replay manager for the server
#[derive(Debug, Default)]
pub struct ReplayManager {
    pub room_managers: HashMap<String, RoomReplayManager>,
}

impl ReplayManager {
    pub fn new() -> Self {
        Self {
            room_managers: HashMap::new(),
        }
    }

    pub fn create_room_manager(
        &mut self,
        room_id: String,
        chart_id: i32,
        chart_name: String,
    ) -> &mut RoomReplayManager {
        let manager = RoomReplayManager::new(room_id.clone(), chart_id, chart_name);
        self.room_managers.insert(room_id.clone(), manager);
        self.room_managers.get_mut(&room_id).unwrap()
    }

    pub fn get_room_manager(&mut self, room_id: &str) -> Option<&mut RoomReplayManager> {
        self.room_managers.get_mut(room_id)
    }

    pub fn remove_room_manager(&mut self, room_id: &str) -> Option<RoomReplayManager> {
        self.room_managers.remove(room_id)
    }

    /// Initialize player cache for a room
    pub fn init_player(&mut self, room_id: &str, user_id: i32, user_name: String) {
        if let Some(manager) = self.room_managers.get_mut(room_id) {
            manager.init_player(user_id, user_name);
        }
    }

    /// Record touch frames for a player
    pub fn record_touch_frames(&mut self, room_id: &str, user_id: i32, frames: &[TouchFrame]) {
        if let Some(manager) = self.room_managers.get_mut(room_id) {
            manager.record_touch_frames(user_id, frames);
        }
    }

    /// Record judge events for a player
    pub fn record_judge_events(&mut self, room_id: &str, user_id: i32, events: &[JudgeEvent]) {
        if let Some(manager) = self.room_managers.get_mut(room_id) {
            manager.record_judge_events(user_id, events);
        }
    }

    /// Start recording for a room
    pub fn start_recording(&mut self, room_id: &str) {
        if let Some(manager) = self.room_managers.get_mut(room_id) {
            manager.start_recording();
        }
    }

    /// Stop recording for a room
    pub fn stop_recording(&mut self, room_id: &str) {
        if let Some(manager) = self.room_managers.get_mut(room_id) {
            manager.stop_recording();
        }
    }

    /// Save replays for a room and return saved file paths
    pub async fn save_replays(&mut self, room_id: &str) -> Result<Vec<PathBuf>> {
        if let Some(manager) = self.room_managers.get(room_id) {
            manager.save_replays().await
        } else {
            Ok(Vec::new())
        }
    }

    /// Save and upload replays for a room
    pub async fn save_and_upload_replays(
        &mut self,
        room_id: &str,
        api_url: &str,
        api_token: &str,
    ) -> Result<Vec<(PathBuf, Option<UploadResponse>)>> {
        if let Some(manager) = self.room_managers.get(room_id) {
            manager.save_and_upload_replays(api_url, api_token).await
        } else {
            Ok(Vec::new())
        }
    }
}

/// Recorder bot user ID (negative to avoid conflict with real users)
pub const RECORDER_BOT_USER_ID: i32 = -999;
pub const RECORDER_BOT_USER_NAME: &str = "回放录制器";

/// Check if a user is the recorder bot
pub fn is_recorder_bot(user_id: i32) -> bool {
    user_id == RECORDER_BOT_USER_ID
}
