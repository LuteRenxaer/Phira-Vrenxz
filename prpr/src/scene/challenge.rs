//! 课题模式 / Re:课题 / HP 血量模式：单局配置与跨局共享状态。
//!
//! 主程序（ChallengePage）在开始前构造 [`ChallengeConfig`]，逐局传递到
//! [`super::LoadingScene`] / [`super::GameScene`]；跨局的成绩与进度存放在
//! [`ChallengeSession`]（Arc<Mutex> 共享），GameScene 结算时写入，
//! ChallengePage 据此自动推进下一局或展示结算。

use crate::ext::SafeTexture;
use std::sync::{Arc, Mutex};

/// 扣血条件
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HpLoseCondition {
    /// Bad / Miss 扣血
    BadOrMiss,
    /// 非 Perfect（Good / Bad / Miss）都扣血
    NonPerfect,
}

/// HP 配置
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HpConfig {
    pub max_hp: f32,
    pub lose_condition: HpLoseCondition,
}

/// 跨局共享的课题会话状态（GameScene 结算写入，主程序 ChallengePage 读取推进）
pub struct ChallengeSession {
    /// 是否 Re:课题
    pub re: bool,
    /// 每局曲目路径（顺序即游玩顺序）
    pub paths: Vec<String>,
    /// 每局结算 (score, accuracy)
    pub results: Vec<(u32, f32)>,
    /// 每局判定计数 [Perfect, Good, Bad, Miss]（顺序同 paths）
    pub round_counts: Vec<[u32; 4]>,
    /// 每局 Early/Late 计数（顺序同 paths；Phigros 官方结算风格补充）
    pub round_early_late: Vec<(u32, u32)>,
    /// 每局最大连击（顺序同 paths；Phigros 官方课题结算 Max Combo 字段）
    pub round_max_combo: Vec<u32>,
    /// 每局是否 Full Combo（max_combo == num_of_notes，顺序同 paths）
    pub round_fc: Vec<bool>,
    /// 已自然完成的局数
    pub completed_rounds: usize,
    /// 课题是否作废（中途退出 / HP 耗尽未完成）
    pub aborted: bool,
    /// 每局曲名与难度（结算列表展示用，顺序同 paths）
    pub round_names: Vec<(String, String)>,
    /// 每局真实曲绘纹理（随每局完成写入；课题结算封面展示用，顺序同 paths）
    pub round_illustrations: Vec<SafeTexture>,
    /// 上一局结束时的剩余 HP（开启 HP 后供下一局继承）
    pub hp_left: Option<f32>,
    /// HP 配置（开启 HP 时为 Some）
    pub hp: Option<HpConfig>,
    /// 自定义判定（毫秒），None 表示使用默认课题窗口
    pub judge_ms: Option<(u32, u32, u32)>,
}

/// 单局课题配置（引擎侧，随 LoadingScene / GameScene 传递）
#[derive(Clone)]
pub struct ChallengeConfig {
    pub re: bool,
    /// 总局数
    pub total: usize,
    /// 当前局序号（0-based）
    pub index: usize,
    /// 自定义判定（秒），None 使用课题默认窗口 Perfect 0.04 / Good 0.075 / Bad 0.14
    pub custom_limits: Option<(f64, f64, f64)>,
    /// 课题计分：无连击加成，单局满分 1_000_000
    pub scoring: bool,
    pub hp: Option<HpConfig>,
    pub session: Arc<Mutex<ChallengeSession>>,
}

impl ChallengeConfig {
    /// 课题默认判定窗口（秒）：Perfect±40ms / Good 40~75ms / Bad 75~140ms
    pub fn default_limits() -> (f64, f64, f64) {
        (0.04, 0.075, 0.14)
    }

    /// 由 Re:课题自定义毫秒输入转换为判定窗口秒
    pub fn limits_from_ms(p: u32, g: u32, b: u32) -> (f64, f64, f64) {
        (p as f64 / 1000., g as f64 / 1000., b as f64 / 1000.)
    }

