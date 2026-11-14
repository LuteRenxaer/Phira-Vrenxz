prpr_l10n::tl_file!("chapter");

use crate::{
    anim::Anim,
    data::BriefChartInfo,
    dir,
    icons::Icons,
    load_res_tex,
    page::{ChartItem, ChartType, Illustration, SFader},
    resource::rtl,
};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    config::{HpMode, Mods},
    core::{BOLD_FONT, PGR_FONT},
    ext::{semi_black, semi_white, RectExt, SafeTexture, BLACK_TEXTURE, PARALLELOGRAM_SLOPE},
    info::ChartInfo,
    scene::{show_message, NextScene, Scene},
    time::TimeManager,
    ui::{back_sound, button_hit, DRectButton, RectButton, Scroll, Ui},
};
use serde::Deserialize;
use std::{borrow::Cow, collections::HashMap, sync::Arc};
use tap::Tap;

use super::{SongScene, ASSET_CHART_INFO};

#[repr(usize)]
#[derive(Clone, Copy)]
enum Difficulty {
    Easy,
    Hard,
    Extreme,
}
impl Difficulty {
    pub fn name(&self) -> Cow<'static, str> {
        match self {
            Self::Easy => tl!("diff-easy"),
            Self::Hard => tl!("diff-hard"),
            Self::Extreme => tl!("diff-extreme"),
        }
    }

    pub fn color(&self) -> Color {
        Color::from_hex_rgb(match self {
            Self::Easy => 0x16a34a,
            Self::Hard => 0xf97316,
            Self::Extreme => 0xdc2626,
        })
    }
}

#[derive(Deserialize)]
struct LevelInfo {
    level: String,
    charter: String,
    difficulty: f32,
}

#[derive(Deserialize)]
struct SongInfo {
    name: String,
    intro: String,
    composer: String,
    illustrator: String,
    levels: Vec<LevelInfo>,
}

struct ChartInstance {
    id: String,
    info: SongInfo,
    illu: SafeTexture,
    btn: DRectButton,
}

/// Remember 章节的谱面卡片（本地谱面或占位）
struct RememberChart {
    name: String,
    level: String,
    illu: SafeTexture,
    info: Option<ChartInfo>,
    installed: bool,
    local_path: Option<String>,
    btn: DRectButton,
    /// Phigros 风格难度标签（EZ / HD / IN）
    tag: String,
    tag_color: Color,
}

pub struct ChapterScene {
    id: String,

    icons: Arc<Icons>,
    rank_icons: [SafeTexture; 8],
    cover: SafeTexture,

    btn_back: RectButton,

    next_scene: Option<NextScene>,

    first_in: bool,

    diff: Difficulty,
    diff_btn: DRectButton,
    diff_btn_color: Anim<Color>,

    sf: SFader,

    scroll: Scroll,
    charts: Vec<ChartInstance>,

    // Remember 章节专用
    remember_charts: Vec<RememberChart>,
    remember_bg: SafeTexture,
    /// 当前选中的列表行索引（决定亮白/灰暗样式）
    remember_selected: usize,
}

impl ChapterScene {
    const WIDTH: f32 = 0.5;
    const HEIGHT: f32 = 0.3;
    const PAD: f32 = 0.05;

    /// Remember 列表行高 / 间距（Phigros 选歌列表风格；Unity: 行高100px/行距80px → step=0.125，行间重叠0.031）
    const ROW_H: f32 = 0.156;
    const ROW_PAD: f32 = -0.031;

    /// Remember 章节固定谱面列表
    const REMEMBER_SONGS: [&'static str; 7] = [
        "Act Beloved",
        "Weavers",
        "Mind Enhancement",
        "Luminescence",
        "無人区-Vacuum Track#ADD8E6-",
        "Twin Skies",
        "Realms",
    ];

