mod music;
pub use music::{Music, MusicParams};

mod sfx;
pub use sfx::{PlaySfxParams, Sfx};

pub trait Renderer: Send {
    fn alive(&self) -> bool;
    fn render_mono(&mut self, sample_rate: u32, data: &mut [f32]);
    fn render_stereo(&mut self, sample_rate: u32, data: &mut [f32]);
}