    /// 课题结算颜色档位（三局总分 = 三张定数和向下取整的标）：
    /// 彩 3_000_000 / 金 2_940_000~2_999_999 / 红橙 2_850_000~
    /// 蓝 2_700_000~ / 绿 2_460_000~ / 低于 246 万无标不记录
    pub fn color_rank(total: u32) -> (u8, Option<u32>) {
        if total >= 3_000_000 {
            (0, Some(3_000_000))
        } else if total >= 2_940_000 {
            (1, None)
        } else if total >= 2_850_000 {
            (2, None)
        } else if total >= 2_700_000 {
            (3, None)
        } else if total >= 2_460_000 {
            (4, None)
        } else {
            (5, None)
        }
    }

    /// 下一局的 HP 配置：开启 HP 且已有继承值时用剩余 HP 重建；未开启返回 None
    pub fn next_hp(&self) -> Option<HpConfig> {
        let st = self.session.lock().unwrap();
        match (st.hp, st.hp_left) {
            (Some(base), Some(left)) => Some(HpConfig {
                max_hp: left.max(1.),
                lose_condition: base.lose_condition,
            }),
            (Some(base), None) => Some(base),
            (None, _) => None,
        }
    }
}

/// 单局课题结算记录（结算列表展示：曲名 / 难度 / Lv / 满分与得分 / 判定计数 / Early/Late）
#[derive(Clone)]
pub struct ChallengeRoundRecord {
    /// 曲名
    pub name: String,
    /// 难度文本（如 "IN 15"，含难度字母与 Lv 定数）
    pub difficulty: String,
    /// 本局得分
    pub score: u32,
    /// 本局准确率
    pub accuracy: f32,
    /// 本局最大连击（Phigros 官方课题结算 Max Combo 字段）
    pub max_combo: u32,
    /// 判定计数 [Perfect, Good, Bad, Miss]
    pub counts: [u32; 4],
    /// Early / Late 计数（Phigros 官方结算风格字段）
    pub early: u32,
    pub late: u32,
    /// 是否 Full Combo
    pub full_combo: bool,
}

/// PhiraPro 徽章档位：彩 3000000 / 金 2940000~ / 红 2850000~ / 蓝 2700000~ / 绿 2460000~
pub fn badge_index(total: u32) -> Option<usize> {
    match total {
        3_000_000 => Some(0),
        2_940_000..=2_999_999 => Some(1),
        2_850_000..=2_939_999 => Some(2),
        2_700_000..=2_849_999 => Some(3),
        2_460_000..=2_699_999 => Some(4),
        _ => None,
    }
}

/// PhiraPro 色档图标索引：与 Resource::load_icons 顺序（F/C/B/A/S/V/FC/phi）对应
pub fn rank_index(total: u32, full_combo: bool) -> u8 {
    match badge_index(total) {
        Some(0) => 7,
        Some(1) => {
            if full_combo {
                6
            } else {
                5
            }
        }
        Some(2) => 4,
        Some(3) => 3,
        Some(4) => 2,
        _ => 0,
    }
}

/// PhiraPro 色档名
pub fn grade(total: u32) -> String {
    match badge_index(total) {
        Some(0) => "彩色".to_string(),
        Some(1) => "金色".to_string(),
        Some(2) => "红色".to_string(),
        Some(3) => "蓝色".to_string(),
        Some(4) => "绿色".to_string(),
        _ => "未达标".to_string(),
    }
}

/// 课题结算数据（EndingScene 挑战模式展示：所选谱面记录 + 总分色档评分）
#[derive(Clone)]
pub struct ChallengeEndData {
    /// 是否 Re:课题
    pub re: bool,
    /// 每局记录（曲名 / 难度 / 7 位分 / 判定计数 / FC）
    pub rounds: Vec<ChallengeRoundRecord>,
    /// 各局得分总和
    pub total_score: u32,
    /// 色档名（彩色 / 金色 / 红色 / 蓝色 / 绿色 / 未达标）
    pub rank_name: String,
    /// 色档图标索引（PhiraPro rank_index：0~7 对应 F/C/B/A/S/V/FC/phi）
    pub rank: u8,
    /// 三局是否全部 Full Combo（用于金色档 FC 图标）
    pub all_fc: bool,
    /// 每局真实曲绘纹理（课题结算右上角封面展示用，顺序同 rounds）
    pub covers: Vec<SafeTexture>,
}