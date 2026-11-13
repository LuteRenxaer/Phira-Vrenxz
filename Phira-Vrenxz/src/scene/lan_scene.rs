use crate::lan::{LanManager, LanPanel};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{draw_parallelogram, poll_future, semi_white, LocalTask},
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{DRectButton, Ui},
};
use std::sync::{Arc, Mutex};
use prpr::scene::{GameMode, LoadingScene};

/// 局域网联机页：把 Firefly 的 LanPanel 包成可返回的独立 Scene。
pub struct LanScene {
    panel: LanPanel,
    manager: Arc<Mutex<LanManager>>,
    back_btn: DRectButton,
    next: Option<NextScene>,
    start_time: f32,
    launched: bool,
    launch_task: LocalTask<Result<NextScene>>,
}

impl LanScene {
    pub fn new() -> Self {
        let manager = Arc::new(Mutex::new(LanManager::new()));
        let mut panel = LanPanel::new(manager.clone());
        panel.show(0.);
        Self {
            panel,
            manager,
            back_btn: DRectButton::new(),
            next: None,
            start_time: 0.,
            launched: false,
            launch_task: None,
        }
    }
}

impl Scene for LanScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        self.start_time = tm.now() as f32;
        self.panel.show(self.start_time);
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.panel.update(tm.now() as f32)?;
        if let Some(task) = &mut self.launch_task {
            if let Some(result) = poll_future(task.as_mut()) {
                self.launch_task = None;
                match result {
                    Ok(scene) => self.next = Some(scene),
                    Err(err) => prpr::scene::show_error(err),
                }
            }
        }
        if !self.launched {
            let started = self.manager.lock().map(|m| m.is_started()).unwrap_or(false);
            if started {
                self.launched = true;
                let mut manager = self.manager.lock().unwrap();
                let local_path = if manager.is_host() {
                    manager.started_chart()
                } else {
                    let id = manager.started_chart().unwrap_or_default();
                    let relative = format!("download/{id}");
                    let dir = crate::dir::charts()?;
                    let full = format!("{dir}/{relative}");
                    if !std::path::Path::new(&full).exists() {
                        if let Err(err) = manager.download_host_chart() {
                            prpr::scene::show_error(err);
                            return Ok(());
                        }
                    }
                    Some(relative)
                };
                drop(manager);
                let Some(local_path) = local_path else {
                    prpr::scene::show_error(anyhow::anyhow!("房主未选择谱面"));
                    return Ok(());
                };
                self.launch_task = Some(Box::pin(async move {
                    let mut fs = crate::scene::fs_from_path(&local_path)?;
                    let info = prpr::fs::load_info(fs.as_mut()).await?;
                    let config = crate::get_data().config.clone();
                    let scene = LoadingScene::new(
                        GameMode::NoRetry,
                        info,
                        config,
                        fs,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await?;
                    Ok(NextScene::Overlay(Box::new(scene)))
                }));
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        ui.fill_rect(ui.screen_rect(), Color::new(0.04, 0.05, 0.07, 1.));
        self.panel.render(ui, tm.now() as f32);

        let r = Rect::new(-0.95, -ui.top + 0.03, 0.2, 0.075);
        self.back_btn.inner.set(ui, r);
        let v = if self.back_btn.inner.touching() { 0.3 } else { 0.16 };
        draw_parallelogram(r, None, Color::new(v, v, v + 0.05, 0.95), true);
        ui.text("返回")
            .pos(r.center().x, r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.3)
            .color(semi_white(0.9))
            .draw();
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if self.back_btn.touch(touch, tm.now() as f32) {
            self.next = Some(NextScene::Pop);
            return Ok(true);
        }
        Ok(self.panel.touch(touch, tm.now() as f32))
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next.take().unwrap_or_default()
    }
}