    pub async fn new(id: String, icons: Arc<Icons>, rank_icons: [SafeTexture; 8], cover: SafeTexture) -> Result<Self> {
        let is_remember = id == "remember";
        let songs = if is_remember {
            Vec::new()
        } else {
            match id.as_str() {
                "c1" => vec!["snow", "jumping23"],
                _ => vec![],
            }
        };
        let mut charts = Vec::with_capacity(songs.len());
        for song in songs {
            let info = serde_yaml::from_slice(&load_file(&format!("res/song/{song}/info.yml")).await?)?;
            let illu = load_res_tex(&format!("res/song/{song}/cover")).await;
            charts.push(ChartInstance {
                id: song.to_owned(),
                info,
                illu,
                btn: DRectButton::new(),
            });
        }
        let remember_charts = if is_remember { Self::load_remember_charts().await? } else { Vec::new() };
        let remember_bg = if is_remember {
            match load_texture("backgrounds/Remember.png").await {
                Ok(tex) => SafeTexture::from(tex),
                Err(err) => {
                    warn!("failed to load Remember.png background: {err}; using placeholder");
                    BLACK_TEXTURE.clone()
                }
            }
        } else {
            BLACK_TEXTURE.clone()
        };
        Ok(Self {
            id,

            icons,
            rank_icons,
            cover,
            btn_back: RectButton::new(),

            next_scene: None,

            first_in: true,

            diff: Difficulty::Hard,
            diff_btn: DRectButton::new(),
            diff_btn_color: Anim::new(Difficulty::Hard.color()),

            sf: SFader::new(),

            scroll: if is_remember {
                Scroll::new().tap_mut(|it| it.y_scroller.step = Self::ROW_H + Self::ROW_PAD)
            } else {
                Scroll::new().tap_mut(|it| it.y_scroller.step = Self::HEIGHT + Self::PAD)
            },
            charts,

            remember_charts,
            remember_bg,
            remember_selected: 0,
        })
    }

    /// 扫描本地谱面目录，按名称匹配 Remember 章节的 6 首谱面
    async fn load_remember_charts() -> Result<Vec<RememberChart>> {
        let mut found: HashMap<String, (String, ChartInfo)> = HashMap::new();
        for base in [dir::downloaded_charts()?, dir::custom_charts()?] {
            let base_path = std::path::Path::new(&base);
            if !base_path.is_dir() {
                continue;
            }
            let base_name = base_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            for entry in std::fs::read_dir(base_path)? {
                let entry = entry?;
                let dir_path = entry.path();
                if !dir_path.is_dir() {
                    continue;
                }
                let info_path = dir_path.join("info.yml");
                if !info_path.exists() {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&info_path) else { continue };
                let Ok(info) = serde_yaml::from_slice::<ChartInfo>(&bytes) else { continue };
                let local_path = format!("{}/{}", base_name, dir_path.file_name().unwrap_or_default().to_string_lossy());
                found.entry(info.name.clone()).or_insert((local_path, info));
            }
        }

        let mut res = Vec::with_capacity(Self::REMEMBER_SONGS.len());
        for name in Self::REMEMBER_SONGS {
            if let Some((local_path, info)) = found.get(name) {
                // 加载本地谱面封面
                let illu = Self::load_local_cover(local_path, &info.illustration).await.unwrap_or_else(|| BLACK_TEXTURE.clone());
                let (tag, tag_color) = Self::tag_for(&info.level, info.difficulty);
                res.push(RememberChart {
                    name: name.to_owned(),
                    level: info.level.clone(),
                    illu,
                    info: Some(info.clone()),
                    installed: true,
                    local_path: Some(local_path.clone()),
                    btn: DRectButton::new(),
                    tag,
                    tag_color,
                });
            } else {
                res.push(RememberChart {
                    name: name.to_owned(),
                    level: String::new(),
                    illu: BLACK_TEXTURE.clone(),
                    info: None,
                    installed: false,
                    local_path: None,
                    btn: DRectButton::new(),
                    tag: "?".to_owned(),
                    tag_color: Color::from_hex_rgb(0x64748b),
                });
            }
        }
        Ok(res)
    }

