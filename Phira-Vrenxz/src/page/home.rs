prpr_l10n::tl_file!("home");
use super::{
    load_font_with_cksum, set_bold_font, LibraryPage, NextPage, Page, PhigrosLibraryPage, ResPackPage,
    SFader, SettingsPage, SharedState, BOLD_FONT_CKSUM,
};
use super::message::MessagePage;
use crate::{
    client::{recv_raw, Client, LoginParams, User, UserManager},
    dir, get_data, get_data_mut,
    icons::Icons,
    login::Login,
    save_data,
    scene::{check_read_tos_and_policy, ChapterScene, ProfileScene, JUST_LOADED_TOS},
    sync_data,
    threed::ThreeD,
};
use ::rand::{random, thread_rng, Rng};
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use image::DynamicImage;
use macroquad::prelude::*;
use prpr::{
    ext::{open_url, poll_future, semi_black, semi_white, LocalTask, RectExt, SafeTexture, ScaleType, BLACK_TEXTURE},
    info::ChartInfo,
    scene::{show_error, NextScene},
    task::Task,
    ui::{clip_rounded_rect, DRectButton, Dialog, FontArc, Ui},
};
use reqwest::StatusCode;
use serde::Deserialize;
use std::{
    borrow::Cow,
    sync::{atomic::Ordering, Arc},
};
use tap::Tap;
use tracing::{info, warn};

const BOARD_SWITCH_TIME: f32 = 4.;
const BOARD_TRANSIT_TIME: f32 = 1.2;

/// 主页菜单统一斜切量（相对按钮高度）
const MENU_SKEW: f32 = 0.26;

/// 斜切矩形（上边右移 sk），与设置页/结算页同一套语言
fn fill_para(ui: &mut Ui, r: Rect, sk: f32, color: Color) {
    fill_band(ui, r, sk, 0., 1., color);
}

/// 斜切矩形里横向 [from, to] 那一条（用于按钮右端的白条）
fn fill_band(ui: &mut Ui, r: Rect, sk: f32, from: f32, to: f32, color: Color) {
    use lyon::math::point;
    let xa = r.x + r.w * from;
    let xb = r.x + r.w * to;
    let mut b = lyon::path::Path::builder();
    b.begin(point(xa + sk, r.y));
    b.line_to(point(xb + sk, r.y));
    b.line_to(point(xb, r.bottom()));
    b.line_to(point(xa, r.bottom()));
    b.end(true);
    ui.fill_path(&b.build(), color);
}

fn stroke_para(ui: &mut Ui, r: Rect, sk: f32, width: f32, color: Color) {
    use lyon::math::point;
    let mut b = lyon::path::Path::builder();
    b.begin(point(r.x + sk, r.y));
    b.line_to(point(r.right() + sk, r.y));
    b.line_to(point(r.right(), r.bottom()));
    b.line_to(point(r.x, r.bottom()));
    b.end(true);
    ui.stroke_path(&b.build(), width, color);
}


/// 主页菜单按钮：主按钮白底黑字，次按钮黑底白字 + 右端白条
fn draw_menu_button(ui: &mut Ui, r: Rect, primary: bool) {
    let sk = r.h * MENU_SKEW;
    if primary {
        fill_para(ui, r, sk, WHITE);
    } else {
        fill_para(ui, r, sk, Color::new(0.035, 0.037, 0.046, 0.88));
        stroke_para(ui, r, sk, 0.0012, semi_white(0.16));
        fill_band(ui, r, sk, 0.955, 1.0, WHITE);
    }
}

