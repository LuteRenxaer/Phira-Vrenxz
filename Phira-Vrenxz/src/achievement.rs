//! 成就系统
//!
//! 提供成就定义、进度追踪、解锁检测和持久化功能。
//! 成就进度保存在 data/achievements.json 中。
//!
//! 游戏内的接入点：
//! - 游玩结算：`scene::song::SongScene::on_result` 中调用 [`record_play`]
//! - 收藏变化：`Data::set_collection_info` / `remove_collection` 中调用 [`sync_favorites`]
//! - 解锁反馈：新解锁的成就会通过 [`notify_unlocks`] 弹出消息并播放音效

prpr_l10n::tl_file!("achievement");

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// ========== 成就分类 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AchievementCategory {
    /// 游玩次数
    Play,
    /// 分数达成
    Score,
    /// 难度挑战
    Difficulty,
    /// 收藏收集
    Collection,
}

impl AchievementCategory {
    pub fn label_key(&self) -> &'static str {
        match self {
            AchievementCategory::Play => "category-play",
            AchievementCategory::Score => "category-score",
            AchievementCategory::Difficulty => "category-difficulty",
            AchievementCategory::Collection => "category-collection",
        }
    }

    pub fn all() -> [AchievementCategory; 4] {
        [
            AchievementCategory::Play,
            AchievementCategory::Score,
            AchievementCategory::Difficulty,
            AchievementCategory::Collection,
        ]
    }
}

// ========== 成就稀有度 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AchievementRarity {
    /// 普通
    Common,
    /// 稀有
    Rare,
    /// 史诗
    Epic,
    /// 传说
    Legendary,
}

impl AchievementRarity {
    pub fn label_key(&self) -> &'static str {
        match self {
            AchievementRarity::Common => "rarity-common",
            AchievementRarity::Rare => "rarity-rare",
            AchievementRarity::Epic => "rarity-epic",
            AchievementRarity::Legendary => "rarity-legendary",
        }
    }

    /// 稀有度对应的主题色 (r, g, b)
    pub fn color(&self) -> (f32, f32, f32) {
        match self {
            AchievementRarity::Common => (0.6, 0.6, 0.6),
            AchievementRarity::Rare => (0.2, 0.5, 0.9),
            AchievementRarity::Epic => (0.6, 0.3, 0.8),
            AchievementRarity::Legendary => (1.0, 0.75, 0.2),
        }
    }

    /// 稀有度权重（用于排序和音效选择）
    pub fn rank(&self) -> u8 {
        match self {
            AchievementRarity::Common => 0,
            AchievementRarity::Rare => 1,
            AchievementRarity::Epic => 2,
            AchievementRarity::Legendary => 3,
        }
    }
}

// ========== 成就定义 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementDef {
    /// 唯一标识
    pub id: &'static str,
    /// 显示顺序（1-20，对应 achievements_icon/{order}.png）
    pub order: u8,
    /// 显示名称
    pub name: &'static str,
    /// 描述
    pub description: &'static str,
    /// 分类
    pub category: AchievementCategory,
    /// 稀有度
    pub rarity: AchievementRarity,
    /// 目标值（进度达到此值即解锁）
    pub target: u64,
    /// 图标 emoji 或符号（自定义图标加载失败时的回退）
    pub icon: &'static str,
}

// ========== 成就进度 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementProgress {
    /// 当前进度值
    pub current: u64,
    /// 是否已解锁
    pub unlocked: bool,
    /// 解锁时间戳（秒）
    pub unlocked_at: Option<i64>,
}

impl Default for AchievementProgress {
    fn default() -> Self {
        Self {
            current: 0,
            unlocked: false,
            unlocked_at: None,
        }
    }
}

// ========== 统计数据 ==========

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AchievementStats {
    /// 累计完成谱面数
    pub total_plays: u64,
    /// 达到过的最高分数
    pub best_score: u64,
    /// 达到过的最高准确率（0.0 ~ 1.0）
    pub best_accuracy: f32,
    /// 累计全连次数
    pub full_combos: u64,
    /// 完成的HD难度谱面数
    pub hd_clears: u64,
    /// 完成的IN难度谱面数
    pub in_clears: u64,
    /// 完成的AT难度谱面数
    pub at_clears: u64,
    /// 收藏谱面数
    pub favorites: u64,
}

