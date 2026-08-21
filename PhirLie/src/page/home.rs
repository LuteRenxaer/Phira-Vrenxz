prpr_l10n::tl_file!("home");
use super::{
    load_font_with_cksum, set_bold_font, CharacterPage, LibraryPage, NextPage, Page, ResPackPage, SFader, SettingsPage, SharedState,
    BOLD_FONT_CKSUM,
};
use super::message::MessagePage;
use crate::{
    anim::Anim,
    client::{recv_raw, Character, Client, LoginParams, User, UserManager},
    dir, get_data, get_data_mut,
    icons::Icons,
    login::Login,
    save_data,
    scene::{check_read_tos_and_policy, ProfileScene, JUST_LOADED_TOS},
    sync_data,
    threed::ThreeD,
};
use ::rand::{random, thread_rng, Rng};
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use image::DynamicImage;
use macroquad::prelude::*;
use prpr::{
    ext::{open_url, semi_black, semi_white, RectExt, SafeTexture, ScaleType},
    info::ChartInfo,
    scene::{show_error, NextScene},
    task::Task,
    ui::{clip_rounded_rect, ClipType, DRectButton, Dialog, FontArc, RectButton, Scroll, Ui, UI_AUDIO},
};
use prpr_l10n::LANG_IDENTS;
use reqwest::StatusCode;
use sasa::{AudioClip, PlaySfxParams, Sfx};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{atomic::Ordering, Arc},
};
use tap::Tap;
use tracing::{info, warn};
use once_cell::sync::Lazy;

const BOARD_SWITCH_TIME: f32 = 4.;
const BOARD_TRANSIT_TIME: f32 = 1.2;

type BoldFontUpdateTask = Task<Result<Option<(FontArc, String)>>>;

/// 模型文件夹名 → 语音角色名的映射（null 表示该角色无语音）。
/// 从 assets/skel/models.json 加载，新增模型时只需在 JSON 中添加条目。
static VOICE_MAP: Lazy<HashMap<String, Option<String>>> = Lazy::new(|| {
    let json = include_str!("../../../assets/skel/models.json");
    serde_json::from_str(json).unwrap_or_else(|e| {
        warn!("failed to parse models.json: {e}");
        HashMap::new()
    })
});

/// 根据模型文件夹路径推断语音角色名。
/// 1. 默认模型（None）→ Hoshino
/// 2. 优先从 models.json 映射查找
/// 3. 回退到硬编码推断（hoshino/shiroko/kuchinashi）
/// 返回 None 表示该角色没有语音
fn resolve_voice_character(model_path: Option<&str>) -> Option<String> {
    let folder = match model_path {
        None => return Some("Hoshino".to_string()),
        Some(p) => std::path::Path::new(p)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string(),
    };
    // 优先从 JSON 映射查找
    if let Some(mapped) = VOICE_MAP.get(&folder) {
        return mapped.clone();
    }
    // 回退：硬编码推断
    let lower = folder.to_lowercase();
    if lower.contains("hoshino") {
        Some("Hoshino".to_string())
    } else if lower.contains("shiroko") {
        Some("Shiroko".to_string())
    } else if lower.contains("kuchinashi") {
        None
    } else if folder.is_empty() {
        None
    } else {
        Some(folder)
    }
}

/// 加载指定角色的语音文件（普通语音 + _mo 抚摸音效），从 voice_Line/{角色}/ 目录读取。
async fn load_voice_files(voice_char: Option<&str>) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut normal = Vec::new();
    let mut pet = Vec::new();
    let Some(prefix) = voice_char else { return (normal, pet); };
    for i in 1..=5 {
        if let Ok(data) = crate::load_file(&format!("voice_Line/{}/{}{}.mp3", prefix, prefix, i)).await {
            normal.push(data);
        }
        if let Ok(data) = crate::load_file(&format!("voice_Line/{}/{}{}_mo.mp3", prefix, prefix, i)).await {
            pet.push(data);
        }
    }
    (normal, pet)
}

#[derive(Deserialize)]
struct Version {
    version: semver::Version,
    date: NaiveDate,
    description: String,
    url: String,
}

pub struct HomePage {
    icons: Arc<Icons>,

    btn_play: DRectButton,
    btn_respack: DRectButton,
    btn_settings: DRectButton,
    btn_user: DRectButton,
    btn_message: DRectButton,
    btn_character: DRectButton,

    next_page: Option<NextPage>,

    login: Login,
    update_task: Option<Task<Result<User>>>,

    need_back: bool,
    sf: SFader,
    enter_time: f32,

