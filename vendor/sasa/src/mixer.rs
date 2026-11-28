use std::sync::mpsc;

use crate::Renderer;

pub(crate) enum MixerCommand {
    AddRenderer(Box<dyn Renderer>),
}
pub(crate) struct Mixer {
    pub(crate) sample_rate: u32,

    renderers: Vec<Box<dyn Renderer>>,
    cons: mpsc::Receiver<MixerCommand>,
}

impl Mixer {
    pub(crate) fn new(sample_rate: u32, cons: mpsc::Receiver<MixerCommand>) -> Self {
        Self {
            sample_rate,

            renderers: Vec::new(),
            cons,
        }
    }

    fn consume_commands(&mut self) {
        while let Ok(cmd) = self.cons.try_recv() {
            match cmd {
                MixerCommand::AddRenderer(renderer) => self.renderers.push(renderer),
            }
        }
    }

    pub fn render_mono(&mut self, data: &mut [f32]) {
        self.consume_commands();
        data.fill(0.);
        if crate::global_paused() {
            return;
        }

        self.renderers.retain_mut(|renderer| {
            renderer.render_mono(self.sample_rate, data);
            renderer.alive()
        });
    }

    pub fn render_stereo(&mut self, data: &mut [f32]) {
        self.consume_commands();
        data.fill(0.);
        if crate::global_paused() {
            return;
        }

        self.renderers.retain_mut(|renderer| {
            renderer.render_stereo(self.sample_rate, data);
            renderer.alive()
        });
    }
}
