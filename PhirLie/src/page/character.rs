use crate::{
    get_data, get_data_mut,
    page::{NextPage, Page, SharedState},
    save_data,
    spine_model::{SpineModel, SpineModelRawData},
};
use anyhow::Result;
use inputbox::InputBox;
use macroquad::prelude::*;
prpr_l10n::tl_file!("character");
use prpr::{
    ext::{RectExt, semi_black, semi_white},
    scene::{request_input, take_input},
    task::Task,
    ui::{DRectButton, RectButton, Ui},
};
use std::borrow::Cow;
use std::path::Path;
use std::collections::BTreeMap;
use tracing::warn;

// ========== 布局常量 ==========
const GROUP_X: f32 = -0.95;
const GROUP_W: f32 = 0.30;
const PANEL_X: f32 = 0.08;
const PANEL_W: f32 = 0.87;
const ITEM_H: f32 = 0.06;
const GAP: f32 = 0.008;
const HEADER_H: f32 = 0.04;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Model,
    Voice,
    Adjust,
}

/// 角色分组：组名 -> [(显示名, 完整路径)]
struct ModelGroup {
    name: String,
    models: Vec<(String, String)>,
}

pub struct CharacterPage {
    spine_model: Option<SpineModel>,
    spine_raw_task: Option<Task<Result<SpineModelRawData>>>,
    spine_petting: bool,
    spine_anim_dirty: bool,
    spine_touch_id: Option<u64>,

    // 模型数据
    groups: Vec<ModelGroup>,
    selected_group: usize,
    group_btns: Vec<RectButton>,
    model_btns: Vec<DRectButton>,

    // 搜索
    search_query: String,
    search_active: bool,

    // 右侧标签页
    detail_tab: DetailTab,
    tab_model_btn: RectButton,
    tab_voice_btn: RectButton,
    tab_adjust_btn: RectButton,

    // 调整 tab 控件
    name_btn: DRectButton,
    default_expr_btn: DRectButton,
    pet_expr_btn: DRectButton,
    active_slider: Option<usize>,
    reset_btn: DRectButton,
    import_btn: DRectButton,
    voice_btn: DRectButton,

    // 滚动
    scroll_y: f32,
    scroll_vel: f32,
    dragging_scroll: bool,
    last_drag_y: f32,

    // 左侧分组栏滚动
    group_scroll_y: f32,
    group_scroll_vel: f32,
    group_dragging: bool,
    group_last_drag_y: f32,

    next_page: Option<NextPage>,
    exiting: bool,
}

impl CharacterPage {
    fn panel_rect() -> Rect {
        let top = screen_height() / screen_width();
        Rect::new(PANEL_X, -top + 0.08, PANEL_W, top * 2. - 0.12)
    }

    fn group_rect() -> Rect {
        let top = screen_height() / screen_width();
        Rect::new(GROUP_X, -top + 0.08, GROUP_W, top * 2. - 0.12)
    }

    fn content_rect() -> Rect {
        let pr = Self::panel_rect();
        // 标签页高 0.05 + 间距，内容从标签页下方开始
        Rect::new(pr.x + 0.01, pr.y + 0.07, pr.w - 0.02, pr.h - 0.08)
    }

    pub fn new() -> Self {
        let all_models = crate::spine_model::list_builtin_models();

        // 按角色基名分组：从显示名中提取角色名（去掉括号里的变体）
        let mut group_map: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for (display_name, path) in &all_models {
            let base_name = if let Some(idx) = display_name.find('（') {
                display_name[..idx].to_string()
            } else {
                display_name.clone()
            };
            group_map.entry(base_name).or_default().push((display_name.clone(), path.clone()));
        }

        let groups: Vec<ModelGroup> = group_map
            .into_iter()
            .map(|(name, models)| ModelGroup { name, models })
            .collect();

        let group_btns = (0..groups.len()).map(|_| RectButton::new()).collect();
        let total_models: usize = groups.iter().map(|g| g.models.len()).sum();
        let model_btns = (0..total_models).map(|_| DRectButton::new()).collect();

        // 找到当前选中模型所在的分组
        let current_path = get_data().config.character_model_path.clone();
        let mut selected_group = 0;
        if let Some(cp) = &current_path {
            for (i, g) in groups.iter().enumerate() {
                if g.models.iter().any(|(_, p)| p == cp) {
                    selected_group = i;
                    break;
                }
            }
        }

        Self {
            spine_model: None,
            spine_raw_task: {
                let model_path = current_path;
                Some(Task::new(async move {
                    if let Some(dir) = model_path {
                        crate::spine_model::load_model_from_dir(&dir).await
                    } else {
                        crate::spine_model::load_default_model_data().await
                    }
                }))
            },
            spine_petting: false,
            spine_anim_dirty: true,
            spine_touch_id: None,

            groups,
            selected_group,
            group_btns,
            model_btns,

            search_query: String::new(),
            search_active: false,

            detail_tab: DetailTab::Model,
            tab_model_btn: RectButton::new(),
            tab_voice_btn: RectButton::new(),
            tab_adjust_btn: RectButton::new(),

            name_btn: DRectButton::new(),
            default_expr_btn: DRectButton::new(),
            pet_expr_btn: DRectButton::new(),
            active_slider: None,
            reset_btn: DRectButton::new(),
            import_btn: DRectButton::new(),
            voice_btn: DRectButton::new(),

            scroll_y: 0.,
            scroll_vel: 0.,
            dragging_scroll: false,
            last_drag_y: 0.,

            group_scroll_y: 0.,
            group_scroll_vel: 0.,
            group_dragging: false,
            group_last_drag_y: 0.,

            next_page: None,
            exiting: false,
        }
    }