    board_task: Option<Task<Result<Option<DynamicImage>>>>,
    board_last_time: f32,
    board_last: Option<String>,
    board_tex_last: Option<SafeTexture>,
    board_tex: Option<SafeTexture>,
    board_dir: bool,

    has_new_task: Option<Task<Result<bool>>>,
    has_new: bool,

    check_update_task: Option<Task<Result<Option<Version>>>>,
    check_bold_font_update_task: Option<BoldFontUpdateTask>,

    btn_play_3d: ThreeD,

    character: Character,
    char_appear_p: Anim<f32>,
    char_last_illu: Option<String>,
    char_last_user_id: Option<i32>,
    char_fetch_task: Option<Task<Result<Character>>>,
    char_illu: Option<SafeTexture>,
    char_illu_task: Option<Task<Result<DynamicImage>>>,

    char_screen_p: Anim<f32>,
    char_btn: RectButton,
    char_text_start: f32,
    char_cached_size: f32,
    char_scroll: Scroll,
    char_edit_btn: RectButton,

    spine_model: Option<crate::spine_model::SpineModel>,
    spine_raw_task: Option<Task<Result<crate::spine_model::SpineModelRawData>>>,
    spine_alpha: Anim<f32>,
    spine_petting: bool,
    spine_anim_dirty: bool,
    spine_touch_id: Option<u64>,
    spine_loaded_path: Option<String>,

    // 闲置语音：每60秒随机播放一个 voice_Line 目录下的音效
    voice_sfx: Vec<(Sfx, f32)>,
    // 抚摸头音效：文件名带 _mo 后缀
    pet_sfx: Vec<(Sfx, f32)>,
    voice_load_task: Option<Task<Result<(Vec<Vec<u8>>, Vec<Vec<u8>>)>>>,
    voice_timer: f32,
    voice_smile_timer: f32,

    #[cfg(feature = "hykb")]
    beian_btn: RectButton,
}

impl HomePage {
    pub async fn new() -> Result<Self> {
        let update_task = if get_data().config.offline_mode {
            None
        } else if let Some(u) = &get_data().me {
            UserManager::request(u.id);
            Some(Task::new(async {
                Client::login(LoginParams::RefreshToken {
                    token: &get_data().tokens.as_ref().unwrap().1,
                })
                .await?;
                let me = Client::get_me().await?;
                #[cfg(feature = "hykb")]
                crate::obtain_hykb_credential_silent().await?.ok_or_err()?;
                Ok(me)
            }))
        } else {
            None
        };

        let flavor = match load_file("flavor").await.map(String::from_utf8) {
            Ok(Ok(flavor)) => flavor.trim().to_owned(),
            _ => "none".to_owned(),
        };

        let icons = Arc::new(Icons::new().await?);
        let mut res = Self {
            icons: Arc::clone(&icons),

            btn_play: DRectButton::new().with_delta(-0.01),
            btn_respack: DRectButton::new().with_delta(-0.004).with_elevation(0.).no_sound(),
            btn_settings: DRectButton::new().with_delta(-0.004).with_elevation(0.),
            btn_user: DRectButton::new().with_delta(-0.003),
            btn_message: DRectButton::new().with_delta(-0.004).with_elevation(0.),
            btn_character: DRectButton::new().with_delta(-0.004).with_elevation(0.),

            next_page: None,

            login: Login::new(icons),
            update_task,

            need_back: false,
            sf: SFader::new(),
            enter_time: f32::NAN,

            board_task: None,
            board_last_time: f32::NEG_INFINITY,
            board_last: None,
            board_tex_last: None,
            board_tex: None,
            board_dir: false,

            has_new_task: None,
            has_new: false,

            check_update_task: Some(Task::new(async move {
                Ok(recv_raw(Client::get("/check-update").query(&[("version", env!("CARGO_PKG_VERSION")), ("flavor", &flavor)]))
                    .await?
                    .json()
                    .await?)
            })),
            check_bold_font_update_task: {
                let cksum = BOLD_FONT_CKSUM.with(|it| it.borrow().clone());
                Some(Task::new(async move {
                    let resp = Client::get("/font-bold")
                        .query(&[("cksum", cksum)])
                        .query(&[("new_bold_font", "true")])
                        .send()
                        .await?;
                    if resp.status() == StatusCode::NOT_MODIFIED {
                        info!("bold font not modified");
                        return Ok(None);
                    }
                    if !resp.status().is_success() {
                        let status = resp.status().as_str().to_owned();
                        let text = resp.text().await.context("failed to receive text")?;
                        if let Ok(what) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(detail) = what["error"].as_str() {
                                bail!("request failed ({status}): {detail}");
                            }
                        }
                        bail!("request failed ({status}): {text}");
                    }
                    info!("downloading new bold font");
                    let bytes = resp.bytes().await?;
                    std::fs::write(dir::bold_font_path()?, &bytes).context("failed to save font")?;
                    Ok(Some(load_font_with_cksum(bytes.to_vec())?))
                }))
            },

            btn_play_3d: ThreeD::new().tap_mut(|it| {
                it.anchor = vec2(0.3, 0.);
                it.angle = 0.2;
                it.sync();
            }),

            character: get_data().character.clone().unwrap_or_default(),
            char_appear_p: Anim::new(0.),
            char_last_illu: None,
            char_last_user_id: None,
            char_fetch_task: None,
            char_illu: None,
            char_illu_task: None,
            char_screen_p: Anim::new(0.),
            char_btn: RectButton::new(),
            char_text_start: 0.,
            char_cached_size: 0.,
            char_scroll: Scroll::new().use_clip(ClipType::Clip),
            char_edit_btn: RectButton::new(),

            spine_model: None,
            spine_raw_task: {
                let model_path = get_data().config.character_model_path.clone();
                Some(Task::new(async move {
                    if let Some(dir) = model_path {
                        crate::spine_model::load_model_from_dir(&dir).await
                    } else {
                        crate::spine_model::load_default_model_data().await
                    }
                }))
            },
            spine_alpha: Anim::new(0.),
            spine_petting: false,
            spine_anim_dirty: true,
            spine_touch_id: None,
            spine_loaded_path: None,

            voice_sfx: Vec::new(),
            pet_sfx: Vec::new(),
            voice_load_task: {
                let model_path = get_data().config.character_model_path.clone();
                let voice_char = resolve_voice_character(model_path.as_deref());
                Some(Task::new(async move {
                    let (normal, pet) = load_voice_files(voice_char.as_deref()).await;
                    Ok((normal, pet))
                }))
            },
            voice_timer: 0.,
            voice_smile_timer: 0.,

            #[cfg(feature = "hykb")]
            beian_btn: RectButton::new(),
        };
        res.load_char_illu();

