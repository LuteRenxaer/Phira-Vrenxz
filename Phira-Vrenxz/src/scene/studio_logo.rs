//! Studio Logo 开屏。纯黑背景 + 居中 Logo，持续 2 秒后过渡到下一个场景。

use prpr::{
    ext::SafeTexture,
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::Ui,
};
use anyhow::Result;
use macroquad::prelude::*;

const LOGO_DURATION: f32 = 5.0;
const FADE_TIME: f32 = 0.4;

pub struct StudioLogoScene {
    next_scene: Option<Box<dyn Scene>>,
    logo: Option<SafeTexture>,
    enter_time: f32,
    skip: bool,
}

impl StudioLogoScene {
    pub async fn new(next_scene: Box<dyn Scene>) -> Self {
        let logo = match load_texture("icons/Studio_Logo.png").await {
            Ok(tex) => Some(tex.into()),
            Err(e) => {
                warn!("failed to load Studio_Logo.png: {:?}", e);
                None
            }
        };
        Self {
            next_scene: Some(next_scene),
            logo,
            enter_time: f32::NAN,
            skip: false,
        }
    }
}

impl Scene for StudioLogoScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.enter_time.is_nan() {
            self.enter_time = tm.now() as f32;
        }
        Ok(())
    }

    fn update(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }

    fn touch(&mut self, _tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if touch.phase == TouchPhase::Started {
            self.skip = true;
            return Ok(true);
        }
        Ok(false)
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let t = tm.now() as f32;

        // 背景用 bg_camera
        set_camera(&ui.bg_camera());
        let full = ui.screen_rect();
        ui.fill_rect(full, BLACK);

        // UI 元素用 camera
        set_camera(&ui.camera());

        // 计算淡入淡出 alpha
        let elapsed = if self.enter_time.is_nan() { 0. } else { t - self.enter_time };
        let alpha = if elapsed < FADE_TIME {
            (elapsed / FADE_TIME).clamp(0., 1.)
        } else if elapsed > LOGO_DURATION - FADE_TIME {
            ((LOGO_DURATION - elapsed) / FADE_TIME).clamp(0., 1.)
        } else {
            1.
        };

        if let Some(logo) = &self.logo {
            // 居中显示 logo，最大宽度 1.2
            let max_w = 1.2f32;
            let ratio = logo.height() / logo.width();
            let w = max_w;
            let h = w * ratio;
            let r = Rect::new(-w / 2., -h / 2., w, h);
            let r_global = ui.rect_to_global(r);

            draw_texture_ex(
                **logo,
                r_global.x,
                r_global.y,
                Color::new(1., 1., 1., alpha),
                DrawTextureParams {
                    dest_size: Some(vec2(r_global.w, r_global.h)),
                    ..Default::default()
                },
            );
        } else {
            // 兜底：加载失败时显示文字
            ui.text("Studio Logo")
                .pos(0., 0.)
                .anchor(0.5, 0.5)
                .size(0.6)
                .color(Color::new(1., 1., 1., alpha))
                .draw();
        }

        // 底部提示：点击屏幕可跳过
        let tip_y = ui.top - 0.08;
        ui.text(crate::ttl!("studio-tap-to-skip"))
            .pos(0., tip_y)
            .anchor(0.5, 1.)
            .size(0.32)
            .color(Color::new(1., 1., 1., alpha * 0.6))
            .draw();

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        let timeout = !self.enter_time.is_nan() && tm.now() as f32 - self.enter_time >= LOGO_DURATION;
        if (self.skip || timeout) {
            if let Some(scene) = self.next_scene.take() {
                return NextScene::Replace(scene);
            }
        }
        NextScene::None
    }
}