    /// 从本地谱面目录读取封面图片为纹理
    async fn load_local_cover(local_path: &str, illu_file: &str) -> Option<SafeTexture> {
        let dir = format!("{}/{}", dir::charts().ok()?, local_path);
        let bytes = std::fs::read(format!("{}/{}", dir, illu_file)).ok()?;
        let img = image::load_from_memory(&bytes).ok()?;
        Some(SafeTexture::from(img))
    }

    /// 根据谱面等级字符串 / 定数推断 Phigros 风格难度标签（EZ / HD / IN）
    fn tag_for(level: &str, difficulty: f32) -> (String, Color) {
        let l = level.trim().to_ascii_uppercase();
        if l.starts_with("EZ") {
            ("EZ".to_owned(), Color::from_hex_rgb(0x16a34a))
        } else if l.starts_with("HD") {
            ("HD".to_owned(), Color::from_hex_rgb(0xf97316))
        } else if l.starts_with("IN") {
            ("IN".to_owned(), Color::from_hex_rgb(0xdc2626))
        } else if difficulty < 7. {
            ("EZ".to_owned(), Color::from_hex_rgb(0x16a34a))
        } else if difficulty < 11. {
            ("HD".to_owned(), Color::from_hex_rgb(0xf97316))
        } else {
            ("IN".to_owned(), Color::from_hex_rgb(0xdc2626))
        }
    }

    /// 从等级字符串提取数字（"IN Lv.15" -> "15"，"AT Lv.18.2" -> "18.2"）；无法解析时用定数取整兜底
    fn level_num(level: &str, difficulty: f32) -> String {
        let l = level.trim();
        if l.is_empty() {
            return if difficulty > 0. {
                format!("{:.0}", difficulty)
            } else {
                "?".to_owned()
            };
        }
        if let Some(pos) = l.find("Lv") {
            let num: String = l[pos + 2..]
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !num.is_empty() {
                return num;
            }
        }
        if l.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return l.to_owned();
        }
        if difficulty > 0. {
            format!("{:.0}", difficulty)
        } else {
            "?".to_owned()
        }
    }
}