// ========== 持久化数据 ==========

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AchievementData {
    stats: AchievementStats,
    progress: HashMap<String, AchievementProgress>,
}

// ========== 难度分类（含反作弊校验） ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DifficultyClass {
    Hd,
    In,
    At,
}

impl DifficultyClass {
    /// 从谱面的等级字符串（如 "AT Lv.17"、"IN Lv.15"）与数值难度中推断难度分类。
    ///
    /// 反作弊校验：难度可以被玩家篡改（如把低难度谱面的 info.yml 数值改成 16.9、
    /// 或把标签改成 "AT"），因此要求**标签与数值一致**：
    /// - 标准标签（EZ/HD/IN/AT）的数值必须落在该标签的合法区间内，否则视为被篡改，不计数；
    /// - 无标准标签时按数值分档，超出合法区间的数值（如 91、114514）不计数。
    ///
    /// 合法区间：EZ 1-7、HD 3-12、IN 6-15、AT 13-16.9。
    fn classify(level: &str, difficulty: f32) -> Option<DifficultyClass> {
        let prefix = level.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
        match prefix.as_str() {
            // 标准标签：校验数值是否落在该标签的合法区间
            "EZ" | "EASY" => {
                // EZ 难度不参与成就统计；若数值异常同样不计数
                None
            }
            "HD" | "HARD" => {
                if (3. ..=12.).contains(&difficulty) {
                    Some(DifficultyClass::Hd)
                } else {
                    None
                }
            }
            "IN" => {
                if (6. ..=15.).contains(&difficulty) {
                    Some(DifficultyClass::In)
                } else {
                    None
                }
            }
            "AT" => {
                if (13. ..=16.9).contains(&difficulty) {
                    Some(DifficultyClass::At)
                } else {
                    None
                }
            }
            // 无标准标签：按数值分档
            _ => {
                if (1. ..=7.).contains(&difficulty) {
                    None
                } else if (3. ..=12.).contains(&difficulty) {
                    Some(DifficultyClass::Hd)
                } else if (6. ..=15.).contains(&difficulty) {
                    Some(DifficultyClass::In)
                } else if (13. ..=16.9).contains(&difficulty) {
                    Some(DifficultyClass::At)
                } else {
                    None
                }
            }
        }
    }
}

// ========== 成就管理器 ==========

pub struct AchievementManager {
    data_path: PathBuf,
    data: AchievementData,
}

impl AchievementManager {
    /// 创建成就管理器，从指定数据目录加载/保存进度
    pub fn new(data_dir: &std::path::Path) -> Result<Self> {
        let data_path = data_dir.join("achievements.json");
        let data = if data_path.exists() {
            let content = std::fs::read_to_string(&data_path)
                .context(tl!("achievement-read-fail"))?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AchievementData::default()
        };
        let mut mgr = Self { data_path, data };
        // 加载后立即按当前统计数据判定一次，
        // 避免"条件已满足但未解锁"的成就（如版本更新新增的成就）一直保持锁定
        let newly = mgr.check_all_unlocks();
        if !newly.is_empty() {
            let _ = mgr.save();
        }
        Ok(mgr)
    }

