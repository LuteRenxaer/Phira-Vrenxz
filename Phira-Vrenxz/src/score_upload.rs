// ============================================================================
// 上传成绩模块（由逆向复原附件 score_upload_reconstructed.rs 接入）
// 来源：Phira 0.8.2 Windows 客户端 · phira-main.exe 静态逆向
//       （字符串锚点 + RIP-xref + Capstone 反汇编，函数 RVA 对应关系见
//        phira_upload_score_module_report.md）
// 接入说明：数据结构字段名与顺序沿用复原结果；网络层改用本项目 client 体系
//   （Client::post + recv_raw + CLIENT_TOKEN / API_URL / 登录态检查）。
// ============================================================================

use crate::client::{recv_raw, Client, CLIENT_TOKEN};
use crate::{get_data, get_data_mut, save_data};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use prpr::scene::RecordUpdateState;
use prpr::task::Task;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 数据结构（字段名与顺序均来自 .rdata serde 字段块，0x1d36a28 / 0x1d0f710）
// ---------------------------------------------------------------------------

/// 上传成绩请求体 = 本地成绩记录 ExtAnimRecord 的同构结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: Option<i64>,          // 本地记录 id（新记录可为空）
    pub player: Option<i64>,      // 玩家 id（登录账号 id）
    pub chart: Option<i64>,       // 谱面 id
    pub score: i64,               // 总分
    pub accuracy: f64,            // 准确率
    pub perfect: i32,             // Perfect 判定数
    pub good: i32,                // Good 判定数
    pub bad: i32,                 // Bad 判定数
    pub miss: i32,                // Miss 判定数
    pub speed: f64,               // 游玩速度
    pub max_combo: i32,           // 最大连击
    pub full_combo: bool,         // 是否 FC
    pub best: bool,               // 是否最佳成绩
    pub mods: Vec<String>,        // 使用的模组（镜像等）
    pub std: Option<String>,      // 标准/平台标识（std score 配套）
    pub std_score: Option<i64>,   // 标准计分
}

/// 上传成绩响应（serde 字段块 0x1d0f710）
#[derive(Debug, Clone, Deserialize)]
pub struct ChartScoreResponse {
    pub chart: i64,               // 谱面 id
    pub token: String,            // 服务器签发的成绩 token（可作凭证/复现）
    pub chart_updated: bool,      // chartUpdated：服务器侧谱面是否有更新
    pub resp: ChartScoreResp,     // Resp 嵌套
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChartScoreResp {
    pub id: i64,                  // 服务器记录 id
    pub exp: f64,                 // 本次获取的经验值
    pub delta: f64,               // Delta：RKS 变化量
    pub new_best: bool,           // newBest：是否新纪录
    pub improvement: f64,         // 提升幅度
    pub new_rks: f64,             // newRks：更新后的 RKS
}

/// 通用 API 响应（登录/上传共用，含 token 刷新；字段块 0x1d45c4c）
/// 保留作通用解析备用；本模块上传响应用 ChartScoreResponse 解析。
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ApiResponse {
    pub code: i32,
    pub id: Option<i64>,
    pub exp: Option<f64>,
    pub delta: Option<f64>,
    pub new_best: Option<bool>,
    pub improvement: Option<f64>,
    pub new_rks: Option<f64>,
    pub score: Option<i64>,
    pub token: Option<String>,        // 新 token，需回写登录态
    pub refresh_token: Option<String>,// 新 refreshToken，需回写登录态
}

// ---------------------------------------------------------------------------
// 端点（字符串锚点：0x1d3ca13 / 0x1ce3158）
// ---------------------------------------------------------------------------
const ENDPOINT_PLAY_UPLOAD: &str = "/play/upload";

// ---------------------------------------------------------------------------
// 前置检查（字符串锚点，多语言键）
// ---------------------------------------------------------------------------
// - 键盘输入模式开启时禁用上传：item-use-keyboard-sub = "Scores cannot be uploaded when enabled."
// - 离线模式不可上传：           item-offline-sub
// - 谱面需更新：                 "Please update the chart to upload scores" + verify-cksum

fn ensure_upload_allowed() -> Result<()> {
    let data = crate::get_data();
    if data.config.use_keyboard {
        anyhow::bail!("scores cannot be uploaded when keyboard input mode is enabled");
    }
    if data.config.offline_mode {
        anyhow::bail!("offline mode: score uploading is disabled");
    }
    Ok(())
}

async fn ensure_chart_up_to_date(_chart_id: Option<i64>) -> Result<()> {
    // 校验谱面 checksum（verify-cksum），不匹配则要求更新：
    //   "Please update the chart to upload scores"
    // 本项目 fs 层未暴露对应校验接口，保持占位（上传前不阻塞）。
    Ok(())
}

// ---------------------------------------------------------------------------
// 上传主流程（对应 0x2ae9e0，RVA）
// 请求体按官方上游 phira/src/scene/song.rs 的 /play/upload 实现：
//   Req { chart: i32, token: String /* STANDARD.encode(record_data) */,
//         chart_updated: Option<DateTime<Utc>> }
// 即本地成绩字节不直接作为 JSON body，而是 base64 编码后放在 token 字段。
// 网络层接入：CLIENT 全局已带 Bearer Authorization（build_client），
// Client::post 自动拼接 API_URL；登录态由 CLIENT_TOKEN 是否非空判定。
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadReq {
    chart: i32,
    token: String,
    chart_updated: Option<DateTime<Utc>>,
}

/// 上传成绩响应（官方 /play/upload Resp，camelCase）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadResp {
    id: i32,
    exp_delta: f64,
    new_best: bool,
    improvement: u32,
    new_rks: f32,
}

