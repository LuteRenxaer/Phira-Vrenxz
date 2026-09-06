//! 蓝奏云直链解析与下载模块
//!
//! 支持解析蓝奏云文件夹文件列表、带密码文件的直链获取，以及文件下载。

use anyhow::{anyhow, bail, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// 蓝奏云文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanzouFile {
    /// 文件名
    pub name: String,
    /// 文件大小（人类可读格式，如 "1.2 MB"）
    pub size: String,
    /// 文件页面链接（短链接）
    pub url: String,
    /// 是否需要密码
    pub has_password: bool,
}

/// 蓝奏云文件夹解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanzouFolder {
    /// 文件夹名称
    pub name: String,
    /// 文件列表
    pub files: Vec<LanzouFile>,
}

static USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// 简单的 URL 编码函数
fn urlencode(s: &str) -> String {
    let mut result = String::new();
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(*byte as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

static FILE_LIST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<a[^>]*href="(/[^"]+)"[^>]*>\s*<div[^>]*class="[^"]*filename[^"]*"[^>]*>([^<]+)</div>"#).unwrap()
});

static FILE_SIZE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<div[^>]*class="[^"]*filesize[^"]*"[^>]*>([^<]+)</div>"#).unwrap()
});

static FOLDER_NAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<title>([^<]+)</title>"#).unwrap()
});

// 从文件夹页面提取 AJAX 参数
static FID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"fid['"]?\s*[:=]\s*['"]?(\d+)"#).unwrap()
});

static UID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"uid['"]?\s*[:=]\s*['"]([^'"]+)['"]"#).unwrap()
});

static PUID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"puid['"]?\s*[:=]\s*['"]([^'"]+)['"]"#).unwrap()
});

static T_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"iblvkt\s*=\s*['"]([^'"]+)['"]"#).unwrap()
});

static K_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"_hk7mm\s*=\s*['"]([^'"]+)['"]"#).unwrap()
});

static FILEMORE_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"filemoreajax\.php\?file=(\d+)"#).unwrap()
});

static SIGN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"var\s+ajaxdata\s*=\s*'([^']+)'"#).unwrap()
});

static PASSWORD_SIGN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"name="pwd"[^>]*value="([^"]*)""#).unwrap()
});

static DIRECT_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(https?://[a-z0-9.-]+\.lanzou[a-z]+\.com/[^"']+)"#).unwrap()
});

/// 创建带默认 headers 的 reqwest 客户端
fn create_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .context("创建 HTTP 客户端失败")
}