type BoldFontUpdateTask = Task<Result<Option<(FontArc, String)>>>;

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
    btn_challenge: DRectButton,
    btn_respack: DRectButton,
    btn_settings: DRectButton,
    btn_user: DRectButton,
    btn_message: DRectButton,
    btn_chapter: DRectButton,

    next_page: Option<NextPage>,
    next_page_task: LocalTask<Result<NextPage>>,
    /// 章节按钮异步加载 Remember 章节场景
    chapter_scene_task: LocalTask<Result<ChapterScene>>,

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

    /// 已加载的本地插件列表
    plugins: Vec<crate::plugin::Plugin>,


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
        let res = Self {
            icons: Arc::clone(&icons),

            btn_play: DRectButton::new().with_delta(-0.01),
            btn_challenge: DRectButton::new().with_delta(-0.01),
            btn_respack: DRectButton::new().with_delta(-0.004).with_elevation(0.).no_sound(),
            btn_settings: DRectButton::new().with_delta(-0.004).with_elevation(0.),
            btn_user: DRectButton::new().with_delta(-0.003),
            btn_message: DRectButton::new().with_delta(-0.004).with_elevation(0.),
            btn_chapter: DRectButton::new().with_delta(-0.004).with_elevation(0.),

            next_page: None,
            next_page_task: None,
            chapter_scene_task: None,

            login: Login::new(icons),
            update_task,

            need_back: false,
            sf: SFader::new(),
            enter_time: f32::NAN,

            board_task: None,
            board_last_time: f32::NEG_INFINITY,
            board_last: None,
            plugins: crate::plugin::load_plugins(),
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

            #[cfg(feature = "hykb")]
            beian_btn: RectButton::new(),
        };

        Ok(res)
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

    fn render_home(&mut self, ui: &mut Ui, s: &mut SharedState) {
        let t = s.t;
        let rt = s.rt;
        let p = self.appear(rt, 0.0, 0.26);

        // 字标与开屏完全同一位置/字号：点开始之后它原地不动，只有下面的菜单淡入
        let cx = crate::splash_ui::sx(crate::splash_ui::CONTENT_CX);
        s.render_fader(ui, |ui| {
            let logo_size = if cfg!(target_os = "ios") { 1.28 } else { 1.50 };
            crate::splash_ui::draw_logo(ui, cx, -0.02, logo_size, 1.0);
        });

        // 菜单列：与字标同宽，右端和字标右缘对齐
        let x0 = crate::splash_ui::sx(0.620);
        let x1 = crate::splash_ui::sx(0.940);
        let w = x1 - x0;

        // ==== 游玩（主按钮）====
        let h_play = 0.135;
        let r_play = Rect::new(x0, 0.112, w, h_play);
        s.render_fader(ui, |ui| {
            ui.alpha(p, |ui| {
                self.btn_play.build(ui, t, r_play, |ui, _| {
                    let sk = r_play.h * MENU_SKEW;
                    draw_menu_button(ui, r_play, true);
                    let dark = Color::new(0.06, 0.06, 0.08, 1.);
                    ui.text(tl!("play"))
                        .pos(r_play.x + 0.052 + sk * 0.5, r_play.center().y)
                        .anchor(0., 0.5)
                        .size(0.52)
                        .color(dark)
                        .draw();

                });
            });
        });

        // ==== 课题模式（次按钮）====
        // ==== 图标行：消息 / 资源包 / 设置（同一斜切板，右端对齐）====
        let btn = 0.088;
        let gap = 0.024;
        let row_y = r_play.bottom() + 0.042;
        let r_msg = Rect::new(x1 - btn * 3. - gap * 2., row_y, btn, btn);
        let r_res = Rect::new(x1 - btn * 2. - gap, row_y, btn, btn);
        let r_set = Rect::new(x1 - btn, row_y, btn, btn);
        s.render_fader(ui, |ui| {
            ui.alpha(p, |ui| {
                let mut draw_icon = |ui: &mut Ui, r: Rect, tex: &SafeTexture, dot: bool, frac: f32| {
                    let sk = r.h * MENU_SKEW;
                    fill_para(ui, r, sk, Color::new(0.035, 0.037, 0.046, 0.88));
                    stroke_para(ui, r, sk, 0.0012, semi_white(0.16));
                    fill_band(ui, r, sk, 0.955, 1.0, WHITE);
                    // 按贴图原始宽高比缩放：ScaleType::Fit 会直接拉伸填满矩形，
                    // 图标贴图是 51x41 / 45x41 这种非正方形，硬拉会被压扁变形。
                    // frac 逐图标微调：资源包(1.24 宽高比)和设置(1.10)本来就比消息(1.02)显大。
                    let ar = tex.width() / tex.height();
                    let avail_w = r.w * frac;
                    let avail_h = r.h * frac;
                    let (iw, ih) = if ar >= 1. {
                        (avail_w, avail_w / ar)
                    } else {
                        (avail_h * ar, avail_h)
                    };
                    let icx = r.center().x + sk * 0.5;
                    let icy = r.center().y;
                    let ir = Rect::new(icx - iw * 0.5, icy - ih * 0.5, iw, ih);
                    ui.fill_rect(ir, (**tex, ir, ScaleType::CropCenter, WHITE));
                    if dot {
                        let d = Rect::new(ir.right() - 0.006, ir.y - 0.010, 0.026, 0.026);
                        ui.fill_path(&d.rounded(0.013), RED);
                    }
                };
                self.btn_message.build(ui, t, r_msg, |ui, _| {
                    draw_icon(ui, r_msg, &self.icons.msg, self.has_new, 0.46);
                });
                self.btn_respack.build(ui, t, r_res, |ui, _| {
                    draw_icon(ui, r_res, &self.icons.respack, false, 0.38);
                });
                self.btn_settings.build(ui, t, r_set, |ui, _| {
                    draw_icon(ui, r_set, &self.icons.settings, false, 0.36);
                });
            });
        });
    }
}

impl Page for HomePage {
    fn label(&self) -> Cow<'static, str> {
        "".into()
    }

    fn enter(&mut self, s: &mut SharedState) -> Result<()> {
        if self.need_back {
            self.sf.enter(s.t);
            self.need_back = false;
        }
        if self.enter_time.is_nan() {
            self.enter_time = s.rt;
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
        if self.login.touch(touch, s.t) {
            return Ok(true);
        }
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
        if self.btn_message.touch(touch, t) {
            self.next_page = Some(NextPage::Overlay(Box::new(MessagePage::new(
                Arc::clone(&self.icons),
                s.icons.clone(),
            ))));
            return Ok(true);
        }
        // 章节按钮已注释（2026-10-05，插件功能改造），不再响应点击
        // if self.btn_chapter.touch(touch, t) {
        //     if get_data().config.home_chapter_button_show {
        //         let icons = Arc::clone(&self.icons);
        //         let rank_icons = s.icons.clone();
        //         // 点击章节直接进入 Remember 章节（不再经过谱面合集页）
        //         self.chapter_scene_task = Some(Box::pin(async move {
        //             ChapterScene::new("remember".to_owned(), icons, rank_icons, BLACK_TEXTURE.clone()).await
        //         }));
        //     }
        //     return Ok(true);
        // }
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

    Ok(false)
}

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        #[cfg(feature = "hykb")]
        if get_data().me.is_none() {
            self.login.force(t);
        }
        self.login.update(t)?;


        // 章节页面异步加载完成后切换
        if let Some(task) = &mut self.next_page_task {
            if let Some(res) = poll_future(task.as_mut()) {
                self.next_page = Some(res?);
                self.next_page_task = None;
            }
        }

        // 章节页面异步加载完成后再切换场景
        if let Some(task) = &mut self.chapter_scene_task {
            if let Some(res) = poll_future(task.as_mut()) {
                let scene = res?;
                self.sf.goto(t, scene);
                self.chapter_scene_task = None;
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

        self.render_home(ui, s);

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