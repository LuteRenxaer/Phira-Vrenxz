//! Offline chart renderer, ported from Phi Recorder's render pipeline.
//!
//! Phi Recorder renders the real prpr scene into an offscreen MSAA target at a
//! fixed resolution, advances a manual clock, reads back RGBA frames and pipes
//! them to ffmpeg.  This module keeps that pipeline but uses Phira-Vrenxz's
//! `prpr` engine directly instead of Phi Recorder's private `phire` fork.

use anyhow::{bail, Context, Result};
use macroquad::miniquad::gl::*;
use macroquad::prelude::*;
use prpr::{
    config::{Config, Mods},
    core::{internal_id, HitSound, HitSoundMap, MSRenderTarget, ResPackInfo, PGR_FONT},
    dir::Dir,
    ext::{SafeTexture, BLACK_TEXTURE},
    fs::{self, ExternalFileSystem, FileSystem, ZipFileSystem},
    info::ChartInfo,
    judge::PlayResult,
    scene::{BasicPlayer, EndingScene, GameMode, GameScene, LoadingScene, Main},
    time::TimeManager,
    ui::{FontArc, TextPainter},
};
use std::{
    any::Any,
    cell::Cell,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use sasa::{AudioClip, Frame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    Cpu,
    Gpu,
}

impl RenderBackend {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplayStyle {
    Default,
    Combo,
}

impl AutoplayStyle {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Combo => "combo",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub crf: u8,
    pub preset: String,
    pub audio: bool,
    pub end: Option<f64>,
    pub sample_count: u32,
    pub backend: RenderBackend,
    pub loading: bool,
    pub loading_secs: f64,
    pub ending: bool,
    pub ending_secs: f64,
    pub indicator: bool,
    pub watermark: bool,
    pub autoplay_style: AutoplayStyle,
    pub autoplay_text: String,
    pub aspect: Option<f32>,
    pub tip: Option<String>,
    pub res_pack: Option<String>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: PathBuf::new(),
            fps: 60,
            width: 1920,
            height: 1080,
            crf: 18,
            preset: "veryfast".to_string(),
            audio: true,
            end: None,
            sample_count: 1,
            backend: RenderBackend::Cpu,
            loading: false,
            loading_secs: 2.0,
            ending: false,
            ending_secs: 3.0,
            indicator: true,
            watermark: false,
            autoplay_style: AutoplayStyle::Default,
            autoplay_text: "Autoplay".to_string(),
            aspect: None,
            tip: None,
            res_pack: None,
        }
    }
}

/// Options chosen from the in-game render panel.
#[derive(Debug, Clone)]
pub struct GameRenderOptions {
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub crf: u8,
    pub audio: bool,
    pub backend: RenderBackend,
    pub loading: bool,
    pub loading_secs: f64,
    pub ending: bool,
    pub ending_secs: f64,
    pub indicator: bool,
    pub watermark: bool,
    pub autoplay_style: AutoplayStyle,
    pub autoplay_text: String,
    pub aspect: Option<f32>,
    pub tip: Option<String>,
    pub res_pack: Option<String>,
}

impl Default for GameRenderOptions {
    fn default() -> Self {
        Self {
            fps: 60,
            width: 1920,
            height: 1080,
            crf: 18,
            audio: true,
            backend: RenderBackend::Cpu,
            loading: false,
            loading_secs: 2.0,
            ending: false,
            ending_secs: 3.0,
            indicator: true,
            watermark: false,
            autoplay_style: AutoplayStyle::Default,
            autoplay_text: "Autoplay".to_string(),
            aspect: None,
            tip: None,
            res_pack: None,
        }
    }
}

fn absolute(p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir().map(|c| c.join(&p)).unwrap_or(p)
    }
}

fn next_value(args: &[String], i: &mut usize) -> Result<String> {
    *i += 1;
    args.get(*i)
        .cloned()
        .with_context(|| format!("missing value after {}", args[*i - 1]))
}

fn parse_u32(s: &str, what: &str) -> Result<u32> {
    s.parse().with_context(|| format!("invalid {what}: {s}"))
}

fn parse_secs(s: &str, min: f64, max: f64) -> Result<f64> {
    let v: f64 = s.parse().with_context(|| format!("invalid seconds: {s}"))?;
    Ok(v.clamp(min, max))
}

fn parse_on_off(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false" | "no")
}

fn parse_aspect(s: &str) -> Result<f32> {
    let s = s.trim();
    let v = if let Some((w, h)) = s.split_once(':') {
        let w: f32 = w.parse().with_context(|| format!("invalid aspect width: {s}"))?;
        let h: f32 = h.parse().with_context(|| format!("invalid aspect height: {s}"))?;
        w / h
    } else {
        s.parse().with_context(|| format!("invalid aspect: {s}"))?
    };
    if !(0.4..=3.0).contains(&v) {
        bail!("aspect out of range: {v}");
    }
    Ok(v)
}

fn default_output(input: &Path) -> PathBuf {
    let name = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("chart");
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("recordings")
        .join(format!("{name}_{stamp}.mp4"))
}