/// 解析蓝奏云文件夹，获取文件列表
///
/// # Arguments
/// * `folder_url` - 文件夹链接，如 "https://wwamt.lanzout.com/b01bjn9e9i/"
/// * `password` - 文件夹密码（如果有）
pub async fn list_folder(folder_url: &str, password: Option<&str>) -> Result<LanzouFolder> {
    let client = create_client()?;

    // 确保 URL 以 / 结尾
    let url = if folder_url.ends_with('/') {
        folder_url.to_string()
    } else {
        format!("{}/", folder_url)
    };

    let resp = client.get(&url).send().await.context("请求文件夹页面失败")?;
    if !resp.status().is_success() {
        bail!("请求文件夹页面失败，状态码: {}", resp.status());
    }

    let html = resp.text().await.context("读取文件夹页面内容失败")?;

    // 解析文件夹名称
    let name = FOLDER_NAME_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "未知文件夹".to_string());

    // 提取 AJAX 参数
    let fid = FILEMORE_URL_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            FID_RE.captures(&html)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        })
        .context("无法提取文件夹 ID")?;

    let uid = UID_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "5155034".to_string());

    let puid = PUID_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    let t_val = T_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    let k_val = K_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    // 构建基础 URL（只取协议+域名，不包含文件夹路径）
    // e.g. https://wwamt.lanzout.com/b01bjn9e9i/ -> https://wwamt.lanzout.com
    let base_url = {
        let u = url.trim_end_matches('/');
        if let Some(pos) = u.find("://") {
            let after_proto = &u[pos + 3..];
            if let Some(slash_pos) = after_proto.find('/') {
                format!("{}://{}", &u[..pos], &after_proto[..slash_pos])
            } else {
                u.to_string()
            }
        } else {
            "https://wwamt.lanzout.com".to_string()
        }
    };

    // POST 请求获取文件列表
    let ajax_url = format!("{}/filemoreajax.php?file={}", base_url, fid);
    let pwd_str = password.unwrap_or("");

    // 手动构造 URL 编码的请求体（reqwest 无 form feature）
    let body = format!(
        "lx=2&fid={}&uid={}&puid={}&pg=1&rep=0&t={}&k={}&up=1&ls=1&pwd={}",
        urlencode(&fid),
        urlencode(&uid),
        urlencode(&puid),
        urlencode(&t_val),
        urlencode(&k_val),
        urlencode(pwd_str)
    );

    let ajax_resp = client
        .post(&ajax_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Referer", &url)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("Origin", &base_url)
        .body(body)
        .send()
        .await
        .context("请求文件列表 AJAX 失败")?;

    if !ajax_resp.status().is_success() {
        let status = ajax_resp.status();
        let text = ajax_resp.text().await.unwrap_or_default();
        bail!("请求文件列表失败，状态码: {}, 响应: {}", status, text);
    }

    let ajax_text = ajax_resp.text().await.context("读取文件列表响应失败")?;

    // 解析 JSON 响应
    let json: serde_json::Value = match serde_json::from_str(&ajax_text) {
        Ok(v) => v,
        Err(e) => {
            bail!("解析文件列表 JSON 失败: {}, 响应内容: {}", e, &ajax_text[..ajax_text.len().min(500)]);
        }
    };

    let zt = json.get("zt").and_then(|v| v.as_str()).unwrap_or("0");
    if zt != "1" {
        let info = json.get("info").and_then(|v| v.as_str()).unwrap_or("未知错误");
        bail!("获取文件列表失败: {} (zt={})", info, zt);
    }

    let text_arr = json.get("text").and_then(|v| v.as_array()).context("响应中无文件列表")?;

    let mut files = Vec::new();
    for item in text_arr {
        let file_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let file_name = item.get("name_all").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let file_size = item.get("size").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if file_name.is_empty() || file_id.is_empty() || file_id == "-1" {
            continue;
        }

        // 去除文件名中的 HTML 标签
        let file_name = strip_html_tags(&file_name);

        let file_url = if file_id.starts_with("http") {
            file_id.clone()
        } else {
            format!("{}/{}", base_url, file_id)
        };

        files.push(LanzouFile {
            name: file_name,
            size: file_size,
            url: file_url,
            has_password: false,
        });
    }

    Ok(LanzouFolder { name, files })
}

/// 去除字符串中的 HTML 标签
fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result.trim().to_string()
}

/// 获取文件的直链下载地址
///
/// # Arguments
/// * `file_url` - 文件页面链接
/// * `password` - 文件密码（如果有）
pub async fn get_direct_url(file_url: &str, password: Option<&str>) -> Result<String> {
    let client = create_client()?;

    // 第一步：访问文件页面
    let resp = client.get(file_url).send().await.context("请求文件页面失败")?;
    if !resp.status().is_success() {
        bail!("请求文件页面失败，状态码: {}", resp.status());
    }
    let html = resp.text().await.context("读取文件页面内容失败")?;

    // 检查是否需要密码
    let needs_password = html.contains("pwd") || html.contains("password") || html.contains("输入密码");

    let final_html = if needs_password {
        if let Some(pwd) = password {
            // 提取表单提交所需的 sign 参数
            let sign = SIGN_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());

            // 提交密码（手动构造 URL 编码请求体，因为 reqwest 未启用 form feature）
            let mut body = format!("pwd={}", urlencode(pwd));
            if let Some(s) = &sign {
                body.push_str(&format!("&sign={}", urlencode(s)));
            }

            let submit_url = if file_url.contains("/tp/") {
                file_url.to_string()
            } else {
                // 构造 ajax 提交 URL
                format!("{}?ct=file&ac=ajax", file_url)
            };

            let resp = client
                .post(&submit_url)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await
                .context("提交文件密码失败")?;

            let result = resp.text().await.context("读取密码提交结果失败")?;

            // 检查提交结果
            if result.contains("密码错误") || result.contains("pwd_error") {
                bail!("文件密码错误");
            }

            // 提交成功后重新访问文件页面
            let resp = client.get(file_url).send().await.context("重新请求文件页面失败")?;
            resp.text().await.context("读取文件页面内容失败")?
        } else {
            bail!("该文件需要密码，请提供密码");
        }
    } else {
        html
    };

    // 第二步：从页面中提取直链
    // 蓝奏云的直链通常在 iframe 的 src 中，或者在 JavaScript 的变量中
    let direct_url = extract_direct_url(&final_html)
        .ok_or_else(|| anyhow!("无法从页面中提取直链下载地址"))?;

    Ok(direct_url)
}