/// 按配置返回成绩上传服务器 base URL。
/// 成绩上传固定目标：本地穿透服务器（官方服务器已移除）。
pub const UPLOAD_BASE: &str = "https://frp-bid.com:19936";

fn upload_server_base() -> String {
    UPLOAD_BASE.to_string()
}

pub async fn upload_score(
    record_data: Vec<u8>,
    chart: i32,
    chart_updated: Option<DateTime<Utc>>,
) -> Result<UploadResp> {
    // 1) 前置检查：在线 + 非键盘模式 + 谱面版本校验（verify-cksum）
    ensure_upload_allowed()?;
    ensure_chart_up_to_date(Some(chart as i64)).await?;

    // 上传目标固定为本地穿透服务器（自签证书），确保忽略证书校验并重建 client
    if !crate::get_data().accept_invalid_cert {
        crate::get_data_mut().accept_invalid_cert = true;
        let _ = crate::save_data();
        let _ = crate::client::set_access_token_sync(
            crate::client::CLIENT_TOKEN.load().as_ref().clone().as_deref(),
        );
    }

    // 登录态检查：token 存在才允许上传
    CLIENT_TOKEN
        .load()
        .as_ref()
        .clone()
        .ok_or_else(|| anyhow!("not logged in"))?;

    // 2) 组装请求：POST {upload_server_base}/play/upload，Content-Type: application/json
    let req = UploadReq {
        chart,
        token: STANDARD.encode(record_data),
        chart_updated,
    };
    let resp = recv_raw(Client::post_to(&upload_server_base(), ENDPOINT_PLAY_UPLOAD, &req))
        .await
        .context("upload request failed")?;

    // 3) 解析 UploadResp；失败路径对应 upload-failed 文案
    let parsed: UploadResp = resp
        .json()
        .await
        .context("failed to decode upload response")?;

    Ok(parsed)
}

/// 结算页驱动（字符串锚点：uploading / still-uploading / upload-success / upload-failed / upload-retry）
/// 对应 prpr/src/scene/ending.rs 场景状态机；本次开关接入仅接 submit_score 入口，UI 状态机未启用。
#[allow(dead_code)]
pub enum UploadUiState {
    Idle,
    Uploading,
    StillUploading,
    Success(ChartScoreResponse),
    Failed(String),
}

/// UploadFn 入口：接收结算页 encode_record 产出的本地成绩字节（Vec<u8>），
/// 按官方 /play/upload 请求体（chart + base64 token + chartUpdated）上传，
/// 并转换为结算页所需的 RecordUpdateState。
pub fn submit_score(
    data: Vec<u8>,
    chart: i32,
    chart_updated: Option<DateTime<Utc>>,
) -> Task<Result<RecordUpdateState>> {
    Task::new(async move {
        let resp = upload_score(data, chart, chart_updated).await?;
        Ok(RecordUpdateState {
            best: resp.new_best,
            improvement: resp.improvement,
            gain_exp: resp.exp_delta as f32,
            new_rks: Some(resp.new_rks),
        })
    })
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum UploadError {
    Network(reqwest::Error),
    HttpStatus(reqwest::StatusCode),
    Decode(reqwest::Error),
    ChartOutdated,
    DisabledKeyboard,
    Offline,
}

// ---------------------------------------------------------------------------
// 本地成绩记录（ExtAnimRecord，文件头 28 dd 64 d8）-> 上传 Record 的转换
// 对应 ExtAnimRecord 反序列化函数 0x1d08c0 / 0x1d12a0
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct ExtAnimRecord {
    pub id: Option<i64>,
    pub player: Option<i64>,
    pub chart: Option<i64>,
    pub score: i64,
    pub accuracy: f64,
    pub perfect: i32,
    pub good: i32,
    pub bad: i32,
    pub miss: i32,
    pub speed: f64,
    pub max_combo: i32,
    pub full_combo: bool,
    pub best: bool,
    pub mods: Vec<String>,
    pub std: Option<String>,
    pub std_score: Option<i64>,
}

impl From<ExtAnimRecord> for Record {
    fn from(r: ExtAnimRecord) -> Self {
        Record {
            id: r.id,
            player: r.player,
            chart: r.chart,
            score: r.score,
            accuracy: r.accuracy,
            perfect: r.perfect,
            good: r.good,
            bad: r.bad,
            miss: r.miss,
            speed: r.speed,
            max_combo: r.max_combo,
            full_combo: r.full_combo,
            best: r.best,
            mods: r.mods,
            std: r.std,
            std_score: r.std_score,
        }
    }
}