        Ok(res)
    }

    fn load_char_illu(&mut self) {
        let key = if self.character.illust == "@" {
            format!("@{}", self.character.id)
        } else {
            self.character.illust.clone()
        };
        if self.char_last_illu.as_ref() == Some(&key) {
            return;
        }
        self.char_last_illu = Some(key);

        self.char_appear_p.set(0.);

        #[cfg(closed)]
        if self.character.illust == "@" {
            let id = self.character.id.clone();
            self.char_illu_task =
                Some(Task::new(
                    async move { Ok(image::load_from_memory(&crate::inner::resolve_data(load_file(&format!("res/{id}.char")).await?))?) },
                ));
        } else {
            let file = crate::page::File {
                url: self.character.illust.clone(),
            };
            self.char_illu_task =
                Some(Task::new(async move { Ok(image::load_from_memory(&crate::inner::resolve_data(file.fetch().await?.to_vec()))?) }));
        }
    }

    fn fetch_has_new(&mut self) {
        if get_data().config.offline_mode || get_data().me.is_none() || get_data().tokens.is_none() {
            self.has_new_task = None;
            self.has_new = false;
            return;
        }
        let time = get_data().message_check_time.unwrap_or_default();
        self.has_new_task = Some(Task::new(async move {
            #[derive(Deserialize)]
            struct Resp {
                has: bool,
            }
            let resp: Resp = recv_raw(Client::get("/message/has_new").query(&[("checked", time)]))
                .await?
                .json()
                .await?;
            Ok(resp.has)
        }));
    }

    fn appear(&self, rt: f32, delay: f32, duration: f32) -> f32 {
        if self.enter_time.is_nan() || get_data().prefer_reduced_motion {
            return 1.;
        }
        let p = ((rt - self.enter_time - delay) / duration).clamp(0., 1.);
        1. - (1. - p).powi(3)
    }

    fn render_not_char(&mut self, ui: &mut Ui, s: &mut SharedState) {
        let t = s.t;
        let rt = s.rt;

        let pad = 0.04;

        // 先计算各按钮的出现动画参数(x/y 偏移 + 透明度)
        let p_play = self.appear(rt, 0.0, 0.28);
        let p_msg = self.appear(rt, 0.0, 0.26);
        let p_set = self.appear(rt, 0.0, 0.26);
        let p_res = self.appear(rt, 0.0, 0.26);
        let (dx_play, dy_play) = (-0.22 * (1. - p_play), 0.06 * (1. - p_play));
        let (dx_msg, dy_msg) = (-0.22 * (1. - p_msg), 0.05 * (1. - p_msg));
        let (dx_set, dy_set) = (0.18 * (1. - p_set), 0.05 * (1. - p_set));
        let (dx_res, dy_res) = (0.18 * (1. - p_res), 0.05 * (1. - p_res));

        let config = &get_data().config;
        let r = Rect::new(-0.83 + config.home_play_offset_x, -0.33 + config.home_play_offset_y, 0.83, 0.45);
        let mat = self.btn_play_3d.now(ui, r, t);
        ui.with_gl(mat, |ui| {
            s.render_fader(ui, |ui| {
                ui.scope(|ui| {
                    ui.dx(dx_play);
                    ui.dy(dy_play);
                    ui.alpha(p_play, |ui| {
                        let rad = self.btn_play.config.radius;
                        self.btn_play.render_shadow(ui, r, t, |ui, path| {
                            ui.fill_path(&path, semi_black(0.4));
                            if let Some(cur) = &self.board_tex {
                                let p = (t - self.board_last_time) / BOARD_TRANSIT_TIME;
                                if p > 1. {
                                    self.board_tex_last = None;
                                    ui.fill_path(&path, (**cur, r));
                                } else if let Some(last) = &self.board_tex_last {
                                    let (cur, last) = if self.board_dir { (last, cur) } else { (cur, last) };
                                    let p = 1. - (1. - p).powi(3);
                                    let p = if self.board_dir { 1. - p } else { p };
                                    clip_rounded_rect(ui, r, rad, |ui| {
                                        let mut nr = r;
                                        nr.h = r.h * (1. - p);
                                        ui.fill_rect(nr, (**last, nr));

                                        nr.h = r.h * p;
                                        nr.y = r.bottom() - nr.h;
                                        ui.fill_rect(nr, (**cur, nr));
                                    });
                                } else {
                                    ui.fill_path(&path, (**cur, r, ScaleType::CropCenter, semi_white(p)));
                                }
                            }
                            ui.fill_path(&path, (semi_black(0.7), (r.x, r.y), Color::default(), (r.x + 0.6, r.y)));
                            ui.text(tl!("play")).pos(r.x + pad, r.y + pad).draw();
                            let r = Rect::new(r.x + 0.02, r.bottom() - 0.18, 0.17, 0.17);
                            ui.fill_rect(r, (*self.icons.play, r, ScaleType::Fit, semi_white(0.6)));
                        });
                    });
                });
            });
        });

        let btn_size = 0.12;
        let margin = 0.055;
        let bottom_y = ui.top - btn_size - margin + config.home_menu_offset_y;

        let right_x = 0.95 - btn_size * 2.0 - 0.02 + config.home_menu_offset_x;

        let r_res = Rect::new(right_x, bottom_y, btn_size, btn_size);
        s.render_fader(ui, |ui| {
            ui.scope(|ui| {
                ui.dx(dx_res);
                ui.dy(dy_res);
                ui.alpha(p_res, |ui| {
                    self.btn_respack.render_shadow(ui, r_res, t, |ui, path| {
                        ui.fill_path(&path, semi_black(0.2));
                        let ir = r_res.feather(-0.02);
                        ui.fill_rect(ir, (*self.icons.respack, ir, ScaleType::Fit));
                    });
                });
            });
        });

        let r_set = Rect::new(r_res.right() + 0.02, bottom_y, btn_size, btn_size);
        s.render_fader(ui, |ui| {
            ui.scope(|ui| {
                ui.dx(dx_set);
                ui.dy(dy_set);
                ui.alpha(p_set, |ui| {
                    self.btn_settings.render_shadow(ui, r_set, t, |ui, path| {
                        ui.fill_path(&path, semi_black(0.2));
                        let ir = r_set.feather(-0.02);
                        ui.fill_rect(ir, (*self.icons.settings, ir, ScaleType::Fit));
                    });
                });
            });
        });

        if config.show_character {
            let r_char = Rect::new(right_x - btn_size - 0.02, bottom_y, btn_size, btn_size);
            s.render_fader(ui, |ui| {
                ui.scope(|ui| {
                    ui.dx(dx_res);
                    ui.dy(dy_res);
                    ui.alpha(p_res, |ui| {
                        self.btn_character.render_shadow(ui, r_char, t, |ui, path| {
                            ui.fill_path(&path, semi_black(0.2));
                            let ir = r_char.feather(-0.02);
                            ui.fill_rect(ir, (*self.icons.character, ir, ScaleType::Fit));
                        });
                    });
                });
            });
        }

        let left_x = -0.95 + config.home_menu_offset_x;

        let r_msg = Rect::new(left_x, bottom_y, btn_size, btn_size);
        s.render_fader(ui, |ui| {
            ui.scope(|ui| {
                ui.dx(dx_msg);
                ui.dy(dy_msg);
                ui.alpha(p_msg, |ui| {
                    self.btn_message.render_shadow(ui, r_msg, t, |ui, path| {
                        ui.fill_path(&path, semi_black(0.2));
                        let ir = r_msg.feather(-0.02);
                        ui.fill_rect(ir, (*self.icons.msg, ir, ScaleType::Fit));
                        if self.has_new {
                            let dot = Rect::new(ir.right() - 0.025, ir.y - 0.015, 0.035, 0.035);
                            ui.fill_path(&dot.rounded(0.0175), RED);
                        }
                    });
                });
            });
        });
    }
}