    fn switch_model(&mut self, path_str: String, name: String) {
        let data = get_data_mut();
        data.config.character_model_path = Some(path_str.clone());
        if data.config.character_name == "星野(临战)" || data.config.character_name.is_empty() {
            data.config.character_name = name;
        }
        let _ = save_data();
        self.spine_model = None;
        self.spine_raw_task = Some(Task::new(async move {
            crate::spine_model::load_model_from_dir(&path_str).await
        }));
        self.spine_anim_dirty = true;
    }

    /// 获取当前分组（过滤搜索后）的模型列表
    fn filtered_models(&self) -> Vec<(String, String, usize)> {
        let group = &self.groups[self.selected_group];
        let query = self.search_query.to_lowercase();
        let mut result = Vec::new();
        let mut btn_idx = 0;
        for (name, path) in &group.models {
            if query.is_empty() || name.to_lowercase().contains(&query) {
                result.push((name.clone(), path.clone(), btn_idx));
            }
            btn_idx += 1;
        }
        result
    }

    fn tab_content_height(&self, tab: DetailTab) -> f32 {
        match tab {
            DetailTab::Model => {
                let count = self.filtered_models().len() as f32;
                // 搜索框 + 导入按钮 + 模型列表
                HEADER_H + (1.0 + count) * (ITEM_H + GAP) + GAP
            }
            DetailTab::Voice => ITEM_H + GAP + 0.06,
            DetailTab::Adjust => {
                // 名字 + 默认表情 + 抚摸表情 + 7滑块 + 重置 = 11 项
                11.0 * (ITEM_H + GAP)
            }
        }
    }

    fn set_slider_value(&mut self, idx: usize, v: f32) {
        let data = get_data_mut();
        match idx {
            0 => data.config.character_model_scale = v,
            1 => data.config.character_model_offset_x = v,
            2 => data.config.character_model_offset_y = v,
            3 => data.config.character_pet_offset_x = v,
            4 => data.config.character_pet_offset_y = v,
            5 => data.config.character_pet_width = v,
            6 => data.config.character_pet_height = v,
            _ => {}
        }
    }
}