    /// 保存进度到磁盘
    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(&self.data_path, content)?;
        Ok(())
    }

    /// 获取所有成就定义
    pub fn definitions() -> Vec<&'static AchievementDef> {
        ALL_ACHIEVEMENTS.to_vec()
    }

    /// 获取指定分类的成就定义
    pub fn definitions_by_category(category: AchievementCategory) -> Vec<&'static AchievementDef> {
        ALL_ACHIEVEMENTS
            .iter()
            .filter(|a| a.category == category)
            .copied()
            .collect()
    }

    /// 按 id 查找成就定义
    pub fn definition(id: &str) -> Option<&'static AchievementDef> {
        ALL_ACHIEVEMENTS.iter().find(|a| a.id == id).copied()
    }

    /// 获取成就进度
    pub fn progress(&self, id: &str) -> AchievementProgress {
        self.data
            .progress
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取统计数据
    pub fn stats(&self) -> &AchievementStats {
        &self.data.stats
    }

    /// 已解锁成就数量
    pub fn unlocked_count(&self) -> usize {
        ALL_ACHIEVEMENTS
            .iter()
            .filter(|a| self.progress(a.id).unlocked)
            .count()
    }

    /// 总成就数
    pub fn total_count() -> usize {
        ALL_ACHIEVEMENTS.len()
    }

    /// 更新统计数据并检查成就解锁
    /// 返回新解锁的成就ID列表
    ///
    /// 无论是否解锁新成就都会持久化，确保游玩/收藏等统计不会丢失。
    pub fn update_stats<F>(&mut self, update: F) -> Result<Vec<String>>
    where
        F: FnOnce(&mut AchievementStats),
    {
        update(&mut self.data.stats);
        let newly_unlocked = self.check_all_unlocks();
        self.save()?;
        Ok(newly_unlocked)
    }

    /// 直接设置某个成就的进度值
    pub fn set_progress(&mut self, id: &str, current: u64) -> Result<bool> {
        let def = match ALL_ACHIEVEMENTS.iter().find(|a| a.id == id) {
            Some(d) => d,
            None => return Ok(false),
        };
        let prog = self
            .data
            .progress
            .entry(id.to_string())
            .or_insert_with(AchievementProgress::default);
        prog.current = current.max(prog.current);
        let was_unlocked = prog.unlocked;
        if !prog.unlocked && prog.current >= def.target {
            prog.unlocked = true;
            prog.unlocked_at = Some(chrono::Utc::now().timestamp());
        }
        let newly = prog.unlocked && !was_unlocked;
        if newly {
            self.save()?;
        }
        Ok(newly)
    }

    /// 检查所有成就是否满足解锁条件
    fn check_all_unlocks(&mut self) -> Vec<String> {
        let mut newly = Vec::new();
        let stats = self.data.stats.clone();
        for def in ALL_ACHIEVEMENTS.iter() {
            let current = Self::evaluate_progress(def, &stats);
            let prog = self
                .data
                .progress
                .entry(def.id.to_string())
                .or_insert_with(AchievementProgress::default);
            prog.current = current.max(prog.current);
            if !prog.unlocked && prog.current >= def.target {
                prog.unlocked = true;
                prog.unlocked_at = Some(chrono::Utc::now().timestamp());
                newly.push(def.id.to_string());
            }
        }
        newly
    }

    /// 根据统计数据计算成就进度
    fn evaluate_progress(def: &AchievementDef, stats: &AchievementStats) -> u64 {
        match def.id {
            // 游玩类
            "first_play" => stats.total_plays.min(1),
            "play_10" => stats.total_plays.min(10),
            "play_50" => stats.total_plays.min(50),
            "play_100" => stats.total_plays.min(100),
            // 分数类
            "score_a" => if stats.best_score >= 800000 { 1 } else { 0 },
            "score_s" => if stats.best_score >= 960000 { 1 } else { 0 },
            "score_phi" => if stats.best_score >= 1000000 { 1 } else { 0 },
            "acc_99" => if stats.best_accuracy >= 0.99 { 1 } else { 0 },
            "fc_1" => stats.full_combos.min(1),
            "fc_10" => stats.full_combos.min(10),
            // 难度类
            "clear_hd" => stats.hd_clears.min(1),
            "clear_in" => stats.in_clears.min(1),
            "clear_at" => stats.at_clears.min(1),
            "clear_hd_10" => stats.hd_clears.min(10),
            "clear_in_10" => stats.in_clears.min(10),
            "clear_in_25" => stats.in_clears.min(25),
            "clear_at_5" => stats.at_clears.min(5),
            // 收藏类
            "fav_1" => stats.favorites.min(1),
            "fav_10" => stats.favorites.min(10),
            "fav_50" => stats.favorites.min(50),
            _ => 0,
        }
    }

    /// 记录一次谱面完成
    ///
    /// - `score`: 本次游玩分数
    /// - `level`: 谱面等级字符串（如 "AT Lv.17"），用于判断难度分类
    /// - `difficulty`: 谱面数值难度
    /// - `accuracy`: 本次游玩准确率（0.0 ~ 1.0）
    /// - `full_combo`: 是否全连
    /// - `authority`: 服务端权威元数据 `(数值难度, 等级字符串)`（联网谱面）。
    ///   反作弊：本地 info.yml 可能被篡改，联网谱面优先用服务端权威值判定难度，
    ///   本地自定义谱面传 `None` 时退化为本地值 + 区间校验。
    ///
    /// 返回新解锁的成就ID列表
    pub fn record_play(
        &mut self,
        score: u64,
        level: &str,
        difficulty: f32,
        accuracy: f32,
        full_combo: bool,
        authority: Option<(f32, String)>,
    ) -> Result<Vec<String>> {
        self.update_stats(|s| {
            s.total_plays += 1;
            s.best_score = s.best_score.max(score);
            s.best_accuracy = s.best_accuracy.max(accuracy);
            if full_combo {
                s.full_combos += 1;
            }
            // 反作弊：优先使用服务端权威元数据判定难度，本地被篡改的 info.yml 不生效
            let (auth_level, auth_difficulty) = match &authority {
                Some((d, l)) => (l.as_str(), *d),
                None => (level, difficulty),
            };
            match DifficultyClass::classify(auth_level, auth_difficulty) {
                Some(DifficultyClass::Hd) => s.hd_clears += 1,
                Some(DifficultyClass::In) => s.in_clears += 1,
                Some(DifficultyClass::At) => s.at_clears += 1,
                None => {}
            }
        })
    }

    /// 更新收藏数
    pub fn set_favorites(&mut self, count: u64) -> Result<Vec<String>> {
        self.update_stats(|s| {
            s.favorites = count;
        })
    }
}

