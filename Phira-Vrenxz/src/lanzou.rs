
use anyhow::{anyhow, bail, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanzouFile {

    pub name: String,

    pub size: String,

    pub url: String,

    pub has_password: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanzouFolder {

    pub name: String,

    pub files: Vec<LanzouFile>,
}

static USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

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

fn create_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .context("创建 HTTP 客户端失败")
}

pub async fn list_folder(folder_url: &str, password: Option<&str>) -> Result<LanzouFolder> {
    let client = create_client()?;

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

    let name = FOLDER_NAME_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "未知文件夹".to_string());

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

    let ajax_url = format!("{}/filemoreajax.php?file={}", base_url, fid);
    let pwd_str = password.unwrap_or("");

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

pub async fn get_direct_url(file_url: &str, password: Option<&str>) -> Result<String> {
    let client = create_client()?;

    let resp = client.get(file_url).send().await.context("请求文件页面失败")?;
    if !resp.status().is_success() {
        bail!("请求文件页面失败，状态码: {}", resp.status());
    }
    let html = resp.text().await.context("读取文件页面内容失败")?;

    let needs_password = html.contains("pwd") || html.contains("password") || html.contains("输入密码");

    let final_html = if needs_password {
        if let Some(pwd) = password {

            let sign = SIGN_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());

            let mut body = format!("pwd={}", urlencode(pwd));
            if let Some(s) = &sign {
                body.push_str(&format!("&sign={}", urlencode(s)));
            }

            let submit_url = if file_url.contains("/tp/") {
                file_url.to_string()
            } else {

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

            if result.contains("密码错误") || result.contains("pwd_error") {
                bail!("文件密码错误");
            }

            let resp = client.get(file_url).send().await.context("重新请求文件页面失败")?;
            resp.text().await.context("读取文件页面内容失败")?
        } else {
            bail!("该文件需要密码，请提供密码");
        }
    } else {
        html
    };

    let direct_url = extract_direct_url(&final_html)
        .ok_or_else(|| anyhow!("无法从页面中提取直链下载地址"))?;

    Ok(direct_url)
}

fn extract_direct_url(html: &str) -> Option<String> {

    let iframe_re = Regex::new(r#"<iframe[^>]*src="([^"]+)"[^>]*>"#).unwrap();
    if let Some(caps) = iframe_re.captures(html) {
        if let Some(url) = caps.get(1) {
            let url = url.as_str();
            if url.contains("lanzou") && !url.contains("about:blank") {
                return Some(url.to_string());
            }
        }
    }

    let var_re = Regex::new(r#"(?:var|let|const)\s+\w*(?:url|download|link)\w*\s*=\s*["']([^"']+)["']"#).unwrap();
    for caps in var_re.captures_iter(html) {
        if let Some(url) = caps.get(1) {
            let url = url.as_str();
            if url.contains("lanzou") && url.contains("http") {
                return Some(url.to_string());
            }
        }
    }

    for caps in DIRECT_URL_RE.captures_iter(html) {
        if let Some(url) = caps.get(1) {
            let url = url.as_str();

            if !url.contains("/b0") && !url.contains("/tp/") && url.len() > 30 {
                return Some(url.to_string());
            }
        }
    }

    None
}

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

// ==== 分卷下载（含 ACW 反爬 acw_sc__v2 JS 挑战现算 cookie） ====

static ISNGIS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"var\s+isngis\s*=\s*'([^']+)'"#).unwrap());
static AJAX_FILE_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"ajaxfile\.php\?file=(\d+)"#).unwrap());
static ACW_SCRIPT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?s)<script[^>]*>(.*?)</script>"#).unwrap());

/// 解析蓝奏云新式分享页：提取文件 ID(fid) 与 isngis(sign)，POST downprocess 接口换取直链。
pub async fn resolve_lanzou_direct_url(
    client: &reqwest::Client,
    share_url: &str,
    password: &str,
) -> Result<String> {
    let resp = client
        .get(share_url)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .send()
        .await
        .context("请求蓝奏云分享页失败")?;
    if !resp.status().is_success() {
        bail!("请求分享页失败，状态码: {}", resp.status());
    }
    let html = resp.text().await.context("读取分享页内容失败")?;

    let fid = AJAX_FILE_ID_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| FID_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()))
        .context("无法提取文件 ID (fid)")?;
    let sign = ISNGIS_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .context("无法提取 isngis(sign)")?;

    let api_url = format!("https://apifile.woozooo.com/ajaxfile.php?file={}", fid);
    let form_body = format!(
        "action=downprocess&sign={}&kd=1&p={}",
        percent_encode(&sign),
        percent_encode(password)
    );
    let ajax = client
        .post(&api_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Referer", share_url)
        .header("Origin", "https://apifile.woozooo.com")
        .header("X-Requested-With", "XMLHttpRequest")
        .body(form_body)
        .send()
        .await
        .context("请求蓝奏云下载接口失败")?;
    let text = ajax.text().await.context("读取下载接口响应失败")?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("解析下载接口 JSON 失败: {}", &text[..text.len().min(300)]))?;
    let zt = json.get("zt").and_then(|v| v.as_str()).unwrap_or("0");
    if zt != "1" {
        let info = json.get("info").and_then(|v| v.as_str()).unwrap_or("未知错误");
        bail!("获取下载直链失败: {} (zt={})", info, zt);
    }
    let dom = json.get("dom").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let url = json.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if dom.is_empty() || url.is_empty() {
        bail!("下载接口未返回有效的 dom/url");
    }
    Ok(format!("{}/file/{}", dom.trim_end_matches('/'), url))
}

