//! Blue Archive 学生口吻的加载页提示语。

use rand::Rng;

prpr_l10n::tl_file!("blue_archive_tips");

const TIP_COUNT: usize = 109;

/// 随机返回一条本地化的加载提示语。
pub fn random_tip() -> String {
    let idx = rand::thread_rng().gen_range(0..TIP_COUNT);
    let key = format!("tip-{:03}", idx + 1);
    tl!(key).to_string()
}