// ========== 全局单例 ==========

static mut MANAGER: Option<Arc<Mutex<AchievementManager>>> = None;

pub fn init_manager(data_dir: &std::path::Path) -> Result<()> {
    let mgr = AchievementManager::new(data_dir)?;
    unsafe {
        MANAGER = Some(Arc::new(Mutex::new(mgr)));
    }
    Ok(())
}

pub fn manager() -> Arc<Mutex<AchievementManager>> {
    unsafe {
        MANAGER
            .as_ref()
            .expect("AchievementManager 未初始化")
            .clone()
    }
}

pub fn manager_opt() -> Option<Arc<Mutex<AchievementManager>>> {
    unsafe { MANAGER.as_ref().cloned() }
}

// ========== 便捷封装 ==========

/// 成就自定义图标（achievements_icon/1..20.png），按下标 order-1 存放，由页面初始化时注入
static ACHIEVEMENT_ICONS: Mutex<Vec<Option<prpr::ext::SafeTexture>>> = Mutex::new(Vec::new());

/// 多成就同时解锁时的汇总图标（achievements_icon/Many_achievements.png），由页面初始化时注入
static MANY_ICON: Mutex<Option<prpr::ext::SafeTexture>> = Mutex::new(None);

/// 注入某个顺序位的成就图标纹理（order 1-20 对应 achievements_icon/{order}.png）
pub fn set_achievement_icon(order: u8, tex: prpr::ext::SafeTexture) {
    if let Ok(mut guard) = ACHIEVEMENT_ICONS.lock() {
        let idx = order.saturating_sub(1) as usize;
        if guard.len() <= idx {
            guard.resize(idx + 1, None);
        }
        guard[idx] = Some(tex);
    }
}

/// 获取成就的自定义图标纹理（未加载时返回 None，调用方回退 emoji）
pub fn achievement_icon(def: &AchievementDef) -> Option<prpr::ext::SafeTexture> {
    ACHIEVEMENT_ICONS
        .lock()
        .ok()
        .and_then(|guard| {
            let idx = def.order.saturating_sub(1) as usize;
            guard.get(idx).and_then(|it| it.clone())
        })
}

/// 注入多成就汇总图标纹理（在 SharedState 初始化时调用）
pub fn set_many_icon(tex: prpr::ext::SafeTexture) {
    if let Ok(mut guard) = MANY_ICON.lock() {
        *guard = Some(tex);
    }
}

fn many_icon() -> Option<prpr::ext::SafeTexture> {
    MANY_ICON.lock().ok().and_then(|guard| guard.clone())
}

/// 记录一次谱面完成（全局便捷入口）。
/// 成功解锁新成就时会自动弹出提示。
///
/// `authority` 为服务端权威元数据 `(数值难度, 等级字符串)`（联网谱面），
/// 用于反作弊：联网谱面以服务端值为准判定难度，本地自定义谱面传 `None`。
pub fn record_play(
    score: u64,
    level: &str,
    difficulty: f32,
    accuracy: f32,
    full_combo: bool,
    authority: Option<(f32, String)>,
) -> Result<()> {
    let mgr_arc = manager();
    let mut mgr = mgr_arc.lock().unwrap();
    let newly = mgr.record_play(score, level, difficulty, accuracy, full_combo, authority)?;
    notify_unlocks(&newly);
    Ok(())
}