impl Page for HomePage {
    fn label(&self) -> Cow<'static, str> {
        "PHIRLIE".into()
    }

    fn enter(&mut self, s: &mut SharedState) -> Result<()> {
        if self.need_back {
            self.sf.enter(s.t);
            self.need_back = false;
        }
        if self.enter_time.is_nan() {
            self.enter_time = s.rt;
        }
        // 角色模型配置变化时重新加载
        let current_path = get_data().config.character_model_path.clone();
        if self.spine_loaded_path != current_path {
            self.spine_model = None;
            let path = current_path.clone();
            self.spine_raw_task = Some(Task::new(async move {
                if let Some(dir) = path {
                    crate::spine_model::load_model_from_dir(&dir).await
                } else {
                    crate::spine_model::load_default_model_data().await
                }
            }));
            self.spine_anim_dirty = true;

            // 同时重新加载对应角色的语音
            let voice_char = resolve_voice_character(current_path.as_deref());
            self.voice_sfx.clear();
            self.pet_sfx.clear();
            self.voice_load_task = Some(Task::new(async move {
                let (normal, pet) = load_voice_files(voice_char.as_deref()).await;
                Ok((normal, pet))
            }));
        }
        self.fetch_has_new();
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        if self.sf.transiting() {
            return Ok(true);
        }
        #[cfg(feature = "hykb")]
        if self.update_task.is_some() {
            return Ok(true);
        }
        let t = s.t;
        let rt = s.rt;
        if self.login.touch(touch, s.t) {
            return Ok(true);
        }
        if self.char_screen_p.now(rt) < 1e-2 {
            self.btn_play_3d.touch(touch, t);
            if self.btn_play.touch(touch, t) {
                self.next_page = Some(NextPage::Overlay(Box::new(LibraryPage::new(Arc::clone(&self.icons), s.icons.clone())?)));
                return Ok(true);
            }
            if self.btn_respack.touch(touch, t) {
                self.next_page = Some(NextPage::Overlay(Box::new(ResPackPage::new(Arc::clone(&self.icons))?)));
                return Ok(true);
            }
            if self.btn_settings.touch(touch, t) {
                self.next_page = Some(NextPage::Overlay(Box::new(SettingsPage::new(self.icons.icon.clone(), self.icons.lang.clone()))));
                return Ok(true);
            }
            if get_data().config.show_character && self.btn_character.touch(touch, t) {
                self.next_page = Some(NextPage::Overlay(Box::new(CharacterPage::new())));
                return Ok(true);
            }
            if self.btn_message.touch(touch, t) {
                self.next_page = Some(NextPage::Overlay(Box::new(MessagePage::new(
                    Arc::clone(&self.icons),
                    s.icons.clone(),
                ))));
                return Ok(true);
            }
        } else {
            if self.char_scroll.touch(touch, t) {
                return Ok(true);
            }
            if self.char_edit_btn.touch(touch) {
                let _ = open_url("https://phira.moe/settings/account");
            }
        }
        if self.btn_user.touch(touch, t) {
            if let Some(me) = &get_data().me {
                self.need_back = true;
                self.sf.goto(t, ProfileScene::new(me.id, self.icons.user.clone(), s.icons.clone()));
            } else {
                self.login.enter(t);
            }
            return Ok(true);
        }
        #[cfg(feature = "hykb")]
        if self.beian_btn.touch(touch) {
            let _ = open_url("https://beian.miit.gov.cn/#/home");
            return Ok(true);
        }

        if self.spine_model.is_some() && get_data().config.show_character {
            let config = &get_data().config;
            let model_cx = 0.50 + config.character_model_offset_x;
            let model_cy = 0.48 + config.character_model_offset_y;
            let head_rect = Rect::new(
                model_cx + config.character_pet_offset_x - config.character_pet_width / 2.,
                model_cy + config.character_pet_offset_y - config.character_pet_height / 2.,
                config.character_pet_width,
                config.character_pet_height,
            );
            let inside = head_rect.contains(touch.position);
            match touch.phase {
                TouchPhase::Started => {
                    if inside {
                        self.spine_touch_id = Some(touch.id);
                        self.spine_petting = true;
                        self.spine_anim_dirty = true;
                        // 播放抚摸头音效（_mo），随机选一个
                        if !self.pet_sfx.is_empty() {
                            let idx = thread_rng().gen_range(0..self.pet_sfx.len());
                            let (sfx, duration) = &mut self.pet_sfx[idx];
                            if let Ok(()) = sfx.play(PlaySfxParams::default()) {
                                self.voice_smile_timer = self.voice_smile_timer.max(*duration);
                            }
                        }
                        return Ok(true);
                    }
                }
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if self.spine_touch_id == Some(touch.id) {
                        if self.spine_petting && !inside {
                            self.spine_petting = false;
                            self.spine_anim_dirty = true;
                        } else if !self.spine_petting && inside {
                            self.spine_petting = true;
                            self.spine_anim_dirty = true;
                        }
                        return Ok(true);
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if self.spine_touch_id == Some(touch.id) {
                        self.spine_touch_id = None;
                        self.spine_petting = false;
                        self.spine_anim_dirty = true;
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let rtime = s.rt;
        #[cfg(feature = "hykb")]
        if get_data().me.is_none() {
            self.login.force(t);
        }
        self.login.update(t)?;

        // Spine 模型加载：先异步加载原始数据，再在主线程创建 GPU 资源
        if let Some(task) = &mut self.spine_raw_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(data) => {
                        match crate::spine_model::SpineModel::from_memory(
                            &data.atlas_data, &data.skel_data, &data.textures, "Idle_01"
                        ) {
                            Ok(model) => {
                                self.spine_model = Some(model);
                                self.spine_loaded_path = get_data().config.character_model_path.clone();
                                self.spine_alpha.goto(1., rtime, 0.6);
                                if let Some(m) = &mut self.spine_model {
                                    m.set_expression(get_data().config.character_default_expr);
                                }
                            }
                            Err(err) => {
                                warn!(?err, "failed to create spine model");
                            }
                        }
                    }
                    Err(err) => {
                        warn!(?err, "failed to load spine model data");
                    }
                }
                self.spine_raw_task = None;
            }
        }
        if get_data().config.show_character {
            if let Some(model) = &mut self.spine_model {
                model.update(get_frame_time());
                let target_expr = if self.spine_petting || self.voice_smile_timer > 0. {
                    get_data().config.character_pet_expr
                } else {
                    get_data().config.character_default_expr
                };
                if self.spine_anim_dirty {
                    model.set_expression(target_expr);
                    self.spine_anim_dirty = false;
                }
            }
        }

        if let Some(task) = &mut self.voice_load_task {
            if let Some(res) = task.take() {
                match res {
                    Ok((normal_files, pet_files)) => {
                        // 普通闲置语音
                        for data in normal_files {
                            match AudioClip::new(data) {
                                Ok(clip) => {
                                    let duration = clip.length() as f32;
                                    match UI_AUDIO.with(|it| it.borrow_mut().create_sfx(clip, None)) {
                                        Ok(sfx) => self.voice_sfx.push((sfx, duration)),
                                        Err(e) => warn!("failed to create voice sfx: {e:?}"),
                                    }
                                }
                                Err(e) => warn!("failed to create voice audio clip: {e:?}"),
                            }
                        }
                        // 抚摸头音效
                        for data in pet_files {
                            match AudioClip::new(data) {
                                Ok(clip) => {
                                    let duration = clip.length() as f32;
                                    match UI_AUDIO.with(|it| it.borrow_mut().create_sfx(clip, None)) {
                                        Ok(sfx) => self.pet_sfx.push((sfx, duration)),
                                        Err(e) => warn!("failed to create pet sfx: {e:?}"),
                                    }
                                }
                                Err(e) => warn!("failed to create pet audio clip: {e:?}"),
                            }
                        }
                    }
                    Err(e) => warn!("failed to load voice files: {e:?}"),
                }
                self.voice_load_task = None;
            }
        }
        if get_data().config.show_character && !self.voice_sfx.is_empty() {
            let dt = get_frame_time();
            self.voice_timer += dt;
            if self.voice_smile_timer > 0. {
                self.voice_smile_timer -= dt;
                if self.voice_smile_timer <= 0. {
                    self.voice_smile_timer = 0.;
                    self.spine_anim_dirty = true; // 语音结束，切回表情 00
                }
            }
            if self.voice_timer >= 60. {
                self.voice_timer = 0.;
                let idx = thread_rng().gen_range(0..self.voice_sfx.len());
                let (sfx, duration) = &mut self.voice_sfx[idx];
                if let Ok(()) = sfx.play(PlaySfxParams::default()) {
                    self.voice_smile_timer = *duration;
                    self.spine_anim_dirty = true; // 播放开始，切到表情 25
                }
            }
        }
        let current_user = Some(get_data().me.as_ref().map_or(-1, |it| it.id));
        self.char_scroll.update(t);
        if self.char_last_user_id != current_user {
            let locale = get_data().language.clone().unwrap_or(LANG_IDENTS[0].to_string());
            self.char_last_user_id = current_user;
            if get_data().config.offline_mode || get_data().me.is_none() || get_data().tokens.is_none() {
                self.char_fetch_task = None;
            } else {
                self.char_fetch_task =
                    Some(Task::new(async move { Ok(recv_raw(Client::get("/me/char").query(&[("locale", locale)])).await?.json().await?) }));
            }
        }
        if let Some(task) = &mut self.update_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        if format!("{err:?}").contains("invalid token") {
                            get_data_mut().me = None;
                            get_data_mut().tokens = None;
                            let _ = save_data();
                            sync_data();
                        }
                        show_error(err.context(tl!("failed-to-update") + "\n" + tl!("note-try-login-again")));
                    }
                    Ok(val) => {
                        get_data_mut().me = Some(val);
                        save_data()?;
                    }
                }
                self.update_task = None;
            }
        }
        if self.board_task.is_none() && t - self.board_last_time > BOARD_SWITCH_TIME {
            let charts = &get_data().charts;
            let last_index = self
                .board_last
                .as_ref()
                .and_then(|path| charts.iter().position(|it| &it.local_path == path));
            if charts.is_empty() || (charts.len() == 1 && last_index.is_some()) {
                self.board_task = Some(Task::new(async move { Ok(None) }));
            } else {
                let mut index = thread_rng().gen_range(0..(charts.len() - last_index.is_some() as usize));
                if last_index.is_some_and(|it| it <= index) {
                    index += 1;
                }
                let path = charts[index].local_path.clone();
                let dir = prpr::dir::Dir::new(format!("{}/{}", dir::charts()?, path))?;
                self.board_last = Some(path);
                self.board_task = Some(Task::new(async move {
                    let info: ChartInfo = serde_yaml::from_reader(dir.open("info.yml")?)?;
                    let bytes = dir.read(info.illustration)?;
                    Ok(Some(image::load_from_memory(&bytes)?))
                }));
            }
        }
        if let Some(task) = &mut self.board_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!(?err, "failed to load illustration for board");
                    }
                    Ok(image) => {
                        if let Some(image) = image {
                            let tex: SafeTexture = image.into();
                            self.board_tex_last = self.board_tex.replace(tex);
                            self.board_dir = random();
                        }
                    }
                }
                self.board_last_time = t;
                self.board_task = None;
            }
        }
        if let Some(task) = &mut self.has_new_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!("fail to load has new {:?}", err);
                    }
                    Ok(has) => {
                        self.has_new = has;
                    }
                }
                self.has_new_task = None;
            }
        }
        if let Some(task) = &mut self.check_update_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!("fail to check update {:?}", err);
                    }
                    Ok(Some(ver)) => {
                        if get_data().ignored_version.as_ref().is_none_or(|it| it < &ver.version) {
                            Dialog::plain(
                                tl!("update", "version" => ver.version.to_string()),
                                tl!("update-desc", "date" => ver.date.to_string(), "desc" => ver.description),
                            )
                            .buttons(vec![
                                tl!("cancel").into_owned(),
                                tl!("update-ignore").into_owned(),
                                tl!("update-go").into_owned(),
                            ])
                            .listener(move |_dialog, pos| {
                                match pos {
                                    1 => {
                                        get_data_mut().ignored_version = Some(ver.version.clone());
                                        let _ = save_data();
                                    }
                                    2 => {
                                        let _ = open_url(&ver.url);
                                    }
                                    _ => {}
                                }
                                false
                            })
                            .show();
                        }
                    }
                    _ => {}
                }
                self.check_update_task = None;
            }
        }
        if let Some(task) = &mut self.check_bold_font_update_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!("fail to check bold font update {:?}", err);
                    }
                    Ok(None) => {}
                    Ok(Some(parsed)) => {
                        info!(cksum = parsed.1, "new bold font");
                        set_bold_font(parsed);
                    }
                }
                self.check_bold_font_update_task = None;
            }
        }
        if let Some(task) = &mut self.char_illu_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!(?err, "fail to load char illu");
                    }
                    Ok(image) => {
                        self.char_appear_p.goto(1., t, 0.5);
                        let tex: SafeTexture = image.into();
                        self.char_illu = Some(tex.with_mipmap());
                    }
                }
                self.char_illu_task = None;
            }
        }
        if let Some(task) = &mut self.char_fetch_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!(?err, "fail to load char");
                    }
                    Ok(char) => {
                        info!(?char, "char loaded");
                        self.character = char;
                        get_data_mut().character = Some(self.character.clone());
                        let _ = save_data();
                        self.char_cached_size = 0.;
                        self.load_char_illu();
                    }
                }
                self.char_fetch_task = None;
            }
        }
        if JUST_LOADED_TOS.fetch_and(false, Ordering::Relaxed) {
            check_read_tos_and_policy(true, true);
        }

        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let rtime = s.rt;

        // 删掉切进主页时的向上移动画（Fader 页面滑动），仅保留淡入
        let old_distance = s.fader.distance;
        if s.fader.transiting() {
            s.fader.distance = 0.;
        }

        if get_data().config.show_character {
            if let Some(model) = &mut self.spine_model {
                let fader_p = s.fader.progress(rtime);
                let fader_alpha = 1. - fader_p.abs();
                let alpha = self.spine_alpha.now(rtime) * fader_alpha;
                if alpha > 0.001 {
                    unsafe { get_internal_gl() }.flush();
                    let config = &get_data().config;
                    model.position = glam::Vec2::new(0.50 + config.character_model_offset_x, 0.48 + config.character_model_offset_y);
                    model.scale = config.character_model_scale;
                    model.alpha = alpha;
                    model.render();
                    unsafe { get_internal_gl() }.flush();
                }
            }
        }

        self.render_not_char(ui, s);

        let p_user = self.appear(rtime, 0.0, 0.26);
        let (dx_user, dy_user) = (0.18 * (1. - p_user), -0.05 * (1. - p_user));

        s.render_fader(ui, |ui| {
            ui.scope(|ui| {
                ui.dx(dx_user);
                ui.dy(dy_user);
                ui.alpha(p_user, |ui| {
                    let rad = 0.05;
                    let ct = (0.92, -ui.top + 0.08);
                    self.btn_user.config.radius = rad;
                    let r = Rect::new(ct.0, ct.1, 0., 0.).feather(rad);
                    self.btn_user.build(ui, t, r, |ui, _| {
                        ui.avatar(
                            ct.0,
                            ct.1,
                            r.w / 2.,
                            t,
                            get_data()
                                .me
                                .as_ref()
                                .map(|user| UserManager::opt_avatar(user.id, &self.icons.user))
                                .unwrap_or(Err(self.icons.user.clone())),
                        );
                    });
                    let rtx = ct.0 - rad - 0.02;
                    if let Some(me) = &get_data().me {
                        ui.text(&me.name).pos(rtx, r.center().y + 0.002).anchor(1., 1.).size(0.6).draw();
                        ui.text(format!("RKS {:.2}", me.rks))
                            .pos(rtx, r.center().y + 0.008)
                            .anchor(1., 0.)
                            .size(0.4)
                            .color(semi_white(0.6))
                            .draw();
                    } else {
                        ui.text(tl!("not-logged-in"))
                            .pos(rtx, r.center().y)
                            .anchor(1., 0.5)
                            .no_baseline()
                            .size(0.6)
                            .draw();
                    }
                });
            });

            #[cfg(feature = "hykb")]
            {
                let r = ui.screen_rect();
                let r = ui
                    .text("备案号：闽ICP备18008307号-64A")
                    .pos(r.x + 0.02, r.bottom() - 0.03)
                    .size(0.5)
                    .anchor(0., 1.)
                    .draw();
                self.beian_btn.set(ui, r);
            }
        });

        self.login.render(ui, t);
        #[cfg(feature = "hykb")]
        if self.update_task.is_some() {
            ui.full_loading_simple(t);
        }
        self.sf.render(ui, t);

        s.fader.distance = old_distance;
        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        self.next_page.take().unwrap_or_default()
    }

    fn next_scene(&mut self, s: &mut SharedState) -> NextScene {
        self.sf.next_scene(s.t).unwrap_or_default()
    }
}