impl Page for CharacterPage {
    fn label(&self) -> std::borrow::Cow<'static, str> {
        "Character".into()
    }

    fn update(&mut self, _s: &mut SharedState) -> Result<()> {
        if let Some(task) = &mut self.spine_raw_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(data) => {
                        if let Ok(mut model) = SpineModel::from_memory(
                            &data.atlas_data,
                            &data.skel_data,
                            &data.textures,
                            "Idle_01",
                        ) {
                            model.scale = 0.00065;
                            let config = &get_data().config;
                            model.set_expression(config.character_default_expr);
                            self.spine_model = Some(model);
                        }
                    }
                    Err(err) => {
                        warn!("failed to load character model: {:?}", err);
                    }
                }
                self.spine_raw_task = None;
            }
        }

        if get_data().config.show_character {
            if let Some(model) = &mut self.spine_model {
                model.update(get_frame_time());
                let config = &get_data().config;
                let target_expr = if self.spine_petting {
                    config.character_pet_expr
                } else {
                    config.character_default_expr
                };
                if self.spine_anim_dirty {
                    model.set_expression(target_expr);
                    self.spine_anim_dirty = false;
                }
            }
        }

        if let Some((id, text)) = take_input() {
            let data = get_data_mut();
            match id.as_str() {
                "char_name" => {
                    data.config.character_name = text;
                    save_data()?;
                }
                "char_default_expr" => {
                    if let Ok(v) = text.parse::<u32>() {
                        data.config.character_default_expr = v;
                        self.spine_anim_dirty = true;
                        save_data()?;
                    }
                }
                "char_pet_expr" => {
                    if let Ok(v) = text.parse::<u32>() {
                        data.config.character_pet_expr = v;
                        save_data()?;
                    }
                }
                "char_search" => {
                    self.search_query = text;
                    self.scroll_y = 0.;
                }
                _ => {}
            }
        }

        // 惯性滚动（右侧内容区）
        if !self.dragging_scroll && self.scroll_vel.abs() > 0.0001 {
            let cr = Self::content_rect();
            let total_h = self.tab_content_height(self.detail_tab);
            let max_scroll = (total_h - cr.h).max(0.);
            self.scroll_y += self.scroll_vel;
            self.scroll_vel *= 0.92;
            self.scroll_y = self.scroll_y.clamp(-max_scroll, 0.);
        }

        // 惯性滚动（左侧分组栏）
        if !self.group_dragging && self.group_scroll_vel.abs() > 0.0001 {
            let gr = Self::group_rect();
            let item_h = 0.06;
            let gap = 0.004;
            let total_h = self.groups.len() as f32 * (item_h + gap);
            let max_scroll = (total_h - (gr.h - 0.08)).max(0.);
            self.group_scroll_y += self.group_scroll_vel;
            self.group_scroll_vel *= 0.92;
            self.group_scroll_y = self.group_scroll_y.clamp(-max_scroll, 0.);
        }

        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        if !get_data().config.show_character {
            return Ok(false);
        }
        let t = s.t;
        let config = &get_data().config;
        let model_x = -0.35 + config.character_model_offset_x;
        let model_y = 0.48 + config.character_model_offset_y;
        let mr = Rect::new(
            model_x + config.character_pet_offset_x - config.character_pet_width / 2.,
            model_y + config.character_pet_offset_y - config.character_pet_height / 2.,
            config.character_pet_width,
            config.character_pet_height,
        );

        match touch.phase {
            TouchPhase::Started => {
                if mr.contains(touch.position) {
                    self.spine_touch_id = Some(touch.id);
                    self.spine_petting = true;
                    self.spine_anim_dirty = true;
                    return Ok(true);
                }
            }
            TouchPhase::Moved => {
                if let Some(id) = self.spine_touch_id {
                    if id == touch.id {
                        let inside = mr.contains(touch.position);
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
            }
            TouchPhase::Ended => {
                if let Some(id) = self.spine_touch_id {
                    if id == touch.id {
                        self.spine_touch_id = None;
                        self.spine_petting = false;
                        self.spine_anim_dirty = true;
                        return Ok(true);
                    }
                }
            }
            _ => {}
        }

        // 左侧分组栏（支持滚动）
        let gr = Self::group_rect();
        if gr.contains(touch.position) || self.group_dragging {
            let item_h = 0.06;
            let gap = 0.004;
            let total_h = self.groups.len() as f32 * (item_h + gap);
            let max_scroll = (total_h - (gr.h - 0.08)).max(0.);

            match touch.phase {
                TouchPhase::Started => {
                    self.group_dragging = true;
                    self.group_last_drag_y = touch.position.y;
                    self.group_scroll_vel = 0.;
                    // 检测点击
                    let start_y = gr.y + 0.06 + self.group_scroll_y;
                    for (i, _group) in self.groups.iter().enumerate() {
                        let r = Rect::new(gr.x + 0.01, start_y + i as f32 * (item_h + gap), gr.w - 0.02, item_h);
                        if r.contains(touch.position) {
                            self.selected_group = i;
                            self.scroll_y = 0.;
                            self.scroll_vel = 0.;
                            break;
                        }
                    }
                    return Ok(true);
                }
                TouchPhase::Moved => {
                    if self.group_dragging {
                        let dy = touch.position.y - self.group_last_drag_y;
                        self.group_last_drag_y = touch.position.y;
                        self.group_scroll_y += dy;
                        self.group_scroll_vel = dy;
                        self.group_scroll_y = self.group_scroll_y.clamp(-max_scroll, 0.);
                        return Ok(true);
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    self.group_dragging = false;
                    return Ok(true);
                }
                _ => {}
            }
            return Ok(true);
        }

        // 右侧面板边界（拖动滚动时不拦截，允许手指滑出面板继续滚）
        let pr = Self::panel_rect();
        if !self.dragging_scroll && !pr.contains(touch.position) {
            return Ok(false);
        }

        // 标签页切换
        if touch.phase == TouchPhase::Started {
            let tab_y = pr.y + 0.01;
            let tab_h = 0.05;
            let tab_w = (pr.w - 0.04) / 3.;
            let tab_defs = [DetailTab::Model, DetailTab::Voice, DetailTab::Adjust];
            for (i, tab) in tab_defs.iter().enumerate() {
                let r = Rect::new(pr.x + 0.02 + i as f32 * tab_w, tab_y, tab_w - 0.004, tab_h);
                if r.contains(touch.position) {
                    self.detail_tab = *tab;
                    self.scroll_y = 0.;
                    self.scroll_vel = 0.;
                    return Ok(true);
                }
            }
        }

        // 内容区
        let cr = Self::content_rect();
        let total_h = self.tab_content_height(self.detail_tab);
        let max_scroll = (total_h - cr.h).max(0.);
        let base_y = cr.y + self.scroll_y;

        // 滚动检测：只要在右侧面板内按下就开始拖动（放在最前面，避免被搜索框/按钮拦截）
        if touch.phase == TouchPhase::Started && pr.contains(touch.position) {
            // 标签页区域不触发滚动
            let tab_y = pr.y + 0.01;
            let tab_h = 0.05;
            let in_tab = touch.position.y >= tab_y && touch.position.y <= tab_y + tab_h;
            if !in_tab {
                self.dragging_scroll = true;
                self.last_drag_y = touch.position.y;
                self.scroll_vel = 0.;
            }
        }
        if self.dragging_scroll {
            match touch.phase {
                TouchPhase::Moved => {
                    let dy = touch.position.y - self.last_drag_y;
                    self.last_drag_y = touch.position.y;
                    self.scroll_y += dy;
                    self.scroll_vel = dy;
                    self.scroll_y = self.scroll_y.clamp(-max_scroll, 0.);
                    return Ok(true);
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    self.dragging_scroll = false;
                }
                _ => {}
            }
        }

        // 搜索框（仅模型 tab，Started 时）
        if self.detail_tab == DetailTab::Model && touch.phase == TouchPhase::Started {
            let search_r = Rect::new(cr.x + 0.01, base_y, cr.w - 0.02, HEADER_H);
            if search_r.contains(touch.position) {
                self.search_active = true;
                request_input("char_search", InputBox::new().default_text(&self.search_query));
                return Ok(true);
            }
        }

        // 滑动条激活时优先处理
        if self.active_slider.is_some() {
            return self.touch_adjust_sliders(touch, cr, base_y);
        }

        // 拖动时不触发按钮点击
        let just_dragged = touch.phase == TouchPhase::Ended && self.scroll_vel.abs() > 0.008;
        if just_dragged {
            return Ok(false);
        }

        // 内容区按钮
        match self.detail_tab {
            DetailTab::Model => self.touch_model_tab(touch, t, cr, base_y),
            DetailTab::Voice => self.touch_voice_tab(touch, t, cr, base_y),
            DetailTab::Adjust => self.touch_adjust_tab(touch, t, cr, base_y),
        }
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let rtime = s.rt;

        if !get_data().config.show_character {
            ui.text(tl!("char-disabled"))
                .pos(0., 0.)
                .anchor(0.5, 0.5)
                .size(0.3)
                .color(semi_white(0.7))
                .draw();
            ui.text(tl!("char-disabled-hint"))
                .pos(0., 0.15)
                .anchor(0.5, 0.5)
                .size(0.18)
                .color(semi_white(0.4))
                .draw();
            return Ok(());
        }

        // 模型预览（中间区域）
        if let Some(model) = &mut self.spine_model {
            unsafe { get_internal_gl() }.flush();
            let fader_p = s.fader.progress(rtime);
            let fader_alpha = 1. - fader_p.abs();
            let config = &get_data().config;
            model.position = glam::Vec2::new(
                -0.35 + config.character_model_offset_x,
                0.48 + config.character_model_offset_y,
            );
            model.scale = config.character_model_scale;
            model.alpha = fader_alpha;
            model.render();
            unsafe { get_internal_gl() }.flush();
        }

        if self.exiting {
            return Ok(());
        }

        let config = &get_data().config;

        // 抚摸区域可视化框
        let model_x = -0.35 + config.character_model_offset_x;
        let model_y = 0.48 + config.character_model_offset_y;
        let pet_rect = Rect::new(
            model_x + config.character_pet_offset_x - config.character_pet_width / 2.,
            model_y + config.character_pet_offset_y - config.character_pet_height / 2.,
            config.character_pet_width,
            config.character_pet_height,
        );
        ui.fill_path(&pet_rect.rounded(0.002), Color::new(0.2, 0.6, 1.0, 0.15));
        ui.stroke_path(&pet_rect.rounded(0.002), 0.003, Color::new(0.2, 0.6, 1.0, 0.6));

        // ===== 左侧分组栏 =====
        let gr = Self::group_rect();
        ui.fill_path(&gr.rounded(0.012), semi_black(0.55));
        ui.stroke_path(&gr.rounded(0.012), 0.003, semi_white(0.1));

        // 分组标题
        ui.text(tl!("char-group-title"))
            .pos(gr.center().x, gr.y + 0.025)
            .anchor(0.5, 0.5)
            .size(0.24)
            .color(semi_white(0.9))
            .draw();

        // 分组列表（带滚动和裁剪）
        let item_h = 0.06;
        let gap = 0.004;
        let list_top = gr.y + 0.06;
        let list_rect = Rect::new(gr.x, list_top, gr.w, gr.h - 0.08);
        ui.scissor(list_rect, |ui| {
            for (i, group) in self.groups.iter().enumerate() {
                let r = Rect::new(gr.x + 0.01, list_top + self.group_scroll_y + i as f32 * (item_h + gap), gr.w - 0.02, item_h);
                let selected = i == self.selected_group;
                if selected {
                    ui.fill_path(&r.rounded(0.008), Color::new(0.2, 0.5, 0.9, 0.6));
                    ui.stroke_path(&r.rounded(0.008), 0.003, Color::new(0.4, 0.7, 1.0, 0.8));
                }
                ui.text(&group.name)
                    .pos(r.x + 0.015, r.center().y)
                    .anchor(0., 0.5)
                    .size(0.2)
                    .max_width(r.w - 0.03)
                    .color(if selected { WHITE } else { semi_white(0.75) })
                    .draw();
                ui.text(format!("{}", group.models.len()))
                    .pos(r.right() - 0.015, r.center().y)
                    .anchor(1., 0.5)
                    .size(0.16)
                    .color(semi_white(0.4))
                    .draw();
            }
        });

        // ===== 右侧面板 =====
        let pr = Self::panel_rect();
        ui.fill_path(&pr.rounded(0.012), semi_black(0.55));
        ui.stroke_path(&pr.rounded(0.012), 0.003, semi_white(0.1));

        // 标签页
        let tab_y = pr.y + 0.01;
        let tab_h = 0.05;
        let tab_w = (pr.w - 0.04) / 3.;
        let tab_defs = [(DetailTab::Model, tl!("char-tab-model")), (DetailTab::Voice, tl!("char-tab-voice")), (DetailTab::Adjust, tl!("char-tab-adjust"))];
        for (i, (tab, label)) in tab_defs.iter().enumerate() {
            let r = Rect::new(pr.x + 0.02 + i as f32 * tab_w, tab_y, tab_w - 0.004, tab_h);
            let active = self.detail_tab == *tab;
            if active {
                ui.fill_path(&r.rounded(0.006), Color::new(0.2, 0.5, 0.9, 0.5));
            } else {
                ui.fill_path(&r.rounded(0.006), semi_black(0.3));
            }
            ui.text(label.as_ref())
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .size(0.2)
                .color(if active { WHITE } else { semi_white(0.7) })
                .draw();
        }

        // 内容区（带裁剪）
        let cr = Self::content_rect();
        let scroll_y = self.scroll_y;
        ui.scissor(cr, |ui| {
            ui.scope(|ui| {
                ui.dx(cr.x);
                ui.dy(cr.y + scroll_y);
                let mut y = 0.;
                let panel_x = 0.01;
                let panel_w = cr.w - 0.02;

                match self.detail_tab {
                    DetailTab::Model => {
                        // 搜索框
                        let search_r = Rect::new(panel_x, y, panel_w, HEADER_H);
                        ui.fill_path(&search_r.rounded(0.006), semi_black(0.4));
                        ui.stroke_path(&search_r.rounded(0.006), 0.002, if self.search_active { Color::new(0.4, 0.7, 1.0, 0.8) } else { semi_white(0.15) });
                        let search_text = if self.search_query.is_empty() {
                            tl!("char-search-placeholder").to_string()
                        } else {
                            self.search_query.clone()
                        };
                        ui.text(search_text)
                            .pos(search_r.x + 0.015, search_r.center().y)
                            .anchor(0., 0.5)
                            .size(0.18)
                            .color(if self.search_query.is_empty() { semi_white(0.4) } else { WHITE })
                            .draw();
                        y += HEADER_H + GAP;

                        // 导入模型
                        let import_r = Rect::new(panel_x, y, panel_w, ITEM_H);
                        let model_text = if let Some(p) = &config.character_model_path {
                            tl!("char-model", "name" => Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or("custom").to_string())
                        } else {
                            tl!("char-model-default").to_string()
                        };
                        self.import_btn.build(ui, t, import_r, |ui, _| {
                            ui.fill_path(&import_r.rounded(0.008), Color::new(0.15, 0.4, 0.7, 0.5));
                            ui.stroke_path(&import_r.rounded(0.008), 0.002, Color::new(0.3, 0.6, 1.0, 0.5));
                            ui.text(model_text)
                                .pos(import_r.x + 0.02, import_r.center().y)
                                .anchor(0., 0.5)
                                .size(0.18)
                                .draw();
                        });
                        y += ITEM_H + GAP;

                        // 模型列表
                        let filtered = self.filtered_models();
                        if filtered.is_empty() {
                            ui.text(tl!("char-no-match"))
                                .pos(panel_x + panel_w / 2., y + 0.05)
                                .anchor(0.5, 0.)
                                .size(0.18)
                                .color(semi_white(0.4))
                                .draw();
                        }
                        for (name, path, btn_idx) in filtered {
                            let model_r = Rect::new(panel_x, y, panel_w, ITEM_H);
                            let is_current = config.character_model_path.as_deref() == Some(path.as_str());
                            self.model_btns[btn_idx].build(ui, t, model_r, |ui, _| {
                                if is_current {
                                    ui.fill_path(&model_r.rounded(0.008), Color::new(0.2, 0.5, 0.9, 0.55));
                                    ui.stroke_path(&model_r.rounded(0.008), 0.002, Color::new(0.4, 0.7, 1.0, 0.8));
                                } else {
                                    ui.fill_path(&model_r.rounded(0.008), semi_black(0.35));
                                }
                                ui.text(&name)
                                    .pos(model_r.x + 0.02, model_r.center().y)
                                    .anchor(0., 0.5)
                                    .size(0.17)
                                    .max_width(model_r.w - 0.04)
                                    .color(if is_current { WHITE } else { semi_white(0.8) })
                                    .draw();
                            });
                            y += ITEM_H + GAP;
                        }
                    }
                    DetailTab::Voice => {
                        let voice_r = Rect::new(panel_x, y, panel_w, ITEM_H);
                        let voice_text = if let Some(p) = &config.character_voice_dir {
                            tl!("char-voice", "name" => Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or("custom").to_string())
                        } else {
                            tl!("char-voice-default").to_string()
                        };
                        self.voice_btn.build(ui, t, voice_r, |ui, _| {
                            ui.fill_path(&voice_r.rounded(0.008), semi_black(0.35));
                            ui.text(voice_text)
                                .pos(voice_r.x + 0.02, voice_r.center().y)
                                .anchor(0., 0.5)
                                .size(0.18)
                                .draw();
                        });
                        y += ITEM_H + 0.02;
                        ui.text(tl!("char-voice-hint"))
                            .pos(panel_x, y)
                            .anchor(0., 0.)
                            .size(0.15)
                            .color(semi_white(0.45))
                            .max_width(panel_w)
                            .draw();
                    }
                    DetailTab::Adjust => {
                        // 名字
                        let name_r = Rect::new(panel_x, y, panel_w, ITEM_H);
                        self.name_btn.build(ui, t, name_r, |ui, _| {
                            ui.fill_path(&name_r.rounded(0.008), semi_black(0.35));
                            ui.text(tl!("char-name", "name" => config.character_name.clone()))
                                .pos(name_r.x + 0.02, name_r.center().y)
                                .anchor(0., 0.5)
                                .size(0.18)
                                .draw();
                        });
                        y += ITEM_H + GAP;

                        // 默认表情
                        let def_r = Rect::new(panel_x, y, panel_w, ITEM_H);
                        self.default_expr_btn.build(ui, t, def_r, |ui, _| {
                            ui.fill_path(&def_r.rounded(0.008), semi_black(0.35));
                            ui.text(tl!("char-default-expr", "expr" => config.character_default_expr.to_string()))
                                .pos(def_r.x + 0.02, def_r.center().y)
                                .anchor(0., 0.5)
                                .size(0.18)
                                .draw();
                        });
                        y += ITEM_H + GAP;

                        // 抚摸表情
                        let pet_r = Rect::new(panel_x, y, panel_w, ITEM_H);
                        self.pet_expr_btn.build(ui, t, pet_r, |ui, _| {
                            ui.fill_path(&pet_r.rounded(0.008), semi_black(0.35));
                            ui.text(tl!("char-pet-expr", "expr" => config.character_pet_expr.to_string()))
                                .pos(pet_r.x + 0.02, pet_r.center().y)
                                .anchor(0., 0.5)
                                .size(0.18)
                                .draw();
                        });
                        y += ITEM_H + GAP;

                        // 滑动条
                        let slider_track_x = panel_x + 0.16;
                        let slider_track_w = panel_w - 0.18;
                        let slider_labels = [
                            tl!("char-model-scale"),
                            tl!("char-model-x"),
                            tl!("char-model-y"),
                            tl!("char-pet-x"),
                            tl!("char-pet-y"),
                            tl!("char-pet-w"),
                            tl!("char-pet-h"),
                        ];
                        let slider_values = [
                            format!("{:.6}", config.character_model_scale),
                            format!("{:.3}", config.character_model_offset_x),
                            format!("{:.3}", config.character_model_offset_y),
                            format!("{:.3}", config.character_pet_offset_x),
                            format!("{:.3}", config.character_pet_offset_y),
                            format!("{:.3}", config.character_pet_width),
                            format!("{:.3}", config.character_pet_height),
                        ];
                        let vals = [
                            config.character_model_scale,
                            config.character_model_offset_x,
                            config.character_model_offset_y,
                            config.character_pet_offset_x,
                            config.character_pet_offset_y,
                            config.character_pet_width,
                            config.character_pet_height,
                        ];
                        let slider_ranges: [(f32, f32); 7] = [
                            (0.0001, 0.002),
                            (-0.5, 0.5),
                            (-0.5, 0.5),
                            (-0.5, 0.5),
                            (-1.0, 0.0),
                            (0.1, 1.0),
                            (0.1, 1.0),
                        ];
                        for i in 0..7 {
                            let row_y = y + i as f32 * (ITEM_H + GAP);
                            ui.text(slider_labels[i].clone())
                                .pos(panel_x, row_y + ITEM_H / 2.)
                                .anchor(0., 0.5)
                                .size(0.15)
                                .draw();
                            ui.text(slider_values[i].clone())
                                .pos(panel_x + 0.14, row_y + ITEM_H / 2.)
                                .anchor(1., 0.5)
                                .size(0.13)
                                .color(semi_white(0.55))
                                .draw();
                            let track = Rect::new(slider_track_x, row_y + ITEM_H * 0.3, slider_track_w, ITEM_H * 0.4);
                            ui.fill_path(&track.rounded(0.004), semi_black(0.4));
                            let (min, max) = slider_ranges[i];
                            let p = ((vals[i] - min) / (max - min)).clamp(0., 1.);
                            let fill = Rect::new(track.x, track.y, track.w * p, track.h);
                            ui.fill_path(&fill.rounded(0.004), Color::new(0.3, 0.6, 1.0, 0.7));
                            let knob_x = track.x + track.w * p;
                            ui.fill_circle(knob_x, track.center().y, 0.011, semi_white(0.95));
                        }
                        y += 7.0 * (ITEM_H + GAP);

                        // 重置
                        let reset_r = Rect::new(panel_x, y, panel_w, ITEM_H);
                        self.reset_btn.build(ui, t, reset_r, |ui, _| {
                            ui.fill_path(&reset_r.rounded(0.008), Color::new(0.7, 0.2, 0.2, 0.7));
                            ui.text(tl!("char-reset"))
                                .pos(reset_r.center().x, reset_r.center().y)
                                .anchor(0.5, 0.5)
                                .size(0.22)
                                .color(WHITE)
                                .draw();
                        });
                    }
                }
            });
        });

        // 滚动条
        let total_h = self.tab_content_height(self.detail_tab);
        if total_h > cr.h {
            let bar_w = 0.006;
            let bar_x = cr.right() - bar_w - 0.002;
            let bar_h = cr.h * (cr.h / total_h);
            let bar_y = cr.y + (-self.scroll_y / total_h) * cr.h;
            ui.fill_path(&Rect::new(bar_x, bar_y, bar_w, bar_h).rounded(0.003), semi_white(0.3));
        }

        Ok(())
    }

    fn on_back_pressed(&mut self, _s: &mut SharedState) -> bool {
        self.exiting = true;
        false
    }

    fn next_page(&mut self) -> NextPage {
        self.next_page.take().unwrap_or(NextPage::None)
    }
}

// ===== 各 tab 的 touch 实现 =====

impl CharacterPage {
    fn touch_model_tab(&mut self, touch: &Touch, t: f32, cr: Rect, base_y: f32) -> Result<bool> {
        let panel_x = cr.x + 0.01;
        let panel_w = cr.w - 0.02;
        let mut y = base_y + HEADER_H + GAP;

        // 导入模型
        let import_r = Rect::new(panel_x, y, panel_w, ITEM_H);
        if self.import_btn.touch(touch, t) {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                let path_str = path.to_string_lossy().to_string();
                let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Character").to_string();
                self.switch_model(path_str, folder_name);
            }
            return Ok(true);
        }
        y += ITEM_H + GAP;

        // 模型列表
        let filtered = self.filtered_models();
        for (name, path, btn_idx) in filtered {
            let model_r = Rect::new(panel_x, y, panel_w, ITEM_H);
            if self.model_btns[btn_idx].touch(touch, t) {
                self.switch_model(path, name);
                return Ok(true);
            }
            y += ITEM_H + GAP;
        }

        Ok(false)
    }

    fn touch_voice_tab(&mut self, touch: &Touch, t: f32, cr: Rect, base_y: f32) -> Result<bool> {
        let panel_x = cr.x + 0.01;
        let panel_w = cr.w - 0.02;
        let y = base_y;

        let voice_r = Rect::new(panel_x, y, panel_w, ITEM_H);
        if self.voice_btn.touch(touch, t) {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                get_data_mut().config.character_voice_dir = Some(path.to_string_lossy().to_string());
                save_data()?;
            }
            return Ok(true);
        }

        Ok(false)
    }

    fn touch_adjust_tab(&mut self, touch: &Touch, t: f32, cr: Rect, base_y: f32) -> Result<bool> {
        let panel_x = cr.x + 0.01;
        let panel_w = cr.w - 0.02;
        let mut y = base_y;

        // 名字
        let name_r = Rect::new(panel_x, y, panel_w, ITEM_H);
        if self.name_btn.touch(touch, t) {
            let name = get_data().config.character_name.clone();
            request_input("char_name", InputBox::new().default_text(&name));
            return Ok(true);
        }
        y += ITEM_H + GAP;

        // 默认表情
        let def_r = Rect::new(panel_x, y, panel_w, ITEM_H);
        if self.default_expr_btn.touch(touch, t) {
            let v = get_data().config.character_default_expr.to_string();
            request_input("char_default_expr", InputBox::new().default_text(&v));
            return Ok(true);
        }
        y += ITEM_H + GAP;

        // 抚摸表情
        let pet_r = Rect::new(panel_x, y, panel_w, ITEM_H);
        if self.pet_expr_btn.touch(touch, t) {
            let v = get_data().config.character_pet_expr.to_string();
            request_input("char_pet_expr", InputBox::new().default_text(&v));
            return Ok(true);
        }
        y += ITEM_H + GAP;

        // 滑动条
        let slider_track_x = panel_x + 0.16;
        let slider_track_w = panel_w - 0.18;
        let slider_ranges: [(f32, f32, f32); 7] = [
            (0.0001, 0.002, 0.00005),
            (-0.5, 0.5, 0.01),
            (-0.5, 0.5, 0.01),
            (-0.5, 0.5, 0.01),
            (-1.0, 0.0, 0.01),
            (0.1, 1.0, 0.01),
            (0.1, 1.0, 0.01),
        ];

        if touch.phase == TouchPhase::Started {
            for i in 0..7 {
                let sy = y + i as f32 * (ITEM_H + GAP);
                let hit = Rect::new(slider_track_x - 0.02, sy, slider_track_w + 0.04, ITEM_H);
                if hit.contains(touch.position) {
                    self.active_slider = Some(i);
                    let track = Rect::new(slider_track_x, sy + ITEM_H * 0.3, slider_track_w, ITEM_H * 0.4);
                    let pct = ((touch.position.x - track.x) / track.w).clamp(0., 1.);
                    let (min, max, step) = slider_ranges[i];
                    let v = ((min + (max - min) * pct) / step).round() * step;
                    self.set_slider_value(i, v);
                    save_data()?;
                    return Ok(true);
                }
            }
        }
        y += 7.0 * (ITEM_H + GAP);

        // 重置
        let reset_r = Rect::new(panel_x, y, panel_w, ITEM_H);
        if self.reset_btn.touch(touch, t) {
            let data = get_data_mut();
            data.config.character_model_path = None;
            data.config.character_name = "星野(临战)".to_string();
            data.config.character_default_expr = 0;
            data.config.character_pet_expr = 24;
            data.config.character_model_offset_x = 0.;
            data.config.character_model_offset_y = 0.;
            data.config.character_model_scale = 0.00065;
            data.config.character_pet_offset_x = 0.;
            data.config.character_pet_offset_y = -0.650;
            data.config.character_pet_width = 0.3;
            data.config.character_pet_height = 0.3;
            data.config.character_voice_dir = None;
            self.spine_raw_task = Some(Task::new(crate::spine_model::load_default_model_data()));
            self.spine_model = None;
            self.spine_anim_dirty = true;
            save_data()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn touch_adjust_sliders(&mut self, touch: &Touch, cr: Rect, base_y: f32) -> Result<bool> {
        let idx = self.active_slider.unwrap();
        let panel_x = cr.x + 0.01;
        let panel_w = cr.w - 0.02;
        let slider_track_x = panel_x + 0.16;
        let slider_track_w = panel_w - 0.18;
        let slider_ranges: [(f32, f32, f32); 7] = [
            (0.0001, 0.002, 0.00005),
            (-0.5, 0.5, 0.01),
            (-0.5, 0.5, 0.01),
            (-0.5, 0.5, 0.01),
            (-1.0, 0.0, 0.01),
            (0.1, 1.0, 0.01),
            (0.1, 1.0, 0.01),
        ];

        match touch.phase {
            TouchPhase::Moved => {
                let sy = base_y + 3.0 * (ITEM_H + GAP) + idx as f32 * (ITEM_H + GAP);
                let track = Rect::new(slider_track_x, sy + ITEM_H * 0.3, slider_track_w, ITEM_H * 0.4);
                let pct = ((touch.position.x - track.x) / track.w).clamp(0., 1.);
                let (min, max, step) = slider_ranges[idx];
                let v = ((min + (max - min) * pct) / step).round() * step;
                self.set_slider_value(idx, v);
                save_data()?;
                Ok(true)
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.active_slider = None;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