/// 统计当前收藏谱面数（所有收藏夹中去重后的谱面数）。
fn favorite_count() -> u64 {
    // 数据尚未初始化（如 Data::init 阶段）时跳过，避免 panic
    // SAFETY: 仅读取静态的 None 状态；成就同步路径与 set_data 单线程串行执行
    if unsafe { (*std::ptr::addr_of!(crate::DATA)).is_none() } {
        return 0;
    }
    let data = crate::get_data();
    let mut seen = HashSet::new();
    for col in data.collections() {
        for chart in col.charts.iter() {
            seen.insert(chart.path.clone());
        }
    }
    seen.len() as u64
}

/// 同步收藏成就进度（全局便捷入口）。
/// 收藏数量变化后调用；新解锁的成就会自动弹出提示。
pub fn sync_favorites() {
    let Some(mgr) = manager_opt() else { return };
    let mut mgr = mgr.lock().unwrap();
    let count = favorite_count();
    if let Ok(newly) = mgr.set_favorites(count) {
        notify_unlocks(&newly);
    }
}

/// 为新解锁的成就弹出横幅提示并播放音效。
/// 单个成就弹单个横幅；同时解锁多个成就时合并为一个汇总横幅，
/// 由最高稀有度决定音效（史诗/传说播放挑战音效，其余播放普通音效）。
pub fn notify_unlocks(newly: &[String]) {
    if newly.is_empty() {
        return;
    }
    // 收集本次解锁的成就定义
    let mut defs: Vec<&'static AchievementDef> = Vec::new();
    for id in newly {
        if let Some(def) = AchievementManager::definition(id) {
            defs.push(def);
        }
    }
    if defs.is_empty() {
        return;
    }
    // 最高稀有度决定音效与主题色
    let mut best = defs[0];
    for def in &defs {
        if def.rarity.rank() > best.rarity.rank() {
            best = def;
        }
    }
    match best.rarity {
        AchievementRarity::Epic | AchievementRarity::Legendary => prpr::ui::achievement_challenge_sound(),
        _ => prpr::ui::achievement_sound(),
    }
    if defs.len() == 1 {
        let def = defs[0];
        // 使用成就自定义图标（achievements_icon/{order}.png），未加载则回退 emoji
        prpr::scene::show_achievement_popup(def.icon, achievement_icon(def), tl!(def.name).into_owned(), tl!(def.description).into_owned(), def.rarity.color());
    } else {
        // 多个成就同时解锁：合并为一个汇总横幅
        let names: Vec<String> = defs.iter().map(|d| tl!(d.name).into_owned()).collect();
        let joined = if names.len() <= 6 {
            names.join("、")
        } else {
            format!("{}、…", names[..6].join("、"))
        };
        let args = prpr_l10n::fluent_args!["count" => defs.len()];
        let title = tl!("achievement-new-multiple", &args).into_owned();
        prpr::scene::show_achievement_popup(
            "🎉",
            many_icon(),
            title,
            joined,
            best.rarity.color(),
        );
    }
}

/// 重新检测所有成就（例如打开成就页面时调用）。
/// 对于已满足条件但尚未解锁的成就会立即判定并弹出提示。
pub fn recheck_unlocks() {
    let Some(mgr) = manager_opt() else { return };
    let mgr_arc = mgr;
    let mut mgr = mgr_arc.lock().unwrap();
    let newly = mgr.check_all_unlocks();
    if !newly.is_empty() {
        let _ = mgr.save();
    }
    drop(mgr);
    notify_unlocks(&newly);
}

// ========== 成就定义列表 ==========