/// 从 HTML 中提取直链下载地址
fn extract_direct_url(html: &str) -> Option<String> {
    // 方式1：匹配 iframe src
    let iframe_re = Regex::new(r#"<iframe[^>]*src="([^"]+)"[^>]*>"#).unwrap();
    if let Some(caps) = iframe_re.captures(html) {
        if let Some(url) = caps.get(1) {
            let url = url.as_str();
            if url.contains("lanzou") && !url.contains("about:blank") {
                return Some(url.to_string());
            }
        }
    }

    // 方式2：匹配 JavaScript 中的直链变量
    let var_re = Regex::new(r#"(?:var|let|const)\s+\w*(?:url|download|link)\w*\s*=\s*["']([^"']+)["']"#).unwrap();
    for caps in var_re.captures_iter(html) {
        if let Some(url) = caps.get(1) {
            let url = url.as_str();
            if url.contains("lanzou") && url.contains("http") {
                return Some(url.to_string());
            }
        }
    }

    // 方式3：通用匹配 lanzou 域名的 URL
    for caps in DIRECT_URL_RE.captures_iter(html) {
        if let Some(url) = caps.get(1) {
            let url = url.as_str();
            // 排除页面自身 URL 和非下载链接
            if !url.contains("/b0") && !url.contains("/tp/") && url.len() > 30 {
                return Some(url.to_string());
            }
        }
    }

    None
}

/// 下载文件到指定路径
///
/// # Arguments
/// * `direct_url` - 直链下载地址
/// * `save_path` - 保存路径
/// * `on_progress` - 进度回调 (已下载字节, 总字节)
pub async fn download_file<F>(direct_url: &str, save_path: &Path, mut on_progress: F) -> Result<()>
where
    F: FnMut(u64, Option<u64>),
{
    let client = create_client()?;
    let resp = client.get(direct_url).send().await.context("请求下载失败")?;
    if !resp.status().is_success() {
        bail!("下载失败，状态码: {}", resp.status());
    }

    let total_size = resp.content_length();
    let mut downloaded: u64 = 0;

    // 确保目标目录存在
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent).context("创建下载目录失败")?;
    }

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(save_path).await.context("创建文件失败")?;

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取下载数据失败")?;
        file.write_all(&chunk).await.context("写入文件失败")?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total_size);
    }

    file.flush().await.context("刷新文件失败")?;
    Ok(())
}

/// 便捷函数：从蓝奏云文件夹下载指定名称的文件
///
/// # Arguments
/// * `folder_url` - 文件夹链接
/// * `folder_password` - 文件夹密码
/// * `file_name` - 要下载的文件名（支持模糊匹配）
/// * `file_password` - 文件密码
/// * `save_path` - 保存路径
/// * `on_progress` - 进度回调
pub async fn download_file_by_name<F>(
    folder_url: &str,
    folder_password: Option<&str>,
    file_name: &str,
    file_password: Option<&str>,
    save_path: &Path,
    on_progress: F,
) -> Result<()>
where
    F: FnMut(u64, Option<u64>),
{
    // 列出文件夹文件
    let folder = list_folder(folder_url, folder_password).await?;

    // 查找匹配的文件
    let target = folder
        .files
        .iter()
        .find(|f| f.name.contains(file_name) || file_name.contains(&f.name))
        .ok_or_else(|| anyhow!("未找到文件: {}", file_name))?;

    // 获取直链
    let direct_url = get_direct_url(&target.url, file_password).await?;

    // 下载
    download_file(&direct_url, save_path, on_progress).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_folder() {
        let result = list_folder("https://wwamt.lanzout.com/b01bjn9e9i/", Some("hlib")).await;
        match result {
            Ok(folder) => {
                println!("文件夹: {}", folder.name);
                for f in &folder.files {
                    println!("  - {} ({})", f.name, f.url);
                }
            }
            Err(e) => println!("错误: {}", e),
        }
    }
}