impl Scene for ChapterScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.first_in {
            self.first_in = false;
            tm.reset();
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;
        if self.btn_back.touch(touch) {
            back_sound();
            self.next_scene = Some(NextScene::Pop);
            return Ok(true);
        }
        if self.id == "remember" {
            if self.scroll.touch(touch, t) {
                return Ok(true);
            }
            for (i, chart) in self.remember_charts.iter_mut().enumerate() {
                if chart.btn.touch(touch, t) {
                    button_hit();
                    self.remember_selected = i;
                    if let Some(local_path) = &chart.local_path {
                        let info = chart.info.clone().unwrap();
                        let brief = BriefChartInfo::from(info);
                        let item = ChartItem {
                            info: brief,
                            illu: Illustration::from_done(chart.illu.clone()),
                            local_path: Some(local_path.clone()),
                            chart_type: ChartType::Downloaded,
                            level_author: None,
                            xcsim_preview_url: None,
                            xcsim_illustration_url: None,
                        };
                        self.sf.goto(t, SongScene::new(item, Some(local_path.clone()), Arc::clone(&self.icons), self.rank_icons.clone(), Mods::empty(), HpMode::Off));
                    } else {
                        show_message("该谱面未安装，无法游玩").warn();
                    }
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        if self.diff_btn.touch(touch, t) {
            button_hit();
            self.diff = match self.diff {
                Difficulty::Easy => Difficulty::Hard,
                Difficulty::Hard => Difficulty::Extreme,
                Difficulty::Extreme => Difficulty::Easy,
            };
            self.diff_btn_color.goto(self.diff.color(), t, 0.4);
            return Ok(true);
        }
        if self.scroll.touch(touch, t) {
            return Ok(true);
        }
        for chart in &mut self.charts {
            if chart.btn.touch(touch, t) {
                button_hit();
                let info = &chart.info;
                let level = &info.levels[self.diff as usize];
                let local_path = format!(
                    ":{}:{}",
                    chart.id,
                    match self.diff {
                        Difficulty::Easy => "ez",
                        Difficulty::Hard => "hd",
                        Difficulty::Extreme => "ex",
                    }
                );
                let item = ChartItem {
                    info: BriefChartInfo {
                        id: None,
                        uploader: None,
                        name: info.name.clone(),
                        level: level.level.clone(),
                        difficulty: level.difficulty,
                        intro: info.intro.clone(),
                        charter: level.charter.clone(),
                        composer: info.composer.clone(),
                        illustrator: info.illustrator.clone(),
                        created: None,
                        updated: None,
                        chart_updated: None,
                        has_unlock: false,
                    },
                    illu: Illustration::from_done(chart.illu.clone()),
                    local_path: Some(local_path.clone()),
                    chart_type: ChartType::Integrated,
                    level_author: None,
                    xcsim_preview_url: None,
                    xcsim_illustration_url: None,
                };
                let info = &item.info;
                let dir = format!("{}/{}", dir::charts()?, item.local_path.as_ref().unwrap().replace(':', "_"));
                let path = std::path::Path::new(&dir);
                if !path.exists() {
                    std::fs::create_dir_all(path)?;
                }
                let dir = prpr::dir::Dir::new(dir)?;
                *ASSET_CHART_INFO.lock().unwrap() = Some(ChartInfo {
                    id: None,
                    uploader: None,

                    name: info.name.clone(),
                    difficulty: info.difficulty,
                    level: info.level.clone(),
                    charter: info.charter.clone(),
                    composer: info.composer.clone(),
                    illustrator: info.illustrator.clone(),

                    chart: ":chart".to_owned(),
                    format: None,
                    music: ":music".to_owned(),
                    illustration: ":illu".to_owned(),
                    unlock_video: None,

                    dynamic_background: 0,

                    preview_start: 0.,
                    preview_end: None,
                    aspect_ratio: 16. / 9.,
                    background_dim: 0.6,
                    line_length: 6.,
                    offset: dir
                        .read("offset")
                        .map(|d| {
                            f32::from_be_bytes(
                                d.get(0..4)
                                    .map(|first4| {
                                        let mut result = <[u8; 4]>::default();
                                        result.copy_from_slice(first4);
                                        result
                                    })
                                    .unwrap_or_default(),
                            )
                        })
                        .unwrap_or_default(),
                    tip: None,
                    tags: Vec::new(),

                    intro: info.intro.clone(),

                    hold_partial_cover: true,
                    note_uniform_scale: false,
                    force_aspect_ratio: false,
                    use_rpe_170_speed: Some(false),
                    use_attach_ui_fix: Some(true),

                    arcaea_judgement: false,
                    fnf_judgement: false,

                    created: None,
                    updated: None,
                    chart_updated: None,
                });
                self.sf
                    .goto(t, SongScene::new(item, Some(local_path), Arc::clone(&self.icons), self.rank_icons.clone(), Mods::empty(), HpMode::Off));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        self.scroll.update(t);
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        // 背景使用原始比例，不随 UI 比例缩放
        set_camera(&ui.bg_camera());
        let t = tm.now() as f32;

        if self.id == "remember" {
            // ========== Remember 章节：Phigros 风格选歌页（背景 Remember.png + 竖直列表） ==========
            let r = ui.screen_rect();
            ui.fill_rect(r, (*self.remember_bg, r));
            ui.fill_rect(r, semi_black(0.3));

            // UI 层
            set_camera(&ui.camera());
            let r = ui.back_rect();
            ui.fill_rect(r, (*self.icons.back, r));
            self.btn_back.set(ui, r);

            // 顶部栏：标题（Phigros Chapter9 风格：标题放左上角，字体用 Phigros 官方字体；无副标题行）
            let top = ui.top;
            ui.text("Remember")
                .pos(-0.93, -top + 0.16)
                .anchor(0., 0.)
                .size(1.35)
                .color(WHITE)
                .draw_using(&PGR_FONT);

            // 竖直选歌列表（Unity 精确参数：视口从屏幕左缘到屏内 42.5%，高度容纳 7 行）
            let lr = Rect::new(-1.0, -0.48, 0.85, 0.91);
            self.scroll.size((lr.w, lr.h));
            ui.scope(|ui| {
                ui.dx(lr.x);
                ui.dy(lr.y);
                self.scroll.render(ui, |ui| {
                    ui.dx(lr.w / 2.);
                    // 内容从视口顶部开始排布
                    ui.dy(Self::ROW_H / 2.);
                    let mut y = 0.;
                    let step = Self::ROW_H + Self::ROW_PAD;
                    // 第一遍：所有未选中行（半透明深灰斜切条 + 歌名 + 无色块难度文字）
                    for (i, chart) in self.remember_charts.iter_mut().enumerate() {
                        if i == self.remember_selected {
                            y += step;
                            continue;
                        }
                        let row = Rect::new(-0.658, y - Self::ROW_H / 2., 1.023, Self::ROW_H);
                        chart.btn.inner.set(ui, row);
                        ui.fill_path(&row.feather(-0.008).parallelogram(PARALLELOGRAM_SLOPE), semi_black(0.55));

                        // 行内只画歌名（无曲师行）
                        ui.text(&chart.name)
                            .pos(-0.405, row.center().y)
                            .anchor(0., 0.5)
                            .max_width(0.40)
                            .size(0.44)
                            .color(semi_white(0.6))
                            .draw_using(&BOLD_FONT);

                        // 未选中行只画歌名（参考图规格：难度信息只在选中行的白色难度块中展示）

                        y += step;
                    }
                    // 第二遍：选中行单独绘制，保证盖在相邻行上（纯黑实底 + 歌名/曲师 + 难度容器 + 白色难度块竖排）
                    for (i, chart) in self.remember_charts.iter_mut().enumerate() {
                        if i != self.remember_selected {
                            continue;
                        }
                        let y_sel = i as f32 * step + Self::ROW_H / 2.;
                        // 选中行左缘与未选中行基本对齐（Unity 差 5px≈0.008），靠右侧加宽覆盖难度块区
                        let row_sel = Rect::new(-0.666, y_sel - 0.1125, 1.158, 0.225);
                        chart.btn.inner.set(ui, row_sel);
                        ui.fill_path(&row_sel.feather(-0.006).parallelogram(PARALLELOGRAM_SLOPE), Color::from_hex_rgb(0x000000));
                        // 歌名
                        ui.text(&chart.name)
                            .pos(-0.543, y_sel - 0.031)
                            .anchor(0., 0.5)
                            .max_width(0.50)
                            .size(0.48)
                            .color(WHITE)
                            .draw_using(&BOLD_FONT);
                        // 曲师
                        let composer = chart.info.as_ref().map(|it| it.composer.as_str()).unwrap_or("Unknown");
                        ui.text(composer)
                            .pos(-0.539, y_sel + 0.055)
                            .anchor(0., 0.5)
                            .max_width(0.50)
                            .size(0.30)
                            .color(semi_white(0.75))
                            .draw();
                        // 难度容器背景：右端贴选中行右缘（0.492），Unity Level 191.91x158 比例 1.215
                        let sel_diff_r = Rect::new(0.172, y_sel - 0.130, 0.320, 0.260);
                        ui.fill_path(&sel_diff_r.feather(-0.006).parallelogram(PARALLELOGRAM_SLOPE), semi_black(0.7));
                        // 白色难度块：Level 比例（1.215）右端=选中行右缘，高度≈行高，超出部分按 Unity 画全
                        let level_text = Self::level_num(&chart.level, chart.info.as_ref().map(|it| it.difficulty).unwrap_or_default());
                        let white_r = Rect::new(0.192, y_sel - 0.1235, 0.300, 0.247);
                        ui.fill_path(&white_r.feather(-0.004).parallelogram(PARALLELOGRAM_SLOPE), WHITE);
                        ui.text(&chart.tag)
                            .pos(0.342, y_sel - 0.045)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.34)
                            .color(Color::from_hex_rgb(0x1a1a1a))
                            .draw();
                        ui.text(level_text)
                            .pos(0.342, y_sel + 0.035)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.34)
                            .color(Color::from_hex_rgb(0x1a1a1a))
                            .draw();
                    }
                    (lr.w, step * (self.remember_charts.len() - 1) as f32 + Self::ROW_H)
                });
            });

            // 右侧大封面：当前选中谱面的插画，竖矩形从顶栏下方延伸到屏底，压住列表右缘
            if let Some(sel) = self.remember_charts.get(self.remember_selected) {
                if sel.installed && sel.illu != *BLACK_TEXTURE {
                    // 封面左缘 -0.19 盖住列表/选中行右缘，右缘 1.334 出屏由相机裁剪；无蒙层
                    // 封面左缘右移至列表右缘附近（Unity 仅 8px 重叠，参考图有间隙），不显著压住列表/难度块
                    let cover_r = Rect::new(-0.10, -0.302, 1.524, 0.804);
                    ui.fill_rect(cover_r, (*sel.illu, Rect::new(0., 0., 1., 1.)));
                }
            }

            self.sf.render(ui, t);
            return Ok(());
        }

        let r = ui.screen_rect();
        ui.fill_rect(r, (*self.cover, r));
        ui.fill_rect(r, semi_black(0.3));

        // UI 使用带比例的 camera
        set_camera(&ui.camera());
        let r = ui.back_rect();
        ui.fill_rect(r, (*self.icons.back, r));
        self.btn_back.set(ui, r);

        use crate::resource::L10N_LOCAL;

        let title = rtl!(format!("chap-{}", self.id)).into_owned();
        let intro = rtl!(format!("chap-{}-intro", self.id));

        let p = (t / 0.4).min(1.);
        let p = 1. - (1. - p).powi(3);

        let r = Rect::new(-0.83, -0.35, 0.6, 0.12);
        ui.scissor(r, |ui| {
            ui.text(title).pos(r.x, r.y + (1. - p) * 0.1).size(1.4).draw_using(&BOLD_FONT);
        });
        ui.text(intro)
            .pos(r.x, r.bottom() + 0.02)
            .size(0.44)
            .max_width(0.74)
            .multiline()
            .color(semi_white(p))
            .draw();

        let r = Rect::new(r.x, 0.3, 0.24, 0.1);
        self.diff_btn.render_shadow(ui, r, t, |ui, path| {
            let ct = r.center();
            ui.fill_path(&path, self.diff_btn_color.now(t));
            ui.text(self.diff.name())
                .pos(ct.x, ct.y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.6)
                .draw_using(&BOLD_FONT);
        });

        let r = Rect::new(0.2, -ui.top, 0.6, ui.top * 2.);
        self.scroll.size((r.w, r.h));
        ui.scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            self.scroll.render(ui, |ui| {
                ui.dx(r.w / 2.);
                ui.dy(ui.top);
                let mut y = 0.;
                let step = Self::HEIGHT + Self::PAD;
                for chart in &mut self.charts {
                    let r = Rect::new(-Self::WIDTH / 2., y - Self::HEIGHT / 2., Self::WIDTH, Self::HEIGHT);
                    chart.btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, (*chart.illu, r));
                        ui.fill_path(&path, semi_black(0.4));
                        let mut t = ui
                            .text(&chart.info.levels[self.diff as usize].level)
                            .pos(r.right() - 0.016, r.y + 0.016)
                            .max_width(r.w * 2. / 3.)
                            .anchor(1., 0.)
                            .size(0.52)
                            .color(WHITE);
                        let ms = t.measure();
                        t.ui.fill_path(&ms.feather(0.008).rounded(0.01), Color { a: 0.7, ..t.ui.background() });
                        t.draw();

                        ui.text(&chart.info.name)
                            .pos(r.x + 0.01, r.bottom() - 0.02)
                            .max_width(r.w)
                            .anchor(0., 1.)
                            .size(0.6)
                            .color(WHITE)
                            .draw();
                    });
                    y += step;
                }

                (Self::WIDTH, step * (self.charts.len() - 1) as f32 + ui.top * 2.)
            });
        });

        self.sf.render(ui, t);

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().or_else(|| self.sf.next_scene(tm.now() as f32)).unwrap_or_default()
    }
}