pub const ALL_ACHIEVEMENTS: &[&AchievementDef] = &[
    // ===== 游玩类 =====
    &AchievementDef {
        id: "first_play",
        order: 16,
        name: "ach-first_play-name",
        description: "ach-first_play-desc",
        category: AchievementCategory::Play,
        rarity: AchievementRarity::Common,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "play_10",
        order: 17,
        name: "ach-play_10-name",
        description: "ach-play_10-desc",
        category: AchievementCategory::Play,
        rarity: AchievementRarity::Common,
        target: 10,
        icon: " ",
    },
    &AchievementDef {
        id: "play_50",
        order: 9,
        name: "ach-play_50-name",
        description: "ach-play_50-desc",
        category: AchievementCategory::Play,
        rarity: AchievementRarity::Rare,
        target: 50,
        icon: " ",
    },
    &AchievementDef {
        id: "play_100",
        order: 4,
        name: "ach-play_100-name",
        description: "ach-play_100-desc",
        category: AchievementCategory::Play,
        rarity: AchievementRarity::Epic,
        target: 100,
        icon: " ",
    },
    // ===== 分数类 =====
    &AchievementDef {
        id: "score_a",
        order: 18,
        name: "ach-score_a-name",
        description: "ach-score_a-desc",
        category: AchievementCategory::Score,
        rarity: AchievementRarity::Common,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "score_s",
        order: 10,
        name: "ach-score_s-name",
        description: "ach-score_s-desc",
        category: AchievementCategory::Score,
        rarity: AchievementRarity::Rare,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "score_phi",
        order: 1,
        name: "ach-score_phi-name",
        description: "ach-score_phi-desc",
        category: AchievementCategory::Score,
        rarity: AchievementRarity::Legendary,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "acc_99",
        order: 11,
        name: "ach-acc_99-name",
        description: "ach-acc_99-desc",
        category: AchievementCategory::Score,
        rarity: AchievementRarity::Rare,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "fc_1",
        order: 12,
        name: "ach-fc_1-name",
        description: "ach-fc_1-desc",
        category: AchievementCategory::Score,
        rarity: AchievementRarity::Rare,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "fc_10",
        order: 5,
        name: "ach-fc_10-name",
        description: "ach-fc_10-desc",
        category: AchievementCategory::Score,
        rarity: AchievementRarity::Epic,
        target: 10,
        icon: " ",
    },
    // ===== 难度类 =====
    &AchievementDef {
        id: "clear_hd",
        order: 19,
        name: "ach-clear_hd-name",
        description: "ach-clear_hd-desc",
        category: AchievementCategory::Difficulty,
        rarity: AchievementRarity::Common,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "clear_in",
        order: 13,
        name: "ach-clear_in-name",
        description: "ach-clear_in-desc",
        category: AchievementCategory::Difficulty,
        rarity: AchievementRarity::Rare,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "clear_at",
        order: 6,
        name: "ach-clear_at-name",
        description: "ach-clear_at-desc",
        category: AchievementCategory::Difficulty,
        rarity: AchievementRarity::Epic,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "clear_hd_10",
        order: 14,
        name: "ach-clear_hd_10-name",
        description: "ach-clear_hd_10-desc",
        category: AchievementCategory::Difficulty,
        rarity: AchievementRarity::Rare,
        target: 10,
        icon: " ",
    },
    &AchievementDef {
        id: "clear_in_10",
        order: 7,
        name: "ach-clear_in_10-name",
        description: "ach-clear_in_10-desc",
        category: AchievementCategory::Difficulty,
        rarity: AchievementRarity::Epic,
        target: 10,
        icon: " ",
    },
    &AchievementDef {
        id: "clear_in_25",
        order: 8,
        name: "ach-clear_in_25-name",
        description: "ach-clear_in_25-desc",
        category: AchievementCategory::Difficulty,
        rarity: AchievementRarity::Epic,
        target: 25,
        icon: " ",
    },
    &AchievementDef {
        id: "clear_at_5",
        order: 2,
        name: "ach-clear_at_5-name",
        description: "ach-clear_at_5-desc",
        category: AchievementCategory::Difficulty,
        rarity: AchievementRarity::Legendary,
        target: 5,
        icon: " ",
    },
    // ===== 收藏类 =====
    &AchievementDef {
        id: "fav_1",
        order: 20,
        name: "ach-fav_1-name",
        description: "ach-fav_1-desc",
        category: AchievementCategory::Collection,
        rarity: AchievementRarity::Common,
        target: 1,
        icon: " ",
    },
    &AchievementDef {
        id: "fav_10",
        order: 15,
        name: "ach-fav_10-name",
        description: "ach-fav_10-desc",
        category: AchievementCategory::Collection,
        rarity: AchievementRarity::Rare,
        target: 10,
        icon: " ",
    },
    &AchievementDef {
        id: "fav_50",
        order: 3,
        name: "ach-fav_50-name",
        description: "ach-fav_50-desc",
        category: AchievementCategory::Collection,
        rarity: AchievementRarity::Legendary,
        target: 50,
        icon: " ",
    },
];
