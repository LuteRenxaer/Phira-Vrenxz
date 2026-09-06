use super::{
    line::{run_line_note_pass, LineNoteView},
    note::{FrameGeo, NoteVertexItem, StagingSink},
    note::FADEOUT_TIME,
    BpmList, CtrlObject, Effect, JudgeLine, JudgeLineKind, Matrix, Note, NoteKind, Resource, UIElement, Vector,
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

    /// Use Arcaea-style judgement and scoring (for XC-SIM charts)
    pub arcaea_judgement: bool,
    /// Use FNF-style judgement and scoring
    pub fnf_judgement: bool,

    /// Line order according to z-index, lines with attach_ui will be removed from this list
    ///
    /// Store the index of the line in z-index ascending order
    pub order: Vec<usize>,
    /// TODO: docs from RPE
    pub attach_ui: [Option<usize>; 7],

    pub hitsounds: HitSoundMap,

    /// 并行 Note 顶点生成的复用缓冲（按 `order` 下标索引，跨帧复用容量）
    note_staging: RefCell<Vec<Vec<NoteVertexItem>>>,
}

impl Chart {
    pub fn new(offset: f32, lines: Vec<JudgeLine>, bpm_list: BpmList, settings: ChartSettings, extra: ChartExtra, hitsounds: HitSoundMap) -> Self {
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

    pub fn reset(&mut self) {
        self.lines
            .iter_mut()
            .flat_map(|it| it.notes.iter_mut())
            .for_each(|note| note.judge = JudgeStatus::NotJudged);
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

        let trs = self.lines.iter().map(|it| it.now_transform(res, &self.lines)).collect::<Vec<_>>();
        let rotations = self.lines.iter().map(|it| it.fetch_rot(&self.lines)).collect::<Vec<_>>();
        for ((line, tr), rot) in self.lines.iter_mut().zip(trs).zip(rotations) {
            line.update(res, tr, rot);
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

    /// 计算当前帧的 Note 负载指标。
    ///
    /// 返回 `(屏幕上可见的 Note 数量, 未来 1 秒内需要击打的 Note 数量)`，
    /// 供游戏场景据此启用低分辨率 Note 渲染 / 关闭打击特效。
    ///
    /// 按判定线逐条统计（每帧一次，开销为微秒级，不阻塞主线程）。
    ///
    /// 不再每帧全量遍历所有 Note（百万级谱面会浪费大量主线程时间/内存带宽），
    /// 而是只扫描每条判定线自己维护的“活跃”Note（[`JudgeLineCache::active_ids`]，
    /// 即 `update_order` = 上一帧结束时仍未消亡的 Note，与旧实现 `filter(!dead)` 的
    /// 集合一致，最多滞后一帧，对画面阈值无可感知差异）。
    ///
    /// 活跃 Note 总量极大（罕见的巨型活跃集合）时再交给全局常驻线程池
    /// [`crate::parallel::POOL`] 并行统计，主线程只在屏障处等待；其安全模型与
    /// `parallel.rs` 一致（“调用方阻塞屏障 + 裸指针”）：统计期间主线程阻塞等待
    /// 全部工作线程结束，没有任何并发写者，工作线程只读共享的 Note 数据。
    pub fn load_metrics(&self, res: &Resource) -> (usize, usize) {
        let t = res.time;
        let end = t + 1.0;
        // 屏幕可见的近似时间窗口：Note 一般提前约 2~3 秒进入屏幕，
        // 取 5 秒既不会漏掉慢速滚动，也不会把几十秒后的 Note 误算为"在屏幕上"
        let visible_end = t + 5.0;

        let workers = POOL.worker_count();
        let active_total: usize = self.lines.iter().map(|it| it.cache.active_ids().len()).sum();
        if active_total == 0 {
            return (0, 0);
        }
        // 是否并行由“性能优化档位”决定（见 Config::metrics_parallel / metrics_parallel_min）
        if workers > 1 && res.config.metrics_parallel() && active_total > res.config.metrics_parallel_min() {
            // 每条线一个统计任务（只读共享，无并发写者）
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
            // 常规谱面：活跃集合很小，串行扫描即可（微秒级）
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
        // 平行反转（X 轴镜像）与垂直反转（Y 轴镜像）作用于谱面整体显示；
        // Y 方向的基准 -1 为屏幕方向修正，垂直反转时改回 +1 即实现上下颠倒。
        let mirror_x = res.config.flip_x();
        let mirror_y = res.config.flip_y();
        res.apply_model_of(
            &Matrix::identity().append_nonuniform_scaling(&Vector::new(if mirror_x { -1. } else { 1. }, if mirror_y { 1. } else { -1. })),
            |res| {
                // 默认路径：与画面正常时期一致（判定线视觉 + 逐线 Note 串行生成）
                let mut guard = self.bpm_list.borrow_mut();
                for id in &self.order {
                    self.lines[*id].render(ui, res, &self.lines, &mut guard, &self.settings, *id);
                }
                drop(guard);
                // 一次性 draw_all（仍处于镜像模型矩阵作用域内）
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

    /// 逐线收集 Note 顶点工作项，交由全局常驻线程池按判定线并行生成，
    /// 主线程随后按线序把各线程暂存结果合批进 `note_buffer`。
    ///
    /// 试验性：画面 bug（note 拖影）修复前默认不启用（渲染走旧串行路径）。
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

        /// 一条判定线的顶点生成任务：只读共享 Note 切片 + 拷贝的线视图 + 独占 ctrl 拷贝
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

        // 并行/串行共用同一生成代码；线程数不足或只有一条线时原地串行执行
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

        // 并行路径用的是 ctrl 拷贝：把真实判定线 ctrl 恢复为与串行渲染一致的“帧末状态”
        for job in &jobs {
            if let Some(h) = job.last_h {
                self.lines[job.line_id].ctrl_obj.borrow_mut().set_height(h);
            }
        }

        // 按线序合批进共享 note_buffer（与旧实现相同的插入顺序 → 相同的合批结果）
        let mut nb = res.note_buffer.borrow_mut();
        for (k, mut job) in jobs.into_iter().enumerate() {
            let out = std::mem::take(&mut job.out);
            for item in &out {
                nb.push(item.key, item.verts);
            }
            buffers[k] = out;
        }
    }
}

/// 单条 Note 对负载统计的贡献 `(可见, 即将击打)`。判定口径与旧 `load_metrics` 完全一致：
///
/// - 已判定的非 Hold note 不再渲染（同 `Note::render`）；
/// - 超出淡出窗口的 note 不再渲染（同 `Note::render`）；
/// - 未来 1 秒内仍未判定、非假 note 计入“即将击打”。
///
/// 调用方保证传入的均为“活跃”（尚未消亡）Note，即旧实现 `filter(!it.dead())` 的集合。
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

/// 一条判定线的活跃 Note 统计任务：`notes`/`ids` 裸指针指向的数据在屏障期间被本任务只读。
struct ActiveMetricsJob {
    notes: *const Note,
    notes_len: usize,
    ids: *const u32,
    len: usize,
    visible: usize,
    upcoming: usize,
}

// 裸指针 + 调用方阻塞屏障（与 parallel.rs 同款模式）：`scoped_parallel_for` 阻塞等待
// 期间，`notes`/`ids` 指向的数据只会被本任务读取——主线程（唯一的写者）不会并发写这些数据。
unsafe impl Send for ActiveMetricsJob {}
