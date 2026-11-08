use super::{
    draw_disabled_zones, draw_zones_with_touches,
    line::{run_line_note_pass, LineNoteView},
    note::{FrameGeo, NoteVertexItem, StagingSink},
    note::FADEOUT_TIME,
    BlockArea, BlockRotateEvent, BpmList, CtrlObject, Effect, JudgeLine, JudgeLineKind, Matrix, Note, NoteKind, Point, Resource, UIElement, Vector, Zone, zone_on_screen, zone_on_screen_lim,
};
use crate::{core::Object, fs::FileSystem, judge::JudgeStatus, parallel::POOL, ui::Ui};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use nalgebra::Rotation2;
use sasa::AudioClip;
use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub struct ChartExtra {
    pub effects: Vec<Effect>,
    pub global_effects: Vec<Effect>,
    #[cfg(feature = "video")]
    pub videos: Vec<(super::Video, Option<super::VideoAttach>)>,
    pub lyrics: Vec<crate::parse::lyrics::LyricLine>,
}

#[derive(Default)]
pub struct ChartSettings {
    pub pe_alpha_extension: bool,
    pub hold_partial_cover: bool,
}

pub type HitSoundMap = HashMap<String, AudioClip>;

pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
    pub bpm_list: RefCell<BpmList>,

    pub settings: ChartSettings,
    pub extra: ChartExtra,

    pub arcaea_judgement: bool,

    pub fnf_judgement: bool,

    pub order: Vec<usize>,

    pub attach_ui: [Option<usize>; 7],

    pub hitsounds: HitSoundMap,

    /// 噪域（blockArea）区域对象，由谱面 blockAreaList 解析而来。
    pub block_areas: Vec<BlockArea>,
    /// RNPE/rpe-next 噪域；坐标与时间语义和原生 BlockArea 不同。
    pub next_block_areas: Vec<super::rpe_next_block::Area>,
    pub next_single_inversion: bool,
    next_block_frame: RefCell<(Option<(f64, f32, usize)>, Vec<super::rpe_next_block::Sample>)>,
    /// Chart-space positions of touches blocked by the zones this frame.
    pub blocked_touches: Vec<(u64, Vector)>,
    block_frame: RefCell<BlockFrame>,
    rpe_block_markers: Vec<usize>,
    line_transforms: Vec<Matrix>,
    line_rotations: Vec<f32>,

    note_staging: RefCell<Vec<Vec<NoteVertexItem>>>,
}

#[derive(Default)]
struct BlockFrame {
    timeline: super::block_timeline::BlockTimeline,
    key: Option<(f64, f32, usize)>,
    zones: Vec<Zone>,
}