/// Parse the render-only command line.  Returns None when the process should
/// start the normal game UI.
pub fn parse_cli() -> Option<RenderOptions> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == "--render") {
        return None;
    }

    let mut opts = RenderOptions::default();
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--render" => {
                // `--render <chart>` shorthand, or `--render --input <chart>`
                if let Some(v) = args.get(i + 1) {
                    if !v.starts_with('-') && input.is_none() {
                        input = Some(PathBuf::from(v));
                        i += 1;
                    }
                }
            }
            "--input" | "-i" => input = Some(PathBuf::from(next_value(&args, &mut i).ok()?)),
            "--output" | "-o" => output = Some(PathBuf::from(next_value(&args, &mut i).ok()?)),
            "--fps" => opts.fps = parse_u32(&next_value(&args, &mut i).ok()?, "fps").ok()?.clamp(1, 240),
            "--width" | "-w" => opts.width = parse_u32(&next_value(&args, &mut i).ok()?, "width").ok()?.max(64),
            "--height" | "-h" => opts.height = parse_u32(&next_value(&args, &mut i).ok()?, "height").ok()?.max(64),
            "--crf" => opts.crf = parse_u32(&next_value(&args, &mut i).ok()?, "crf").ok()?.min(51) as u8,
            "--preset" => opts.preset = next_value(&args, &mut i).ok()?,
            "--samples" => opts.sample_count = parse_u32(&next_value(&args, &mut i).ok()?, "samples").ok()?.clamp(1, 8),
            "--end" => opts.end = next_value(&args, &mut i).ok()?.parse().ok(),
            "--no-audio" => opts.audio = false,
            "--backend" => {
                let v = next_value(&args, &mut i).ok()?;
                opts.backend = match v.as_str() {
                    "gpu" => RenderBackend::Gpu,
                    "cpu" => RenderBackend::Cpu,
                    _ => {
                        eprintln!("render: invalid backend: {v} (cpu|gpu)");
                        return None;
                    }
                };
            }
            "--loading" => {
                opts.loading = true;
                opts.loading_secs = parse_secs(&next_value(&args, &mut i).ok()?, 0.2, 60.0).ok()?;
            }
            "--no-loading" => opts.loading = false,
            "--ending" => {
                opts.ending = true;
                opts.ending_secs = parse_secs(&next_value(&args, &mut i).ok()?, 0.2, 60.0).ok()?;
            }
            "--no-ending" => opts.ending = false,
            "--indicator" => opts.indicator = parse_on_off(&next_value(&args, &mut i).ok()?),
            "--no-indicator" => opts.indicator = false,
            "--watermark" => opts.watermark = parse_on_off(&next_value(&args, &mut i).ok()?),
            "--no-watermark" => opts.watermark = false,
            "--autoplay-style" => {
                let v = next_value(&args, &mut i).ok()?;
                opts.autoplay_style = match v.as_str() {
                    "combo" => AutoplayStyle::Combo,
                    "default" => AutoplayStyle::Default,
                    _ => {
                        eprintln!("render: invalid autoplay style: {v} (default|combo)");
                        return None;
                    }
                };
            }
            "--autoplay-text" => opts.autoplay_text = next_value(&args, &mut i).ok()?,
            "--aspect" => opts.aspect = parse_aspect(&next_value(&args, &mut i).ok()?).ok(),
            "--no-aspect" => opts.aspect = None,
            "--tip" => {
                let tip = next_value(&args, &mut i).ok()?;
                opts.tip = (!tip.trim().is_empty()).then_some(tip);
            }
            "--res-pack" => {
                let p = next_value(&args, &mut i).ok()?;
                opts.res_pack = (!p.trim().is_empty()).then_some(p);
            }
            "--help" => {
                eprintln!(
                    "Phira-Vrenxz offline renderer\n\
                     usage: Phira-Vrenxz.exe --render <chart.pez|dir> [options]\n\
                     options: -o/--output out.mp4  --fps 60  -w/--width 1920  -h/--height 1080\n\
                     --crf 18  --preset veryfast  --samples 1  --end seconds  --no-audio\n\
                     --backend cpu|gpu  --loading sec  --no-loading  --ending sec  --no-ending\n\
                     --indicator on|off  --watermark on|off  --autoplay-style default|combo\n\
                     --autoplay-text TEXT  --aspect 16:9|1.777  --no-aspect  --tip TEXT  --res-pack PATH"
                );
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    let Some(input) = input else {
        eprintln!("render: missing --render <chart> / --input <chart>");
        std::process::exit(2);
    };
    if !input.exists() {
        eprintln!("render: input not found: {}", input.display());
        std::process::exit(2);
    }
    opts.input = absolute(input);
    opts.output = absolute(output.unwrap_or_else(|| default_output(&opts.input)));
    Some(opts)
}

