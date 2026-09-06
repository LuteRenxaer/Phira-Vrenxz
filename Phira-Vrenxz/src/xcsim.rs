use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::get_data;

pub const DEFAULT_XCSIM_API_URL: &str = "http://xcapi-dchk.hotanyan.net:20003";
pub const DEFAULT_XCSIM_DOWNLOAD_URL: &str = "http://xcapi-dchk.hotanyan.net:20004";

pub fn api_base_url() -> String {
    get_data()
        .xcsim_api_url
        .as_deref()
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_else(|| DEFAULT_XCSIM_API_URL.to_string())
}

pub fn download_base_url() -> String {
    get_data()
        .xcsim_download_url
        .as_deref()
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_else(|| DEFAULT_XCSIM_DOWNLOAD_URL.to_string())
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct XCSimAccount {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize)]
pub struct XCSimChart {
    pub id: i32,
    pub name: String,
    pub level: Option<String>,
    pub difficulty: Option<f32>,
    pub charter: Option<String>,
    pub composer: Option<String>,
    pub illustrator: Option<String>,
    pub description: Option<String>,
    pub ranked: Option<bool>,
    pub reviewed: Option<bool>,
    pub illustration: Option<String>,
    pub preview: Option<String>,
    pub file: Option<String>,
    pub uploader: Option<i32>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub chart_updated: Option<String>,
    pub tags: Option<Vec<String>>,
    pub rating: Option<f32>,
}

#[derive(Deserialize)]
pub struct XCSimChartList {
    pub count: u64,
    pub results: Vec<XCSimChart>,
}

#[derive(Deserialize)]
pub struct XCSimUser {
    pub id: i32,
    pub name: String,
    pub avatar: Option<String>,
    pub badge: Option<String>,
    pub badges: Option<Vec<String>>,
    pub language: Option<String>,
    pub bio: Option<String>,
    pub exp: Option<f32>,
    pub rks: Option<f32>,
    pub roles: Option<i32>,
    pub joined: Option<String>,
    pub last_login: Option<String>,
}

#[derive(Deserialize)]
struct LoginResponse {
    id: i32,
    token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
}

#[derive(Deserialize)]
struct RegisterResponse {
    id: i32,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

fn build_client() -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .danger_accept_invalid_certs(true);
    if get_data().accept_invalid_cert {
        builder = builder.danger_accept_invalid_certs(true);
    }
    Ok(builder.build()?)
}

fn api_url(path: &str) -> String {
    format!("{}{}", api_base_url(), path)
}

pub fn rehost_url(url: &str) -> String {
    if url.contains("/files/") {
        if let Some(idx) = url.find("/files/") {
            return format!("{}{}", download_base_url(), &url[idx..]);
        }
    }
    url.to_string()
}

async fn recv_raw(resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        let body = resp.text().await?;
        let msg = if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            err.error
        } else {
            body
        };
        Err(anyhow!("request failed ({status}): {msg}"))
    }
}

pub async fn register(email: Option<&str>, name: &str, password: &str) -> Result<i32> {
    let client = build_client()?;
    let mut body = json!({
        "name": name,
        "password": password,
    });
    if let Some(email) = email {
        body["email"] = json!(email);
    }
    let resp = client.post(api_url("/register")).json(&body).send().await?;
    let resp = recv_raw(resp).await?;
    let result: RegisterResponse = resp.json().await?;
    Ok(result.id)
}

pub async fn login(email: &str, password: &str) -> Result<(i32, String, String)> {
    let client = build_client()?;
    let body = json!({
        "email": email,
        "password": password,
    });
    let resp = client.post(api_url("/login")).json(&body).send().await?;
    let resp = recv_raw(resp).await?;
    let result: LoginResponse = resp.json().await?;
    Ok((result.id, result.token, result.refresh_token))
}

pub async fn refresh_login(refresh_token: &str) -> Result<(i32, String, String)> {
    let client = build_client()?;
    let body = json!({
        "refreshToken": refresh_token,
    });
    let resp = client.post(api_url("/login")).json(&body).send().await?;
    let resp = recv_raw(resp).await?;
    let result: LoginResponse = resp.json().await?;
    Ok((result.id, result.token, result.refresh_token))
}

pub async fn get_me(access_token: &str) -> Result<XCSimUser> {
    let client = build_client()?;
    let resp = client
        .get(api_url("/me"))
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await?;
    let resp = recv_raw(resp).await?;
    let user: XCSimUser = resp.json().await?;
    Ok(user)
}

pub async fn fetch_charts(
    access_token: Option<&str>,
    search: Option<&str>,
    page: u32,
    page_num: u32,
) -> Result<(Vec<XCSimChart>, u64)> {
    let client = build_client()?;
    let mut req = client.get(api_url("/chart"))
        .query(&[("page", page + 1), ("pageNum", page_num)]);
    if let Some(search) = search {
        if !search.is_empty() {
            req = req.query(&[("search", search)]);
        }
    }
    if let Some(token) = access_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await?;
    let resp = recv_raw(resp).await?;
    let list: XCSimChartList = resp.json().await?;
    Ok((list.results, list.count))
}

pub async fn fetch_chart(access_token: Option<&str>, id: i32) -> Result<XCSimChart> {
    let client = build_client()?;
    let mut req = client.get(api_url(&format!("/chart/{id}")));
    if let Some(token) = access_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await?;
    let resp = recv_raw(resp).await?;
    let chart: XCSimChart = resp.json().await?;
    Ok(chart)
}

pub async fn download_file_bytes(url: &str) -> Result<Vec<u8>> {
    let client = build_client()?;
    let url = rehost_url(url);
    let resp = client.get(&url).send().await?;
    let resp = recv_raw(resp).await?;
    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

pub async fn download_chart(
    access_token: Option<&str>,
    id: i32,
    target_dir: &std::path::Path,
) -> Result<()> {
    let chart = fetch_chart(access_token, id).await?;
    let file_url = chart
        .file
        .as_ref()
        .ok_or_else(|| anyhow!("XC-SIM 谱面 {} 没有可下载的文件", id))?;
    let bytes = download_file_bytes(file_url).await?;
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir)?;
    }
    std::fs::create_dir_all(target_dir)?;
    let dir = prpr::dir::Dir::new(target_dir)?;
    prpr::ext::unzip_into(std::io::Cursor::new(bytes), &dir, false)?;
    Ok(())
}

pub fn is_logged_in() -> bool {
    get_data().xcsim_account.access_token.is_some()
}

pub fn account() -> &'static XCSimAccount {
    &get_data().xcsim_account
}
