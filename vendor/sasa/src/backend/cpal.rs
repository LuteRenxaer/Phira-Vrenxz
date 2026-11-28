use crate::{mixer::Mixer, Backend};
use anyhow::{anyhow, Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, FromSample, OutputCallbackInfo, Sample, SampleFormat, Stream, StreamError, I24,
    U24,
};
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use super::{BackendSetup, State};

#[derive(Debug, Clone, Default)]
pub struct CpalSettings {
    pub preferred_sample_rate: Option<u32>,
    pub buffer_size: Option<u32>,
}

pub struct CpalBackend {
    settings: CpalSettings,
    stream: Option<Stream>,
    broken: Arc<AtomicBool>,
    state: Option<Arc<Mutex<State>>>,
}

impl CpalBackend {
    pub fn new(settings: CpalSettings) -> Self {
        Self {
            settings,
            stream: None,
            broken: Arc::default(),
            state: None,
        }
    }
}

fn fill_samples(
    mixer: &mut Mixer,
    dst: &mut [f32],
    channels: usize,
    stereo_scratch: &mut Vec<f32>,
) {
    if channels == 1 {
        mixer.render_mono(dst);
    } else if channels == 2 {
        mixer.render_stereo(dst);
    } else {
        // Render as stereo first, then expand per frame.
        // This keeps time progression tied to frame count, not channel count.
        stereo_scratch.resize(dst.len() / channels * 2, 0.0);
        mixer.render_stereo(stereo_scratch);

        for (stereo, frame) in stereo_scratch
            .chunks_exact(2)
            .zip(dst.chunks_exact_mut(channels))
        {
            frame[0] = stereo[0];
            frame[1] = stereo[1];
            frame[2..].fill(0.0);
        }
    }
}

fn write_output<T: Sample + FromSample<f32>>(
    state: &mut State,
    channels: usize,
    data: &mut [T],
    info: &OutputCallbackInfo,
    stereo_scratch: &mut Vec<f32>,
    out_scratch: &mut Vec<f32>,
) {
    let sample_count = data.len() / channels * channels;
    let (data, tail) = data.split_at_mut(sample_count);
    tail.fill(T::from_sample(0.0));

    out_scratch.resize(sample_count, 0.0);
    fill_samples(&mut state.mixer, out_scratch, channels, stereo_scratch);

    for (dst, src) in data.iter_mut().zip(out_scratch.iter().copied()) {
        *dst = T::from_sample(src);
    }

    let ts = info.timestamp();
    if let Some(delay) = ts.playback.duration_since(&ts.callback) {
        state.recorder.push(delay.as_secs_f32());
    }
}

fn write_output_f32(
    state: &mut State,
    channels: usize,
    data: &mut [f32],
    info: &OutputCallbackInfo,
    stereo_scratch: &mut Vec<f32>,
) {
    let sample_count = data.len() / channels * channels;
    let (data, tail) = data.split_at_mut(sample_count);
    tail.fill(0.0);

    fill_samples(&mut state.mixer, data, channels, stereo_scratch);

    let ts = info.timestamp();
    if let Some(delay) = ts.playback.duration_since(&ts.callback) {
        state.recorder.push(delay.as_secs_f32());
    }
}

fn make_error_callback(broken: Arc<AtomicBool>) -> impl FnMut(StreamError) + Send + 'static {
    move |err| {
        eprintln!("audio error: {err:?}");
        if matches!(err, StreamError::DeviceNotAvailable) {
            broken.store(true, Ordering::Relaxed);
        }
    }
}

impl Backend for CpalBackend {
    fn setup(&mut self, setup: BackendSetup) -> Result<()> {
        self.state = Some(Arc::new(Mutex::new(setup.into())));
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no default output device is found"))?;
        let mut supported: Vec<_> = device
            .supported_output_configs()
            .context("cannot get supported output configs")?
            .collect();
        supported.sort_by(|a, b| b.cmp_default_heuristics(a));
        let range = supported
            .first()
            .context("no supported output config is found")?;
        let sample_rate = self
            .settings
            .preferred_sample_rate
            .map_or(range.max_sample_rate(), |r| {
                r.clamp(range.min_sample_rate(), range.max_sample_rate())
            });
        let supported_config = range.with_sample_rate(sample_rate);

        let sample_format = supported_config.sample_format();
        let mut config = supported_config.config();
        config.buffer_size = self
            .settings
            .buffer_size
            .map_or(BufferSize::Default, BufferSize::Fixed);
        let channels = config.channels.max(1) as usize;

        let state = Arc::clone(self.state.as_ref().unwrap());
        state.lock().mixer.sample_rate = config.sample_rate;

        macro_rules! build_stream {
            ($sample_ty:ty) => {{
                let state = Arc::clone(&state);
                let mut stereo_scratch = Vec::new();
                let mut out_scratch = Vec::new();
                device.build_output_stream(
                    &config,
                    move |data: &mut [$sample_ty], info: &OutputCallbackInfo| {
                        write_output(
                            &mut state.lock(),
                            channels,
                            data,
                            info,
                            &mut stereo_scratch,
                            &mut out_scratch,
                        );
                    },
                    make_error_callback(Arc::clone(&self.broken)),
                    None,
                )
            }};
        }

        let stream = match sample_format {
            SampleFormat::I8 => build_stream!(i8),
            SampleFormat::I16 => build_stream!(i16),
            SampleFormat::I24 => build_stream!(I24),
            SampleFormat::I32 => build_stream!(i32),
            SampleFormat::I64 => build_stream!(i64),
            SampleFormat::U8 => build_stream!(u8),
            SampleFormat::U16 => build_stream!(u16),
            SampleFormat::U24 => build_stream!(U24),
            SampleFormat::U32 => build_stream!(u32),
            SampleFormat::U64 => build_stream!(u64),
            SampleFormat::F32 => {
                let state = Arc::clone(&state);
                let mut stereo_scratch = Vec::new();
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], info: &OutputCallbackInfo| {
                        write_output_f32(
                            &mut state.lock(),
                            channels,
                            data,
                            info,
                            &mut stereo_scratch,
                        );
                    },
                    make_error_callback(Arc::clone(&self.broken)),
                    None,
                )
            }
            SampleFormat::F64 => build_stream!(f64),
            _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
        }
        .context("failed to build stream")?;

        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    fn consume_broken(&self) -> bool {
        self.broken.fetch_and(false, Ordering::Relaxed)
    }
}