#[cfg(target_os = "windows")]
fn hide_current_window() {
    use std::ffi::c_void;
    type Hwnd = *mut c_void;
    extern "system" {
        fn EnumWindows(cb: extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn GetCurrentProcessId() -> u32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    }
    extern "system" fn hide_if_mine(hwnd: Hwnd, _: isize) -> i32 {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == GetCurrentProcessId() {
                ShowWindow(hwnd, 0); // SW_HIDE
            }
        }
        1
    }
    unsafe {
        EnumWindows(hide_if_mine, 0);
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_current_window() {}

fn ffmpeg_cmd() -> Command {
    let cmd = Command::new("ffmpeg");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = cmd;
        cmd.creation_flags(0x08000000);
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    cmd
}

/// `ExternalFileSystem` uses `tokio::task::spawn_blocking`, which cannot wake
/// macroquad's single-threaded future executor.  The renderer only needs local
/// files, so use a synchronous wrapper around the same `Dir`.
#[derive(Clone)]
struct SyncDirFs(Arc<Dir>);

#[async_trait::async_trait]
impl FileSystem for SyncDirFs {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        Ok(self.0.read(path)?)
    }

    async fn exists(&mut self, path: &str) -> Result<bool> {
        self.0.exists(path)
    }

    fn list_root(&self) -> Result<Vec<String>> {
        Ok(self
            .0
            .read_dir(".")?
            .filter_map(|res| res.ok()?.file_name().into_string().ok())
            .collect())
    }

    fn clone_box(&self) -> Box<dyn FileSystem> {
        Box::new(self.clone())
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

/// The archive-backed file system also goes through `spawn_task` for reads;
/// read it directly so macroquad's executor never waits on a tokio thread.
#[derive(Clone)]
struct SyncZipFs(ZipFileSystem);

#[async_trait::async_trait]
impl FileSystem for SyncZipFs {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let full = format!("{}{}", self.0.root(), path);
        let mut archive = self.0.0.lock().unwrap();
        let mut entry = archive.by_name(&full)?;
        let mut res = Vec::new();
        entry.read_to_end(&mut res)?;
        Ok(res)
    }

    async fn exists(&mut self, path: &str) -> Result<bool> {
        let full = format!("{}{}", self.0.root(), path);
        Ok(self.0.0.lock().unwrap().by_name(&full).is_ok())
    }

    fn list_root(&self) -> Result<Vec<String>> {
        self.0.list_root()
    }

    fn clone_box(&self) -> Box<dyn FileSystem> {
        Box::new(self.clone())
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

async fn load_background(fs: &mut dyn FileSystem, path: &str) -> Result<(SafeTexture, SafeTexture)> {
    let image = image::load_from_memory(&fs.load_file(path).await?).context("decode illustration")?;
    let (w, h) = (image.width(), image.height());

    let max_w = 1920.0;
    let scale = (max_w / w as f32).min(1.0);
    let (sw, sh) = ((w as f32 * scale) as u32, (h as f32 * scale) as u32);
    let scaled = image.resize_exact(sw, sh, image::imageops::FilterType::Triangle);
    let original_rgba = scaled.to_rgba8();

    let blur_scale = (1280.0 / sw as f32).min(1.0);
    let (bw, bh) = ((sw as f32 * blur_scale) as u32, (sh as f32 * blur_scale) as u32);
    let blurred = scaled.resize_exact(bw, bh, image::imageops::FilterType::Triangle);
    let blurred_rgb = blurred.to_rgb8();
    let mut pixel_data: Vec<[u8; 3]> = blurred_rgb
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect();
    fastblur::gaussian_blur(&mut pixel_data, bw as _, bh as _, 50.0);
    let blurred_rgb_u8: Vec<u8> = pixel_data.into_iter().flat_map(|p| p.to_vec()).collect();
    let mut blurred_rgba = Vec::with_capacity(bw as usize * bh as usize * 4);
    for chunk in blurred_rgb_u8.chunks_exact(3) {
        blurred_rgba.extend_from_slice(chunk);
        blurred_rgba.push(255);
    }

    Ok((
        Texture2D::from_rgba8(sw as _, sh as _, &original_rgba).into(),
        Texture2D::from_image(&Image {
            width: bw as u16,
            height: bh as u16,
            bytes: blurred_rgba,
        })
        .into(),
    ))
}

/// Mix the chart's music with default/custom hit sounds into a temporary WAV.
/// Phi Recorder renders the whole soundtrack offline; doing the same here
/// avoids relying on a loopback audio device.
fn write_mixed_wav(
    path: &Path,
    music: &AudioClip,
    hits: &[(f64, HitSound)],
    custom: &HitSoundMap,
    click: Option<&AudioClip>,
    drag: Option<&AudioClip>,
    flick: Option<&AudioClip>,
    ending: Option<&AudioClip>,
    sfx_gain: f32,
    game_secs: f64,
    lead_secs: f64,
    tail_secs: f64,
) -> Result<()> {
    let rate = music.sample_rate().max(1);
    let lead = (lead_secs.max(0.0) * rate as f64).round() as usize;
    let game = (game_secs.max(0.0) * rate as f64).round() as usize;
    let tail = (tail_secs.max(0.0) * rate as f64).round() as usize;
    let music_len = music.frames().len().min(game);
    let mut mixed: Vec<(f32, f32)> = vec![(0.0f32, 0.0f32); lead];
    mixed.extend(music.frames()[..music_len].iter().map(|f| (f.0, f.1)));
    mixed.resize(lead + game, (0.0, 0.0));
    mixed.resize(lead + game + tail, (0.0, 0.0));

    for (time, sound) in hits {
        let clip = match sound {
            HitSound::None => continue,
            HitSound::Click => click,
            HitSound::Drag => drag,
            HitSound::Flick => flick,
            HitSound::Custom(name) => custom.get(name),
        };
        let Some(clip) = clip else { continue };
        let offset = (lead as f64 + *time * rate as f64) as isize;
        if offset < 0 {
            continue;
        }
        let offset = offset as usize;
        let game_end = lead + game;
        if offset >= game_end {
            continue;
        }
        let n = clip.frames().len().min(game_end - offset);
        for i in 0..n {
            let s = clip.frames()[i];
            let dst = &mut mixed[offset + i];
            dst.0 += s.0 * sfx_gain;
            dst.1 += s.1 * sfx_gain;
        }
    }

    // The ending scene's BGM is mixed into the tail explicitly: the renderer
    // runs with a silent audio backend, so EndingScene's own Music never
    // reaches the WAV.
    if let Some(clip) = ending {
        let frames = clip.frames();
        if !frames.is_empty() && tail > 0 {
            let end_rate = clip.sample_rate().max(1) as f64;
            let start = lead + game;
            for i in 0..tail {
                let src = ((i as f64 * end_rate / rate as f64) as usize) % frames.len();
                let s = frames[src];
                let dst = &mut mixed[start + i];
                dst.0 += s.0;
                dst.1 += s.1;
            }
        }
    }

    let mut out = Vec::with_capacity(mixed.len() * 4);
    // Music is already mastered near 0 dB; adding hitsounds on top can clip.
    // Scale the whole mix only when needed so the music/hit balance is kept.
    let peak = mixed
        .iter()
        .map(|f| f.0.abs().max(f.1.abs()))
        .fold(0.0f32, f32::max);
    let gain = if peak > 0.98 { 0.98 / peak } else { 1.0 };
    for f in &mut mixed {
        f.0 *= gain;
        f.1 *= gain;
        let l = (f.0.clamp(-1.0, 1.0) * 32767.0) as i16;
        let r = (f.1.clamp(-1.0, 1.0) * 32767.0) as i16;
        out.extend_from_slice(&l.to_le_bytes());
        out.extend_from_slice(&r.to_le_bytes());
    }

    let data_len = out.len() as u32;
    let byte_rate = rate * 4;
    let mut wav = Vec::with_capacity(44 + out.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&2u16.to_le_bytes()); // stereo
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&out);
    std::fs::write(path, wav)?;
    Ok(())
}

/// Triple-buffered PBO readback.  A direct `glReadPixels` into client memory
/// stalls the GPU every frame; reading into a pack buffer and mapping the
/// oldest one lets the GPU run a couple of frames ahead instead.
struct FrameReader {
    width: i32,
    height: i32,
    pbos: [GLuint; 3],
    submitted: u64,
    drained: u64,
}

impl FrameReader {
    fn new(width: u32, height: u32) -> Self {
        let bytes = (width * height * 4) as usize;
        let mut pbos = [0; 3];
        unsafe {
            glGenBuffers(pbos.len() as _, pbos.as_mut_ptr());
            for pbo in pbos {
                glBindBuffer(GL_PIXEL_PACK_BUFFER, pbo);
                glBufferData(
                    GL_PIXEL_PACK_BUFFER,
                    bytes as _,
                    std::ptr::null(),
                    GL_STREAM_READ,
                );
            }
            glBindBuffer(GL_PIXEL_PACK_BUFFER, 0);
        }
        Self {
            width: width as i32,
            height: height as i32,
            pbos,
            submitted: 0,
            drained: 0,
        }
    }

    /// Start an asynchronous readback of `fbo` and write the oldest finished
    /// frame to ffmpeg's stdin.
    fn push(&mut self, fbo: GLuint, stdin: &mut impl Write) -> Result<()> {
        let idx = (self.submitted % self.pbos.len() as u64) as usize;
        unsafe {
            glBindFramebuffer(GL_READ_FRAMEBUFFER, fbo);
            glReadBuffer(GL_COLOR_ATTACHMENT0);
            glBindBuffer(GL_PIXEL_PACK_BUFFER, self.pbos[idx]);
            glReadPixels(
                0,
                0,
                self.width,
                self.height,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                std::ptr::null_mut(),
            );
        }
        self.submitted += 1;
        if self.submitted - self.drained >= self.pbos.len() as u64 {
            self.drain_one(stdin)?;
        }
        Ok(())
    }

    fn drain_one(&mut self, stdin: &mut impl Write) -> Result<()> {
        let idx = (self.drained % self.pbos.len() as u64) as usize;
        let bytes = self.width as usize * self.height as usize * 4;
        unsafe {
            glBindBuffer(GL_PIXEL_PACK_BUFFER, self.pbos[idx]);
            let src = glMapBuffer(GL_PIXEL_PACK_BUFFER, 0x88B8 /* GL_READ_ONLY */);
            if src.is_null() {
                glBindBuffer(GL_PIXEL_PACK_BUFFER, 0);
                bail!("failed to map readback buffer");
            }
            let pixels = std::slice::from_raw_parts(src as *const u8, bytes);
            let res = stdin.write_all(pixels);
            glUnmapBuffer(GL_PIXEL_PACK_BUFFER);
            glBindBuffer(GL_PIXEL_PACK_BUFFER, 0);
            res.context("failed to write frame to ffmpeg")?;
        }
        self.drained += 1;
        Ok(())
    }

    /// Drain every frame still in flight (called before closing ffmpeg's stdin).
    fn finish(&mut self, stdin: &mut impl Write) -> Result<()> {
        while self.drained < self.submitted {
            self.drain_one(stdin)?;
        }
        unsafe {
            glBindFramebuffer(GL_READ_FRAMEBUFFER, 0);
            glBindBuffer(GL_PIXEL_PACK_BUFFER, 0);
        }
        Ok(())
    }
}

impl Drop for FrameReader {
    fn drop(&mut self) {
        unsafe {
            glDeleteBuffers(self.pbos.len() as _, self.pbos.as_ptr());
        }
    }
}

/// Render a chart to an mp4.  Must run after macroquad created the GL context.
pub async fn run(mut opts: RenderOptions) -> Result<()> {
    prpr::core::init_assets();
    if let Some(aspect) = opts.aspect {
        let h = opts.height;
        let w = ((h as f32 * aspect).round() as u32).max(2);
        opts.width = w - w % 2;
    }
    // Never open the audio device from the renderer.  The mixed WAV still
    // contains music + hits; only the live playthrough is muted.
    std::env::set_var("PHIRA_RENDER_SILENT", "1");
    prpr::gamepad::set_enabled(false);
    hide_current_window();

    let started = Instant::now();
    let mut fs = fs::fs_from_file(&opts.input)?;
    if let Some(ext) = fs.as_any().downcast_mut::<ExternalFileSystem>() {
        fs = Box::new(SyncDirFs(Arc::clone(&ext.0)));
    } else if let Some(zip) = fs.as_any().downcast_mut::<ZipFileSystem>() {
        fs = Box::new(SyncZipFs(zip.clone()));
    }
    let info = fs::load_info(fs.as_mut()).await?;
    let music_bytes = if opts.audio {
        fs.load_file(&info.music).await.ok()
    } else {
        None
    };

    let mut config = Config::default();
    config.mods = Mods::AUTOPLAY;
    config.volume_music = 0.0;
    config.volume_sfx = 0.0;
    config.show_fps = false;
    config.show_avg_fps = false;
    config.show_acc = false;
    config.show_score = true;
    config.show_combo = true;
    config.show_watermark = opts.watermark;
    config.custom_watermark.clear();
    config.indicator_enabled = opts.indicator;
    config.autoplay_combo_style = matches!(opts.autoplay_style, AutoplayStyle::Combo);
    config.autoplay_display_text = opts.autoplay_text.clone();
    config.aspect_ratio = opts.aspect;
    config.res_pack_path = opts.res_pack.clone();

    // Offline render is a separate process that never calls `the_main`, so
    // `get_data()` is not initialized here.  Read the same data.json directly
    // and use the currently logged-in account for name / rks / avatar instead
    // of the Mivik defaults.
    let profile = load_render_profile(&opts.input).await;
    let player_name = profile
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| config.player_name.clone());
    let player_id = profile.as_ref().map(|p| p.id).unwrap_or(0);
    let player_rks = profile.as_ref().map(|p| p.rks).unwrap_or(config.player_rks);
    let player_avatar = profile.as_ref().and_then(|p| p.avatar.clone());
    config.player_name = player_name.clone();
    config.player_rks = player_rks;

    let (illustration, background) = match load_background(fs.as_mut(), &info.illustration).await {
        Ok(v) => v,
        Err(err) => {
            eprintln!("render: background unavailable, using black: {err:?}");
            (BLACK_TEXTURE.clone(), BLACK_TEXTURE.clone())
        }
    };

    let player = BasicPlayer {
        avatar: player_avatar.clone(),
        id: player_id,
        rks: player_rks,
        historic_best: 0,
    };

    let info_offset = info.offset as f64;
    let info_for_scene = info.clone();
    let scene = GameScene::new_preview(
        GameMode::Normal,
        info_for_scene,
        config,
        fs,
        Some(player),
        background.clone(),
        illustration.clone(),
        None,
        None,
        None,
        true, // preview mode: never write a local record
        None,
        false,
        None,
    )
    .await
    .context("failed to create game scene")?;

    let track_length = scene.res.track_length;
    let chart_offset = scene.chart.offset as f64 + info_offset + scene.res.config.offset as f64;
    let game_duration = opts
        .end
        .unwrap_or(track_length + 0.8)
        .clamp(0.1, track_length + 1.5);
    let lead_secs = if opts.loading { opts.loading_secs } else { 0.0 };
    let tail_secs = if opts.ending { opts.ending_secs } else { 0.0 };
    let total_duration = game_duration + lead_secs + tail_secs;
    let load_frames = (lead_secs * opts.fps as f64).round() as u64;
    let game_frames = (game_duration * opts.fps as f64).ceil() as u64;
    let end_frames = (tail_secs * opts.fps as f64).round() as u64;
    let frames = load_frames + game_frames + end_frames;
    if frames == 0 {
        bail!("render duration is zero");
    }

    // Chart hit sounds (default click/drag/flick + RPE custom hitsounds).
    // `GameScene` draws notes at `tm.now() - chart_offset`, so a note with
    // `note.time` is hit at `note.time + chart_offset` on the video timeline.
    let hits: Vec<(f64, HitSound)> = scene
        .chart
        .lines
        .iter()
        .flat_map(|line| line.notes.iter())
        .filter(|note| !note.fake && !matches!(note.hitsound, HitSound::None))
        .map(|note| ((note.time + chart_offset).max(0.0), note.hitsound.clone()))
        .collect();
    let custom_hitsounds = scene.chart.hitsounds.clone();
    let first_hit = hits.iter().map(|(t, _)| *t).fold(f64::INFINITY, f64::min);
    let num_notes = scene
        .chart
        .lines
        .iter()
        .flat_map(|line| line.notes.iter())
        .filter(|note| !note.fake)
        .count() as u32;

    // Optional intro / outro scenes.  Build them from the game resource before
    // the game scene is moved into `Main`.
    let loading_scene = if opts.loading {
        Some(LoadingScene::render_preview(
            info.clone(),
            background.clone(),
            illustration.clone(),
            opts.tip.clone(),
        ))
    } else {
        None
    };
    let ending_scene = if opts.ending {
        let result = PlayResult {
            score: 1_000_000,
            accuracy: 1.0,
            max_combo: num_notes,
            num_of_notes: num_notes,
            counts: [num_notes, 0, 0, 0],
            early: 0,
            late: 0,
            std: 0.0,
            early_kind: [0; 4],
            late_kind: [0; 4],
            perfect_plus: num_notes,
            great: 0,
            ok: 0,
            meh: 0,
            diffs: Vec::new(),
        };
        let res = &scene.res;
        Some(
            EndingScene::new(
                res.background.clone(),
                res.illustration.clone(),
                player_avatar.clone().unwrap_or_else(|| BLACK_TEXTURE.clone()),
                res.icons.clone(),
                res.arc_icon.clone(),
                res.icon_retry.clone(),
                res.icon_proceed.clone(),
                res.mod_icons.clone(),
                res.info.clone(),
                result,
                &res.config,
                res.res_pack.ending.clone(),
                None,
                Some(player_rks),
                0,
                None,
                None,
                None,
                None,
            )
            .context("failed to create ending scene")?,
        )
    } else {
        None
    };

    // Fonts.  GameScene draws score/combo through PGR_FONT, the UI uses the
    // supplied TextPainter.
    let pgr_font = FontArc::try_from_vec(load_file("fonts/phigros.ttf").await?)
        .context("load phigros font")?;
    PGR_FONT.with(|it| *it.borrow_mut() = Some(TextPainter::new(pgr_font, None)));
    let main_font = FontArc::try_from_vec(load_file("fonts/font.ttf").await?)
        .context("load main font")?;
    let mut painter = TextPainter::new(main_font, None);

    if let Some(parent) = opts.output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let log_path = opts.output.with_extension("render.log");
    let _ = std::fs::write(
        &log_path,
        format!(
            "input={}\noutput={}\n{}x{} @ {}fps, duration={:.3}s, game={:.3}s loading={:.3}s ending={:.3}s, frames={}\nchart_offset={:.3}s, first_hit={:.3}s\n",
            opts.input.display(),
            opts.output.display(),
            opts.width,
            opts.height,
            opts.fps,
            total_duration,
            game_duration,
            lead_secs,
            tail_secs,
            frames,
            chart_offset,
            first_hit
        ),
    );

    let mut music_temp = None;
    if opts.audio {
        if let Some(bytes) = music_bytes {
            let music = AudioClip::new(bytes).context("decode chart music")?;
            // Use the selected resource pack's hit sounds, not the global
            // fallback files, so the audio matches the pack's visuals.
            let click = scene.res.res_pack.sfx_click.clone();
            let drag = scene.res.res_pack.sfx_drag.clone();
            let flick = scene.res.res_pack.sfx_flick.clone();
            let mut tmp = tempfile::Builder::new()
                .prefix("phira-render-audio-")
                .suffix(".wav")
                .tempfile()
                .context("create mixed audio temp file")?;
            let ending_music = (tail_secs > 0.0).then(|| scene.res.res_pack.ending.clone());
            write_mixed_wav(
                tmp.path(),
                &music,
                &hits,
                &custom_hitsounds,
                Some(&click),
                Some(&drag),
                Some(&flick),
                ending_music.as_ref(),
                0.55,
                game_duration,
                lead_secs,
                tail_secs,
            )?;
            tmp.flush()?;
            music_temp = Some(tmp);
        }
    }

    let mut cmd = ffmpeg_cmd();
    cmd.args([
        "-y",
        "-f",
        "rawvideo",
        "-pixel_format",
        "rgba",
        "-video_size",
        &format!("{}x{}", opts.width, opts.height),
        "-framerate",
        &opts.fps.to_string(),
        "-i",
        "pipe:0",
    ]);
    if let Some(tmp) = &music_temp {
        cmd.arg("-i").arg(tmp.path());
    }
    let use_gpu = matches!(opts.backend, RenderBackend::Gpu) && encoder_available("h264_nvenc");
    if use_gpu {
        cmd.args([
            "-vf",
            "vflip",
            "-c:v",
            "h264_nvenc",
            "-preset",
            "p5",
            "-cq",
            &opts.crf.to_string(),
            "-b:v",
            "0",
            "-pix_fmt",
            "yuv420p",
        ]);
    } else {
        cmd.args([
            "-vf",
            "vflip",
            "-c:v",
            "libx264",
            "-preset",
            &opts.preset,
            "-crf",
            &opts.crf.to_string(),
            "-pix_fmt",
            "yuv420p",
        ]);
    }
    cmd.args([
        "-movflags",
        "+faststart",
        "-t",
        &format!("{total_duration:.3}"),
    ]);
    if music_temp.is_some() {
        cmd.args(["-c:a", "aac", "-b:a", "192k"]);
    } else {
        cmd.arg("-an");
    }
    cmd.arg(&opts.output);
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to start ffmpeg; output={}", opts.output.display()))?;
    let mut stdin = child.stdin.take().context("ffmpeg stdin unavailable")?;

    let time = Rc::new(Cell::new(0.0f64));
    let tm = TimeManager::manual(Box::new({
        let time = Rc::clone(&time);
        move || time.get()
    }));

    let mst = Rc::new(MSRenderTarget::new(
        (opts.width, opts.height),
        opts.sample_count.max(1),
    ));
    let chooser_target = Rc::clone(&mst);
    let mut main = Main::new(Box::new(scene), tm, move || Some(chooser_target.output())).await?;
    main.top_level = false;
    main.viewport = Some((0, 0, opts.width as i32, opts.height as i32));

    let mut loading_main = if let Some(load_scene) = loading_scene {
        let load_time = Rc::new(Cell::new(0.0f64));
        let load_tm = TimeManager::manual(Box::new({
            let load_time = Rc::clone(&load_time);
            move || load_time.get()
        }));
        let chooser_target = Rc::clone(&mst);
        let mut m = Main::new(Box::new(load_scene), load_tm, move || Some(chooser_target.output())).await?;
        m.top_level = false;
        m.viewport = Some((0, 0, opts.width as i32, opts.height as i32));
        Some((m, load_time))
    } else {
        None
    };
    let mut ending_main = if let Some(end_scene) = ending_scene {
        let end_time = Rc::new(Cell::new(0.0f64));
        let end_tm = TimeManager::manual(Box::new({
            let end_time = Rc::clone(&end_time);
            move || end_time.get()
        }));
        let chooser_target = Rc::clone(&mst);
        let mut m = Main::new(Box::new(end_scene), end_tm, move || Some(chooser_target.output())).await?;
        m.top_level = false;
        m.viewport = Some((0, 0, opts.width as i32, opts.height as i32));
        Some((m, end_time))
    } else {
        None
    };

    let mut gl = unsafe { get_internal_gl() };
    let mut frames_out = FrameReader::new(opts.width, opts.height);
    let mut last_log = Instant::now();
    let log_step = (frames / 10).max(1);
    let mut written = 0u64;

    // 1) Loading scene keeps its own local clock at 0..loading_secs.
    if let Some((m, load_time)) = loading_main.as_mut() {
        load_time.set(0.0);
        m.update()?;
        for frame in 0..load_frames {
            load_time.set(frame as f64 / opts.fps as f64);
            gl.quad_gl.render_pass(Some(mst.output().render_pass));
            clear_background(Color::new(0.0, 0.0, 0.0, 1.0));
            m.update()?;
            m.render(&mut painter)?;
            gl.flush();
            frames_out.push(internal_id(mst.output()), &mut stdin)?;
            written += 1;
            if frame % log_step == 0 || last_log.elapsed() > Duration::from_secs(1) {
                log_progress(&log_path, written, frames);
                last_log = Instant::now();
            }
        }
    }

    // 2) Gameplay.  Skip the 0.7s "Starting" animation so video frame 0 is
    // chart time 0 (and the mixed music starts at sample 0 on the same frame).
    let start_at = GameScene::BEFORE_TIME;
    time.set(start_at);
    main.update()?;
    for frame in 0..game_frames {
        time.set(start_at + frame as f64 / opts.fps as f64);
        gl.quad_gl
            .render_pass(Some(mst.output().render_pass));
        clear_background(Color::new(0.0, 0.0, 0.0, 1.0));
        main.update()?;
        main.render(&mut painter)?;
        gl.flush();
        frames_out.push(internal_id(mst.output()), &mut stdin)?;
        written += 1;
        if frame % log_step == 0 || last_log.elapsed() > Duration::from_secs(1) {
            log_progress(&log_path, written, frames);
            last_log = Instant::now();
        }
    }

    // 3) Ending.  EndingScene::enter seeks to -0.4s; local clock 0 is the
    // visible start of the results animation.
    if let Some((m, end_time)) = ending_main.as_mut() {
        end_time.set(0.0);
        m.update()?;
        for frame in 0..end_frames {
            end_time.set(frame as f64 / opts.fps as f64);
            gl.quad_gl.render_pass(Some(mst.output().render_pass));
            clear_background(Color::new(0.0, 0.0, 0.0, 1.0));
            m.update()?;
            m.render(&mut painter)?;
            gl.flush();
            frames_out.push(internal_id(mst.output()), &mut stdin)?;
            written += 1;
            if frame % log_step == 0 || last_log.elapsed() > Duration::from_secs(1) {
                log_progress(&log_path, written, frames);
                last_log = Instant::now();
            }
        }
    }

    frames_out.finish(&mut stdin)?;
    drop(frames_out);
    drop(gl);
    drop(loading_main);
    drop(ending_main);
    drop(main);
    drop(stdin);
    let status = child.wait()?;
    if !status.success() {
        bail!("ffmpeg exited with {status}; see {}", log_path.display());
    }

    let line = format!(
        "done in {:.1}s -> {}\n",
        started.elapsed().as_secs_f32(),
        opts.output.display()
    );
    let _ = std::fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
    eprintln!("{line}");
    Ok(())
}

struct RenderProfile {
    name: String,
    id: i32,
    rks: f32,
    avatar: Option<SafeTexture>,
}

/// Locate `data.json` by walking up from the chart path.  `--render` does not
/// run the normal `the_main` data initialization, so this is the only way to
/// know which account is currently logged in.
fn find_data_json(input: &Path) -> Option<PathBuf> {
    let mut dir = if input.is_dir() {
        input.to_path_buf()
    } else {
        input.parent()?.to_path_buf()
    };
    if !dir.is_absolute() {
        dir = std::env::current_dir().ok()?.join(dir);
    }
    loop {
        let candidate = dir.join("data.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

async fn load_render_profile(input: &Path) -> Option<RenderProfile> {
    let path = find_data_json(input)?;
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let me = value.get("me")?;
    let name = me.get("name")?.as_str()?.to_owned();
    let id = me.get("id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let rks = me.get("rks").and_then(|v| v.as_f64()).unwrap_or(0.) as f32;
    let avatar_url = me.get("avatar").and_then(|v| v.as_str());
    let token = value
        .get("tokens")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str());

    let avatar = avatar_url
        .and_then(|url| fetch_avatar_bytes(url, token))
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|image| SafeTexture::from(image).with_mipmap());
    Some(RenderProfile {
        name,
        id,
        rks,
        avatar,
    })
}

/// `--render` does not create the tokio runtime that the normal game sets up,
/// so reqwest's async client would panic with "no reactor running".  Fetch the
/// avatar bytes on a dedicated thread with its own runtime, then upload the
/// texture on the render thread where the GL context lives.
fn fetch_avatar_bytes(url: &str, token: Option<&str>) -> Option<Vec<u8>> {
    let url = url.to_owned();
    let token = token.map(|it| it.to_owned());
    std::thread::spawn(move || -> Option<Vec<u8>> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        rt.block_on(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .ok()?;
            let mut req = client.get(&url);
            if let Some(token) = token {
                req = req.header("Authorization", format!("Bearer {token}"));
            }
            let bytes = req.send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
            Some(bytes.to_vec())
        })
    })
    .join()
    .ok()
    .flatten()
}

fn log_progress(path: &Path, done: u64, total: u64) {
    let p = (done as f64 / total.max(1) as f64 * 100.0).round();
    let line = format!("progress={p:.0}% frame={done}/{total}\n");
    let _ = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

fn encoder_available(name: &str) -> bool {
    static NVENC: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if name != "h264_nvenc" {
        return false;
    }
    *NVENC.get_or_init(|| {
        ffmpeg_cmd()
            .args(["-hide_banner", "-encoders"])
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).contains("h264_nvenc"))
            .unwrap_or(false)
    })
}

/// Spawn this executable in render mode.  Used by the song page button so the
/// UI keeps running while the separate renderer owns its hidden GL context.
pub fn spawn_render(input: &str, output: &Path) -> Result<std::process::Child> {
    spawn_render_with(input, output, GameRenderOptions::default())
}

/// Same as [`spawn_render`], with the resolution/fps/audio choices made in the
/// song page's render panel.
pub fn spawn_render_with(input: &str, output: &Path, opts: GameRenderOptions) -> Result<std::process::Child> {
    // On mobile the "executable" is the APK's native library loaded by
    // app_process; spawning it again aborts with SIGABRT before ffmpeg is ever
    // reached.  Offline rendering is therefore desktop-only.
    #[cfg(any(target_os = "android", target_os = "ios"))]
    bail!("offline rendering is only supported on desktop");

    let exe = std::env::current_exe()?;
    // `local_path` may be stored relative to the data root; the child gets the
    // same cwd as this process, but resolve it explicitly so a copied/moved
    // build cannot silently fall back to the normal game UI.
    let input_path = Path::new(input);
    let input_abs = if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        // SongScene stores local charts as paths relative to data/charts
        // (`download/80201`, `custom/<id>`), not relative to the process cwd.
        let charts = crate::dir::charts()?;
        let candidate = Path::new(&charts).join(input_path);
        if candidate.exists() {
            candidate
        } else {
            std::env::current_dir()?.join(input_path)
        }
    };
    if !input_abs.exists() {
        bail!("chart path not found: {}", input_abs.display());
    }
    let output_abs = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()?.join(output)
    };
    if let Some(parent) = output_abs.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut cmd = Command::new(exe);
    cmd.arg("--render")
        .arg(&input_abs)
        .arg("--output")
        .arg(&output_abs)
        .arg("--fps")
        .arg(opts.fps.to_string())
        .arg("--width")
        .arg(opts.width.to_string())
        .arg("--height")
        .arg(opts.height.to_string())
        .arg("--crf")
        .arg(opts.crf.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if !opts.audio {
        cmd.arg("--no-audio");
    }
    cmd.arg("--backend").arg(opts.backend.as_arg());
    if opts.loading {
        cmd.arg("--loading").arg(format!("{:.2}", opts.loading_secs));
    } else {
        cmd.arg("--no-loading");
    }
    if opts.ending {
        cmd.arg("--ending").arg(format!("{:.2}", opts.ending_secs));
    } else {
        cmd.arg("--no-ending");
    }
    cmd.arg("--indicator").arg(if opts.indicator { "on" } else { "off" });
    cmd.arg("--watermark").arg(if opts.watermark { "on" } else { "off" });
    cmd.arg("--autoplay-style").arg(opts.autoplay_style.as_arg());
    cmd.arg("--autoplay-text").arg(&opts.autoplay_text);
    if let Some(aspect) = opts.aspect {
        cmd.arg("--aspect").arg(format!("{aspect:.5}"));
    } else {
        cmd.arg("--no-aspect");
    }
    if let Some(tip) = &opts.tip {
        if !tip.trim().is_empty() {
            cmd.arg("--tip").arg(tip);
        }
    }
    if let Some(res_pack) = &opts.res_pack {
        if !res_pack.trim().is_empty() {
            cmd.arg("--res-pack").arg(res_pack);
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let child = cmd.spawn().context("spawn renderer")?;
    #[cfg(target_os = "windows")]
    low_priority(&child);
    Ok(child)
}

/// Keep the renderer below the foreground game so it does not steal CPU time
/// from the UI while frames are being encoded.
#[cfg(target_os = "windows")]
fn low_priority(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    extern "system" {
        fn SetPriorityClass(handle: *mut std::ffi::c_void, class: u32) -> i32;
    }
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    unsafe {
        SetPriorityClass(child.as_raw_handle() as *mut _, BELOW_NORMAL_PRIORITY_CLASS);
    }
}

/// Read a resource pack's `info.yml` display name.  The respack list stores
/// directory names (often UUIDs), but the user-facing name lives in info.yml;
/// zip packs use the same layout inside their root directory.
pub(crate) fn respack_display_name(path: &str) -> Option<String> {
    let path = Path::new(path);
    let yaml = if path.is_dir() {
        std::fs::read_to_string(path.join("info.yml")).ok()?
    } else {
        let zip = ZipFileSystem::open(path).ok()?;
        let root = zip.root().to_owned();
        let mut archive = zip.0.lock().ok()?;
        let mut file = archive.by_name(&format!("{root}info.yml")).ok()?;
        let mut text = String::new();
        file.read_to_string(&mut text).ok()?;
        text
    };
    serde_yaml::from_str::<ResPackInfo>(&yaml).ok().map(|info| info.name)
}

/// Default output used by the in-game render button.
pub fn default_game_output(input: &str) -> PathBuf {
    let name = Path::new(input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("chart");
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("recordings")
        .join(format!("{name}_{stamp}.mp4"))
}

/// True while a render child process is being tracked by the UI process.
pub fn is_rendering() -> bool {
    RENDERING.load(Ordering::Relaxed)
}

pub fn set_rendering(v: bool) {
    RENDERING.store(v, Ordering::Relaxed);
}

static RENDERING: AtomicBool = AtomicBool::new(false);