impl Chart {
    pub fn new(
        offset: f32,
        lines: Vec<JudgeLine>,
        bpm_list: BpmList,
        settings: ChartSettings,
        extra: ChartExtra,
        hitsounds: HitSoundMap,
        block_areas: Vec<BlockArea>,
    ) -> Self {
        let mut attach_ui = [None; 7];
        let mut order = (0..lines.len())
            .filter(|it| {
                if let Some(element) = lines[*it].attach_ui {
                    attach_ui[element as usize - 1] = Some(*it);
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<_>>();
        order.sort_by_key(|it| (lines[*it].z_index, *it));
        let rpe_block_markers = lines
            .iter()
            .enumerate()
            .filter_map(|(id, line)| match &line.kind {
                JudgeLineKind::Texture(_, path)
                    if matches!(path.rsplit(['/', '\\']).next().unwrap_or("").to_ascii_lowercase().as_str(), "issubtract0.png" | "issubtract1.png") =>
                {
                    Some(id)
                }
                _ => None,
            })
            .collect();
        Self {
            offset,
            lines,
            bpm_list: RefCell::new(bpm_list),
            settings,
            extra,

            arcaea_judgement: false,
            fnf_judgement: false,

            order,
            attach_ui,

            hitsounds,
            block_areas,
            next_block_areas: Vec::new(),
            next_single_inversion: false,
            next_block_frame: RefCell::default(),
            blocked_touches: Vec::new(),
            block_frame: RefCell::default(),
            rpe_block_markers,
            line_transforms: Vec::new(),
            line_rotations: Vec::new(),
            note_staging: RefCell::new(Vec::new()),
        }
    }

    #[inline]
    pub fn with_element<R>(
        &self,
        ui: &mut Ui,
        res: &Resource,
        element: UIElement,
        scale_point: Option<(f32, f32)>,
        rotation_point: (f32, f32),
        f: impl FnOnce(&mut Ui, Color) -> R,
    ) -> R {
        let scale_point = scale_point.unwrap_or(rotation_point);
        if let Some(id) = self.attach_ui[element as usize - 1] {
            let lines = &self.lines;
            let line = &lines[id];
            let obj = &line.object;
            let mut tr = line.fetch_pos(res, lines);
            tr.y = -tr.y;
            let color = self.lines[id].color.now_opt().unwrap_or(WHITE);
            let scale = obj.now_scale(Vector::new(scale_point.0, scale_point.1));
            let ro =
                Object::new_rotation_wrt_point(Rotation2::new(-obj.rotation.now().to_radians()), Vector::new(rotation_point.0, rotation_point.1));
            ui.with(Matrix::new_translation(&tr) * ro * scale, |ui| ui.alpha(obj.now_alpha().max(0.), |ui| f(ui, color)))
        } else {
            f(ui, WHITE)
        }
    }

    pub async fn load_textures(&mut self, fs: &mut dyn FileSystem) -> Result<()> {
        for line in &mut self.lines {
            if let JudgeLineKind::Texture(tex, path) = &mut line.kind {
                *tex = image::load_from_memory(&fs.load_file(path).await.with_context(|| format!("failed to load illustration {path}"))?)?.into();
            }
        }
        Ok(())
    }

    pub fn set_next_block_areas(&mut self, areas: Vec<super::rpe_next_block::Area>, single_inversion: bool) {
        self.next_block_areas = areas;
        self.next_single_inversion = single_inversion;
        self.next_block_frame.get_mut().0 = None;
    }

    pub fn has_block_areas(&self) -> bool {
        !self.block_areas.is_empty() || !self.rpe_block_markers.is_empty() || !self.next_block_areas.is_empty()
    }

    pub fn next_block_samples(&self, time: f64, aspect: f32) -> Vec<super::rpe_next_block::Sample> {
        let key = (time, aspect, self.next_block_areas.len());
        let mut cache = self.next_block_frame.borrow_mut();
        if cache.0 == Some(key) {
            return cache.1.clone();
        }
        let mut samples: Vec<_> = self
            .next_block_areas
            .iter()
            .filter_map(|area| area.sample(time, aspect))
            .collect();
        samples.sort_by_key(|s| s.layer);
        cache.0 = Some(key);
        cache.1 = samples.clone();
        samples
    }

    pub fn reset(&mut self) {
        self.blocked_touches.clear();
        super::reset_block_effects();
        self.lines
            .iter_mut()
            .flat_map(|it| it.notes.iter_mut())
            .for_each(|note| {
                note.judge = JudgeStatus::NotJudged;
                note.protected = false;
            });
        for line in &mut self.lines {
            line.cache.reset(&mut line.notes);
        }
        #[cfg(feature = "video")]
        for (video, _) in &mut self.extra.videos {
            if let Err(err) = video.reset() {
                use crate::parse::{ptl, L10N_LOCAL};
                crate::scene::show_error(err.context(ptl!("video-load-failed", "path" => video.video_file.path().to_string_lossy())));
            }
        }
    }

    pub fn update(&mut self, res: &mut Resource) {
        for line in &mut self.lines {
            line.object.set_time(res.time);
        }
        self.line_transforms.clear();
        self.line_rotations.clear();
        self.line_transforms
            .extend(self.lines.iter().map(|it| it.now_transform(res, &self.lines)));
        self.line_rotations.extend(self.lines.iter().map(|it| it.fetch_rot(&self.lines)));
        for ((line, tr), rot) in self.lines.iter_mut().zip(&self.line_transforms).zip(&self.line_rotations) {
            line.update(res, *tr, *rot);
        }
        for effect in &mut self.extra.effects {
            effect.update(res);
        }
        #[cfg(feature = "video")]
        for (video, _) in &mut self.extra.videos {
            if let Err(err) = video.update(res.time) {
                tracing::warn!("video error: {err:?}");
            }
        }
    }

    pub fn load_metrics(&self, res: &Resource) -> (usize, usize) {
        let t = res.time;
        let end = t + 1.0;

        let visible_end = t + 5.0;

        let workers = POOL.worker_count();
        let active_total: usize = self.lines.iter().map(|it| it.cache.active_ids().len()).sum();
        if active_total == 0 {
            return (0, 0);
        }

        if workers > 1 && res.config.metrics_parallel() && active_total > res.config.metrics_parallel_min() {

            let mut jobs: Vec<ActiveMetricsJob> = Vec::with_capacity(self.lines.len());
            for line in &self.lines {
                let ids = line.cache.active_ids();
                if !ids.is_empty() {
                    jobs.push(ActiveMetricsJob {
                        notes: line.notes.as_ptr(),
                        notes_len: line.notes.len(),
                        ids: ids.as_ptr(),
                        len: ids.len(),
                        visible: 0,
                        upcoming: 0,
                    });
                }
            }
            POOL.scoped_parallel_for(&mut jobs, |job| {
                let notes = unsafe { std::slice::from_raw_parts(job.notes, job.notes_len) };
                let ids = unsafe { std::slice::from_raw_parts(job.ids, job.len) };
                let mut visible = 0usize;
                let mut upcoming = 0usize;
                for id in ids {
                    let (v, u) = note_metric(&notes[*id as usize], t, end, visible_end);
                    visible += v;
                    upcoming += u;
                }
                job.visible = visible;
                job.upcoming = upcoming;
            });
            let mut visible = 0usize;
            let mut upcoming = 0usize;
            for job in jobs {
                visible += job.visible;
                upcoming += job.upcoming;
            }
            (visible, upcoming)
        } else {

            let mut visible = 0usize;
            let mut upcoming = 0usize;
            for line in &self.lines {
                let notes = &line.notes;
                for id in line.cache.active_ids() {
                    let (v, u) = note_metric(&notes[*id as usize], t, end, visible_end);
                    visible += v;
                    upcoming += u;
                }
            }
            (visible, upcoming)
        }
    }

    pub fn render(&self, ui: &mut Ui, res: &mut Resource) {
        #[cfg(feature = "video")]
        for (video, attach) in &self.extra.videos {
            if let Some(attach) = attach {
                let line = &self.lines[attach.line];
                let color = line.color.now_opt().unwrap_or(res.judge_line_color);
                let mat = self.lines[attach.line].object.now(res);
                res.apply_model_of(&mat, |res| {
                    video.render(res.time, res.aspect_ratio, color);
                });
            } else {
                video.render(res.time, res.aspect_ratio, WHITE);
            }
        }

        let mirror_x = res.config.flip_x();
        let mirror_y = res.config.flip_y();
        res.apply_model_of(
            &Matrix::identity().append_nonuniform_scaling(&Vector::new(if mirror_x { -1. } else { 1. }, if mirror_y { 1. } else { -1. })),
            |res| {

                // Native Background sorting layer order 2, judge lines order 3.
                let zones = Self::scale_zones_for_render(res, &self.block_zones(res));
                draw_disabled_zones(res, res.aspect_ratio, &zones);
                let mut guard = self.bpm_list.borrow_mut();
                for id in &self.order {
                    self.lines[*id].render(ui, res, &self.lines, &mut guard, &self.settings, *id);
                }
                drop(guard);

                res.note_buffer.borrow_mut().draw_all();
                if res.config.sample_count > 1 {
                    unsafe { get_internal_gl() }.flush();
                    if let Some(target) = &res.chart_target {
                        target.blit();
                    }
                }
                if !res.no_effect {
                    let render = |res: &mut Resource| {
                        for effect in &self.extra.effects {
                            effect.render(res);
                        }
                    };
                    if mirror_x || mirror_y {
                        res.apply_model_of(
                            &Matrix::identity().append_nonuniform_scaling(&Vector::new(if mirror_x { -1. } else { 1. }, if mirror_y { -1. } else { 1. })),
                            render,
                        );
                    } else {
                        render(res);
                    }
                }
            },
        );
    }

    #[allow(dead_code)]
    fn render_notes(&self, res: &mut Resource) {
        let order = &self.order;
        let n = order.len();
        if n == 0 {
            return;
        }
        let bpm = self.bpm_list.borrow();
        let frame = FrameGeo::snapshot(res, &self.settings, Some(&bpm));
        let mut buffers = self.note_staging.borrow_mut();
        if buffers.len() < n {
            buffers.resize_with(n, Vec::new);
        }

        struct Job<'l> {
            view: LineNoteView<'l>,
            ctrl: CtrlObject,
            out: Vec<NoteVertexItem>,
            line_id: usize,
            last_h: Option<f64>,
        }

        let mut jobs: Vec<Job> = Vec::with_capacity(n);
        for (k, id) in order.iter().enumerate() {
            let line = &self.lines[*id];
            let view = line.note_view(&*res, &self.lines, &frame);
            let ctrl = line.ctrl_obj.borrow().clone();
            let out = std::mem::take(&mut buffers[k]);
            jobs.push(Job {
                view,
                ctrl,
                out,
                line_id: *id,
                last_h: None,
            });
        }

        let run = |job: &mut Job| {
            let last = run_line_note_pass(&frame, &job.view, &mut job.ctrl, &mut StagingSink(&mut job.out));
            job.last_h = last;
        };
        let parallel = cfg!(not(target_arch = "wasm32")) && POOL.worker_count() > 1 && jobs.len() > 1;
        if parallel {
            POOL.scoped_parallel_for(&mut jobs, run);
        } else {
            for job in &mut jobs {
                run(job);
            }
        }

        for job in &jobs {
            if let Some(h) = job.last_h {
                self.lines[job.line_id].ctrl_obj.borrow_mut().set_height(h);
            }
        }

        let mut nb = res.note_buffer.borrow_mut();
        for (k, mut job) in jobs.into_iter().enumerate() {
            let out = std::mem::take(&mut job.out);
            for item in &out {
                nb.push(item.key, item.verts);
            }
            buffers[k] = out;
        }
    }

    /// [谱面揭晓] 读取模型栈上各帧均匀缩放的累积乘积（取绝对值容忍镜像翻转），
    /// 得到当前谱面相对全屏 NDC 的缩放系数。噪域 mask 光栅化与最终合成均在
    /// 全屏 NDC 空间完成（官方 shader 无 Model uniform，全屏矩形顶点为 CPU 直算），
    /// 不会随 model_stack 缩放，须用该系数把 Zone 几何同倍缩小后再光栅化，
    /// 使噪域随判定线/note 一起缩进游玩区域。
    fn current_zone_scale(res: &Resource) -> f32 {
        res.model_stack
            .iter()
            .fold(1.0f32, |acc, m| acc * m.m11)
            .abs()
    }

    /// [谱面揭晓] 按当前模型栈缩放系数缩小 Zone 列表，并在缩小后的空间重新执行
    /// 屏幕外剔除。缩放后 Zone 变小、Reveal 视野扩大，原始 NDC 剔除（zone_on_screen
    /// 按全屏 [-1,1] 边界判断）会把本应可见的边缘 Zone 丢弃，故须在缩放坐标上
    /// 重新判定可见性；非缩放路径保持原 Zone 原样返回。
    fn scale_zones_for_render(res: &Resource, zones: &[Zone]) -> Vec<Zone> {
        let scale = Self::current_zone_scale(res);
        if (scale - 1.).abs() > 1e-4 {
            zones.iter()
                .map(|z| Zone { center: z.center * scale, half: z.half * scale, ..*z })
                .filter(|z| zone_on_screen(z, res.aspect_ratio))
                .collect()
        } else {
            zones.iter().cloned().collect()
        }
    }

    pub fn render_block_overlay(&self, res: &mut Resource) {
        if !res.config.noise_enabled {
            return;
        }
        let flip_x = res.config.flip_x();
        let mut zones = Self::scale_zones_for_render(res, &self.block_zones(res));
        zones.extend(
            self.next_block_samples(res.time, res.aspect_ratio)
                .into_iter()
                .filter(|sample| super::zone_on_screen_lim(&sample.zone, res.aspect_ratio, 1.))
                .map(|sample| sample.zone),
        );
        res.apply_model_of(&Matrix::identity().append_nonuniform_scaling(&Vector::new(if flip_x { -1. } else { 1. }, -1.)), |res| {
            draw_zones_with_touches(res, res.aspect_ratio, &zones, &self.blocked_touches, flip_x);
        });
    }

    fn block_zones(&self, res: &Resource) -> std::cell::Ref<'_, [Zone]> {
        // [谱面揭晓] Reveal 全景缩放下视野扩大 1/scale 倍，首层剔除必须使用
        // 扩大后的边界（zone_on_screen_lim），否则全景视野内但全屏 NDC 之外的
        // Zone 会被误丢弃，噪域边缘宽度缺失并随 Zone 动画在边界处频繁闪烁。
        // 第二层 scale_zones_for_render 仍按缩放后的 ±1 边界剔除，不受影响。
        let scale = Self::current_zone_scale(res);
        let cull_lim = if (scale - 1.).abs() > 1e-4 { 1. / scale } else { 1. };
        let key = (res.time, res.aspect_ratio, self.block_areas.len());
        let mut cache = self.block_frame.borrow_mut();
        if cache.key != Some(key) {
            let BlockFrame { timeline, zones, .. } = &mut *cache;
            zones.clear();
            zones.extend(
                timeline
                    .at(&self.block_areas, res.time)
                    .iter()
                    .filter_map(|&id| Zone::from_area(&self.block_areas[id], res.time, res.aspect_ratio))
                    // Render-only cull: fully off-screen zones skip mask
                    // rasterization and the full-screen effect pass. Judgement
                    // never reads Zone, so touch blocking is unaffected.
                    .filter(|zone| zone_on_screen_lim(zone, res.aspect_ratio, cull_lim)),
            );
            let marker_areas = self.rpe_marker_areas(res.aspect_ratio);
            zones.extend(
                marker_areas
                    .iter()
                    .filter_map(|area| Zone::from_area(area, res.time, res.aspect_ratio))
                    .filter(|zone| zone_on_screen_lim(zone, res.aspect_ratio, cull_lim)),
            );
            cache.key = Some(key);
        }
        drop(cache);
        std::cell::Ref::map(self.block_frame.borrow(), |cache| cache.zones.as_slice())
    }

    pub(crate) fn touch_blocked(&self, p: Vector, time: f64, aspect: f32) -> bool {
        let marker_areas = self.rpe_marker_areas(aspect);
        let mut cache = self.block_frame.borrow_mut();
        let areas = cache
            .timeline
            .at(&self.block_areas, time)
            .iter()
            .map(|&id| &self.block_areas[id])
            .chain(marker_areas.iter());
        super::block::block_touch_blocked_iter(areas, p, time, aspect)
            || super::rpe_next_block::blocked(
                &self.next_block_samples(time, aspect),
                p,
                aspect,
                self.next_single_inversion,
            )
    }

    /// RPE charts commonly encode block areas as opaque texture lines named
    /// `isSubtract0.png` (normal) / `isSubtract1.png` (subtract). Resolve those
    /// animated lines into the same rectangle model used by official block areas.
    fn rpe_marker_areas(&self, aspect: f32) -> Vec<BlockArea> {
        self.rpe_block_markers
            .iter()
            .filter_map(|&id| {
                let line = self.lines.get(id)?;
                let (texture, path) = match &line.kind {
                    JudgeLineKind::Texture(texture, path) => (texture, path),
                    _ => return None,
                };
                let is_subtract = match path.rsplit(['/', '\\']).next()?.to_ascii_lowercase().as_str() {
                    "issubtract0.png" => false,
                    "issubtract1.png" => true,
                    _ => return None,
                };
                let alpha = line.object.alpha.now_opt().unwrap_or(1.).max(0.);
                let scale = line.object.scale.now_with_def(1., 1.);
                if scale.x.abs() < 1e-6 || scale.y.abs() < 1e-6 || texture.width() <= 0. || texture.height() <= 0. {
                    return None;
                }
                let center = self
                    .line_transforms
                    .get(id)
                    .map(|matrix| matrix.transform_point(&Point::new(0., 0.)))
                    .unwrap_or_else(|| {
                        let mut pos = line.object.translation.now();
                        pos.y /= aspect;
                        Point::new(pos.x, pos.y)
                    });
                let rotation = self.line_rotations.get(id).copied().unwrap_or_else(|| line.object.rotation.now());
                let half_x = texture.width() * scale.x.abs() * 0.5;
                let half_y = texture.height() * scale.y.abs() * 0.5;
                let to_pct_x = |x: f32| (x + 1.) * 0.5;
                let to_pct_y = |y: f32| (y * aspect + 1.) * 0.5;
                let c = Vector::new(center.x, center.y);
                let top_right = Vector::new(to_pct_x(c.x + half_x), to_pct_y(c.y + half_y));
                let bottom_left = Vector::new(to_pct_x(c.x - half_x), to_pct_y(c.y - half_y));
                let anchor = Vector::new(to_pct_x(c.x), to_pct_y(c.y));
                Some(BlockArea {
                    top_right,
                    bottom_left,
                    appear_time: if alpha > 0. { 0. } else { f64::INFINITY },
                    enable_time: if alpha > 0. { 0. } else { f64::INFINITY },
                    disable_time: f64::INFINITY,
                    disappear_time: f64::INFINITY,
                    is_subtract,
                    rotate_events: vec![BlockRotateEvent {
                        anchor,
                        time: 0.,
                        ease: 0,
                        rotation,
                    }],
                    move_events: Vec::new(),
                    scale_events: Vec::new(),
                    active_intervals: Vec::new(),
                    absolute_scale: false,
                    base_px: Vector::zeros(),
                    move_x_events: Vec::new(),
                    move_y_events: Vec::new(),
                })
            })
            .collect()
    }
}

#[inline]
fn note_metric(note: &Note, t: f64, end: f64, visible_end: f64) -> (usize, usize) {
    if matches!(note.judge, JudgeStatus::Judged) && !matches!(note.kind, NoteKind::Hold { .. }) {
        return (0, 0);
    }
    let mut visible = 0usize;
    if t - FADEOUT_TIME < note.time && note.time <= visible_end {
        visible = 1;
    }
    let mut upcoming = 0usize;
    if matches!(note.judge, JudgeStatus::NotJudged) && !note.fake && note.time >= t && note.time <= end {
        upcoming = 1;
    }
    (visible, upcoming)
}

struct ActiveMetricsJob {
    notes: *const Note,
    notes_len: usize,
    ids: *const u32,
    len: usize,
    visible: usize,
    upcoming: usize,
}

unsafe impl Send for ActiveMetricsJob {}
