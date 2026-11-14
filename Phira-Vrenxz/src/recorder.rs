use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static RENDER_ON_NEXT_PLAY: AtomicBool = AtomicBool::new(false);

pub fn set_render_on_next_play(v: bool) {
    RENDER_ON_NEXT_PLAY.store(v, Ordering::Relaxed);
}

pub fn take_render_on_next_play() -> bool {
    RENDER_ON_NEXT_PLAY.swap(false, Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderConfig {
    pub fps: u32,
    pub crf: u8,
    pub preset: String,
    pub output_dir: String,
    pub record_audio: bool,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            fps: 60,
            crf: 23,
            preset: "fast".to_string(),
            output_dir: "recordings".to_string(),
            record_audio: true,
        }
    }
}

pub struct Recorder {
    config: RecorderConfig,
    child: Option<std::process::Child>,
    stdin: Option<ChildStdin>,
    width: u16,
    height: u16,
    recording: bool,
    frame_count: u64,
    total_bytes: u64,
    start_time: Instant,
    last_capture: Instant,
    frame_interval: Duration,
    output_path: Option<PathBuf>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            config: RecorderConfig::default(),
            child: None,
            stdin: None,
            width: 0,
            height: 0,
            recording: false,
            frame_count: 0,
            total_bytes: 0,
            start_time: Instant::now(),
            last_capture: Instant::now(),
            frame_interval: Duration::from_secs_f64(1.0 / 60.0),
            output_path: None,
        }
    }

    pub fn config(&self) -> &RecorderConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut RecorderConfig {
        &mut self.config
    }

    pub fn toggle(&mut self) {
        if self.recording {
            self.stop();
        } else {
            self.start();
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn start(&mut self) {
        let (width, height) = (screen_width() as u16, screen_height() as u16);
        if width == 0 || height == 0 {
            warn!("Recorder: screen size is 0");
            return;
        }

        self.frame_interval = Duration::from_secs_f64(1.0 / self.config.fps as f64);

        let output_dir = PathBuf::from(&self.config.output_dir);
        std::fs::create_dir_all(&output_dir).ok();

        let filename = format!(
            "recording_{}.mp4",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );

        let output_path = output_dir.join(&filename);
        let stderr_path = output_dir.join("ffmpeg_stderr.log");

        info!("Recorder: starting {}x{} @ {}fps, audio: {}", width, height, self.config.fps, self.config.record_audio);
        info!("Recorder: output: {}", output_path.display());

        let stderr_file = std::fs::File::create(&stderr_path).ok();

        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-y")
            .arg("-f").arg("rawvideo")
            .arg("-pixel_format").arg("rgba")
            .arg("-video_size").arg(format!("{}x{}", width, height))
            .arg("-framerate").arg(self.config.fps.to_string())
            .arg("-i").arg("pipe:0");

        if self.config.record_audio {
            cmd.arg("-f").arg("dshow")
                .arg("-i").arg("audio=virtual-audio-capturer");
        }

        cmd.arg("-vf").arg("vflip,scale=trunc(iw/2)*2:trunc(ih/2)*2")
            .arg("-c:v").arg("libx264")
            .arg("-preset").arg(&self.config.preset)
            .arg("-crf").arg(self.config.crf.to_string())
            .arg("-pix_fmt").arg("yuv420p");

        if self.config.record_audio {
            cmd.arg("-c:a").arg("aac")
                .arg("-b:a").arg("192k");
        }

        cmd.arg("-movflags").arg("+faststart")
            .arg(&output_path)
            .stdin(Stdio::piped());

        if let Some(f) = stderr_file {
            cmd.stderr(Stdio::from(f));
        } else {
            cmd.stderr(Stdio::null());
        }

        match cmd.spawn() {
            Ok(mut child) => {
                self.stdin = child.stdin.take();
                self.child = Some(child);
                self.width = width;
                self.height = height;
                self.recording = true;
                self.frame_count = 0;
                self.total_bytes = 0;
                self.start_time = Instant::now();
                self.last_capture = Instant::now();
                self.output_path = Some(output_path);
                info!("Recorder: ffmpeg started successfully");
            }
            Err(e) => {
                error!("Recorder: Failed to start ffmpeg: {}", e);
            }
        }
    }

    pub fn stop(&mut self) {
        self.recording = false;
        if let Some(stdin) = self.stdin.take() {
            drop(stdin);
        }
        if let Some(mut child) = self.child.take() {
            match child.wait() {
                Ok(status) => info!("Recorder: ffmpeg exited with {}", status),
                Err(e) => error!("Recorder: failed to wait ffmpeg: {}", e),
            }
        }
        let elapsed = self.start_time.elapsed().as_secs_f32();
        info!(
            "Recorder: stopped: {} frames, {} MB in {:.1}s ({:.1} fps)",
            self.frame_count,
            self.total_bytes as f32 / 1024.0 / 1024.0,
            elapsed,
            self.frame_count as f32 / elapsed
        );
        if let Some(path) = &self.output_path {
            if let Ok(meta) = std::fs::metadata(path) {
                info!("Recorder: output file size: {} bytes", meta.len());
            }
        }
    }

    pub fn capture_frame(&mut self) {
        if !self.recording {
            return;
        }

        if self.last_capture.elapsed() < self.frame_interval {
            return;
        }
        self.last_capture += self.frame_interval;

        let pixels = get_screen_data();
        let (w, h) = (pixels.width, pixels.height);

        if w != self.width || h != self.height {
            return;
        }

        if let Some(ref mut stdin) = self.stdin {
            let data = &pixels.bytes;
            if stdin.write_all(data).is_err() {
                error!("Recorder: Failed to write frame to ffmpeg");
                self.stop();
            }
            self.frame_count += 1;
            self.total_bytes += data.len() as u64;
        }
    }
}

static RECORDER: std::sync::OnceLock<Mutex<Recorder>> = std::sync::OnceLock::new();

pub fn recorder() -> &'static Mutex<Recorder> {
    RECORDER.get_or_init(|| Mutex::new(Recorder::new()))
}