/// 简易 application/x-www-form-urlencoded 百分号编码（保留 RFC 3986 unreserved 字符）。
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 从响应体中提取 ACW 反爬挑战脚本（含 acw_sc__v2 的 <script> 块）。
fn extract_acw_script(body: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(body);
    if !text.contains("acw_sc__v2") {
        return None;
    }
    for caps in ACW_SCRIPT_RE.captures_iter(&text) {
        if let Some(script) = caps.get(1) {
            if script.as_str().contains("acw_sc__v2") {
                return Some(script.as_str().to_string());
            }
        }
    }
    None
}

/// 用系统 node 子进程执行 ACW 挑战脚本（真实脚本内容原样执行，抗混淆变更），
/// 返回 "acw_sc__v2=xxx" cookie（cookie 与当次 arg1/上下文绑定）。
pub fn solve_acw_cookie(script: &str) -> Result<String> {
    let shim = r#"
var window = this;
var _acw_cookie = '';
var document = {};
Object.defineProperty(document, 'cookie', {
  get: function() { return _acw_cookie; },
  set: function(v) { _acw_cookie = String(v); }
});
var location = { href: '', reload: function() {}, replace: function() {} };
var navigator = { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' };
"#;
    let program = r#"
let buf = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', d => { buf += d; });
process.stdin.on('end', () => {
  try {
    eval(buf);
    console.log(String(_acw_cookie));
  } catch (e) {
    console.error('ACW eval error: ' + (e && e.message ? e.message : e));
    process.exit(1);
  }
});
"#;
    let mut child = std::process::Command::new("node")
        .arg("-e")
        .arg(program)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("无法启动 node 执行 ACW 脚本（请确认系统已安装 Node.js）")?;
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(format!("{shim}\n{script}\n").as_bytes())
                .context("写入 ACW 脚本到 node 失败")?;
        }
    }
    let output = child
        .wait_with_output()
        .context("等待 node 执行 ACW 脚本失败")?;
    if !output.status.success() {
        bail!(
            "ACW 脚本执行失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let cookie = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if cookie.is_empty() {
        bail!("ACW 挑战未生成 cookie");
    }
    Ok(cookie)
}

/// 下载蓝奏云直链到 save_path；首次响应若为 ACW 挑战页（gzip JS 混淆），现算 cookie 后带 cookie 重试。
pub async fn download_part<F>(client: &reqwest::Client, direct_url: &str, save_path: &Path, mut on_progress: F) -> Result<u64>
where
    F: FnMut(u64, Option<u64>),
{
    let mut resp = client
        .get(direct_url)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .send()
        .await
        .context("请求下载直链失败")?;
    if !resp.status().is_success() {
        bail!("下载失败，状态码: {}", resp.status());
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let is_html = content_type.contains("text/html") || content_type.contains("text/plain");

    if is_html {
        let body = resp.bytes().await.context("读取挑战页失败")?;
        let script = extract_acw_script(&body)
            .context("直链首次响应为 HTML，但未解析到 ACW 挑战脚本")?;
        let cookie = solve_acw_cookie(&script)?;
        resp = client
            .get(direct_url)
            .header("Cookie", cookie)
            .header("Referer", "https://wwamt.lanzout.com/")
            .send()
            .await
            .context("携带 ACW cookie 重试下载失败")?;
        if !resp.status().is_success() {
            bail!("携带 ACW cookie 后下载失败，状态码: {}", resp.status());
        }
    }

    let total_size = resp.content_length();
    let mut downloaded: u64 = 0;

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
    Ok(downloaded)
}

/// 按序下载蓝奏云分卷列表到 save_dir（文件名 part1..partN），逐卷解析直链 + ACW 反爬。
pub async fn download_lanzou_parts<F>(
    parts: &[(&str, &str)],
    save_dir: &Path,
    mut on_part_progress: F,
) -> Result<()>
where
    F: FnMut(usize, usize, u64, Option<u64>),
{
    let client = create_client()?;
    std::fs::create_dir_all(save_dir).context("创建分卷目录失败")?;
    for (i, (url, pwd)) in parts.iter().enumerate() {
        let direct_url = resolve_lanzou_direct_url(&client, url, pwd).await?;
        let save_path = save_dir.join(format!("part{}", i + 1));
        download_part(&client, &direct_url, &save_path, |down, total| {
            on_part_progress(i, parts.len(), down, total);
        })
        .await?;
    }
    Ok(())
}

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

    let folder = list_folder(folder_url, folder_password).await?;

    let target = folder
        .files
        .iter()
        .find(|f| f.name.contains(file_name) || file_name.contains(&f.name))
        .ok_or_else(|| anyhow!("未找到文件: {}", file_name))?;

    let direct_url = get_direct_url(&target.url, file_password).await?;

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
