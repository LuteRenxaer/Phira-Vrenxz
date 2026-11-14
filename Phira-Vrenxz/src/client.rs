
mod model;
pub use model::*;
use tracing::debug;

use crate::{get_data, get_data_mut, save_data};
use anyhow::{anyhow, bail, Context, Result};
use arc_swap::ArcSwap;
use once_cell::sync::Lazy;
use prpr::scene::SimpleRecord;
use prpr_l10n::LANG_IDENTS;
use reqwest::{header, ClientBuilder, Method, RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{borrow::Cow, collections::HashMap, marker::PhantomData, sync::Arc};

pub static CLIENT_TOKEN: Lazy<ArcSwap<Option<String>>> = Lazy::new(|| ArcSwap::from_pointee(None));

static CLIENT: Lazy<ArcSwap<reqwest::Client>> = Lazy::new(|| ArcSwap::from_pointee(basic_client_builder().build().unwrap()));

pub struct Client;

pub const API_URL: &str = "https://phira.5wyxi.com";

pub fn basic_client_builder() -> ClientBuilder {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if let Some(_cid) = attempt.url().as_str().strip_prefix("anys://") {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });
    let mut builder = reqwest::ClientBuilder::new().redirect(policy);
    if get_data().accept_invalid_cert {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder
}

fn client_locale() -> String {
    get_data().language.clone().unwrap_or(LANG_IDENTS[0].to_string())
}

fn build_client(access_token: Option<&str>) -> Result<Arc<reqwest::Client>> {
    CLIENT_TOKEN.store(access_token.map(str::to_owned).into());
    let mut headers = header::HeaderMap::new();
    headers.append(header::ACCEPT_LANGUAGE, header::HeaderValue::from_str(&client_locale())?);
    if let Some(token) = access_token {
        let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {token}"))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);
    }
    Ok(basic_client_builder().default_headers(headers).build()?.into())
}

pub fn set_access_token_sync(access_token: Option<&str>) -> Result<()> {
    CLIENT.store(build_client(access_token)?);
    Ok(())
}

async fn set_access_token(access_token: &str) -> Result<()> {
    CLIENT.store(build_client(Some(access_token))?);
    Ok(())
}

pub async fn recv_raw(request: RequestBuilder) -> Result<Response> {
    let response = request.send().await?;
    if !response.status().is_success() {
        let status = response.status().as_str().to_owned();
        let text = response.text().await.context("failed to receive text")?;
        if let Ok(what) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(detail) = what["error"].as_str() {
                bail!("request failed ({status}): {detail}");
            }
        }
        bail!("request failed ({status}): {text}");
    }
    Ok(response)
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum LoginParams<'a> {
    Password {
        email: &'a str,
        password: &'a str,
    },
    RefreshToken {
        #[serde(rename = "refreshToken")]
        token: &'a str,
    },
}

#[cfg(feature = "hykb")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginResp {
    id: i32,
    token: String,
    refresh_token: String,
}

#[cfg(feature = "hykb")]
#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum HykbLoginResp {
    Ok {
        #[serde(flatten)]
        login: LoginResp,
    },
    NeedChoice {
        hykb_token: String,
    },
}

#[cfg(feature = "hykb")]
pub enum HykbLoginOutcome {

    LoggedIn,

    NeedChoice { hykb_token: String },
}

impl Client {
    #[inline]
    pub fn get(path: impl AsRef<str>) -> RequestBuilder {
        Self::request(Method::GET, path)
    }

    #[inline]
    pub fn post<T: Serialize>(path: impl AsRef<str>, data: &T) -> RequestBuilder {
        Self::request(Method::POST, path).json(data)
    }

    /// 向指定 base URL 发起 POST（用于自定义成绩上传服务器，绕过固定 API_URL）。
    #[inline]
    pub fn post_to<T: Serialize>(base: &str, path: impl AsRef<str>, data: &T) -> RequestBuilder {
        CLIENT.load().post(base.to_string() + path.as_ref()).json(data)
    }

    /// 向指定 base URL 发起 GET（用于自定义服务器拉取排行榜，绕过固定 API_URL）。
    #[inline]
    pub fn get_to(base: &str, path: impl AsRef<str>) -> RequestBuilder {
        CLIENT.load().get(base.to_string() + path.as_ref())
    }

    #[inline]
    pub fn delete(path: impl AsRef<str>) -> RequestBuilder {
        Self::request(Method::DELETE, path)
    }

    pub fn request(method: Method, path: impl AsRef<str>) -> RequestBuilder {
        CLIENT.load().request(method, API_URL.to_string() + path.as_ref())
    }

    pub fn clear_cache<T: Object + 'static>(id: i32) -> Result<bool> {
        let map = obtain_map_cache::<T>();
        let mut guard = map.lock().unwrap();
        let Some(actual_map) = guard.downcast_mut::<ObjectMap<T>>() else {
            unreachable!()
        };
        Ok(actual_map.pop(&id).is_some())
    }

    pub async fn load<T: Object + 'static>(id: i32) -> Result<Arc<T>> {
        {
            let map = obtain_map_cache::<T>();
            let mut guard = map.lock().unwrap();
            let Some(actual_map) = guard.downcast_mut::<ObjectMap<T>>() else {
                unreachable!()
            };
            if let Some(value) = actual_map.get(&id) {
                return Ok(Arc::clone(value));
            }
            drop(guard);
            drop(map);
        }
        Self::fetch(id).await
    }

    pub async fn fetch<T: Object + 'static>(id: i32) -> Result<Arc<T>> {
        Self::fetch_opt(id).await?.ok_or_else(|| anyhow!("entry not found"))
    }

    pub async fn fetch_opt<T: Object + 'static>(id: i32) -> Result<Option<Arc<T>>> {
        let value = Client::fetch_inner::<T>(id).await?;
        let Some(value) = value else { return Ok(None) };
        let value = Arc::new(value);
        let map = obtain_map_cache::<T>();
        let mut guard = map.lock().unwrap();
        let Some(actual_map) = guard.downcast_mut::<ObjectMap<T>>() else {
            unreachable!()
        };
        actual_map.put(id, Arc::clone(&value));
        Ok(Some(value))
    }

    async fn fetch_inner<T: Object>(id: i32) -> Result<Option<T>> {
        let resp = Self::get(format!("/{}/{id}", T::QUERY_PATH)).send().await?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let status = resp.status().as_str().to_owned();
            let text = resp.text().await.context("failed to receive text")?;
            if let Ok(what) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(detail) = what["error"].as_str() {
                    bail!("request failed ({status}): {detail}");
                }
            }
            bail!("request failed ({status}): {text}");
        }
        Ok(Some(resp.json().await?))
    }

    pub fn query<T: Object>() -> QueryBuilder<T> {
        QueryBuilder {
            queries: HashMap::new(),
            page: None,
            suffix: "",
            _phantom: PhantomData,
        }
    }

    pub async fn register(email: &str, username: &str, password: &str) -> Result<()> {
        recv_raw(Self::post(
            "/register",
            &json!({
                "email": email,
                "name": username,
                "password": password,
            }),
        ))
        .await?;
        Ok(())
    }

    pub async fn login(params: LoginParams<'_>) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct FullLoginParams<'a> {
            #[serde(flatten)]
            inner: LoginParams<'a>,
            #[serde(rename = "clientVersion")]
            client_version: &'static str,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            id: i32,
            token: String,
            refresh_token: String,
        }
        let resp: Resp = recv_raw(Self::post(
            "/login",
            &FullLoginParams {
                inner: params,
                client_version: env!("CARGO_PKG_VERSION"),
            },
        ))
        .await?
        .json()
        .await?;

        Self::store_login(resp.id, resp.token, resp.refresh_token).await?;
        Ok(())
    }

    async fn store_login(_id: i32, token: String, refresh_token: String) -> Result<()> {
        set_access_token(&token).await?;
        get_data_mut().tokens = Some((token, refresh_token));
        save_data()?;
        Ok(())
    }

    #[cfg(feature = "hykb")]

    pub async fn login_hykb(uid: i64, access_token: &str) -> Result<HykbLoginOutcome> {
        let resp: HykbLoginResp = recv_raw(Self::post(
            "/login/hykb",
            &json!({
                "hykbUid": uid,
                "accessToken": access_token,
            }),
        ))
        .await?
        .json()
        .await?;
        match resp {
            HykbLoginResp::Ok { login } => {
                Self::store_login(login.id, login.token, login.refresh_token).await?;
                Ok(HykbLoginOutcome::LoggedIn)
            }
            HykbLoginResp::NeedChoice { hykb_token } => Ok(HykbLoginOutcome::NeedChoice { hykb_token }),
        }
    }

    #[cfg(feature = "hykb")]

    pub async fn login_hykb_register(hykb_token: &str, username: &str) -> Result<()> {
        let resp: LoginResp = recv_raw(Self::post(
            "/login/hykb/register",
            &json!({
                "hykbToken": hykb_token,
                "nick": username,
            }),
        ))
        .await?
        .json()
        .await?;
        Self::store_login(resp.id, resp.token, resp.refresh_token).await?;
        Ok(())
    }

    #[cfg(feature = "hykb")]

    pub async fn login_hykb_claim(hykb_token: &str, email: &str, password: &str) -> Result<()> {
        let resp: LoginResp = recv_raw(Self::post(
            "/login/hykb/claim",
            &json!({
                "hykbToken": hykb_token,
                "email": email,
                "password": password,
            }),
        ))
        .await?
        .json()
        .await?;
        Self::store_login(resp.id, resp.token, resp.refresh_token).await?;
        Ok(())
    }

    #[cfg(feature = "hykb")]

    pub async fn bind_hykb(uid: i64, access_token: &str) -> Result<()> {
        recv_raw(Self::post(
            "/me/bind-hykb",
            &json!({
                "hykbUid": uid,
                "accessToken": access_token,
            }),
        ))
        .await?;
        Ok(())
    }

    #[cfg(feature = "hykb")]

    pub async fn unbind_hykb() -> Result<()> {
        recv_raw(Self::post("/me/unbind-hykb", &())).await?;
        Ok(())
    }

    #[cfg(feature = "hykb")]

    pub async fn transfer_request(email: &str) -> Result<()> {
        recv_raw(Self::post("/me/transfer-request", &json!({ "email": email }))).await?;
        Ok(())
    }

    pub async fn get_me() -> Result<User> {
        let user: User = Self::get_me_unchecked().await?;

        #[cfg(feature = "hykb")]
        if user.hykb_uid.is_none() {
            get_data_mut().me = None;
            get_data_mut().tokens = None;
            save_data()?;
            crate::sync_data();
            bail!("{}", crate::ttl!("hykb-not-bound-logout"));
        }
        Ok(user)
    }

    pub async fn get_me_unchecked() -> Result<User> {
        Ok(recv_raw(Self::get("/me")).await?.json().await?)
    }

    pub async fn best_record(id: i32) -> Result<SimpleRecord> {
        Ok(recv_raw(Self::get(format!("/record/best/{id}"))).await?.json().await?)
    }

    pub async fn upload_file(name: &str, bytes: Vec<u8>) -> Result<String> {
        #[derive(Deserialize)]
        struct Resp {
            id: String,
        }
        let resp: Resp = recv_raw(Self::request(Method::POST, format!("/upload/{name}")).body(bytes))
            .await?
            .json()
            .await?;
        Ok(resp.id)
    }

    /// 向指定 base URL 发起 raw body POST（用于上传谱面 zip 等），携带全局鉴权头。
    #[inline]
    pub fn post_raw(base: &str, path: impl AsRef<str>, body: Vec<u8>) -> RequestBuilder {
        CLIENT
            .load()
            .post(base.to_string() + path.as_ref())
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(body)
    }

    /// 本地服务器注册：POST {UPLOAD_BASE}/auth/register {email,password,nick}，
    /// 成功返回并落盘本地会话（id/token/refreshToken），协议对齐本地 server.py。
    pub async fn register_local(email: &str, username: &str, password: &str) -> Result<()> {
        Self::ensure_local_client().await?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            id: i32,
            token: String,
            refresh_token: String,
        }
        let resp: Resp = recv_raw(Self::post_to(
            crate::score_upload::UPLOAD_BASE,
            "/auth/register",
            &json!({
                "email": email,
                "nick": username,
                "password": password,
            }),
        ))
        .await?
        .json()
        .await?;
        Self::store_login(resp.id, resp.token, resp.refresh_token).await?;
        Ok(())
    }

    /// 本地服务器登录：POST {UPLOAD_BASE}/login {email,password,clientVersion}，
    /// 成功返回并落盘本地会话（id/token/refreshToken），协议对齐本地 server.py。
    pub async fn login_local(email: &str, password: &str) -> Result<()> {
        Self::ensure_local_client().await?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            id: i32,
            token: String,
            refresh_token: String,
        }
        let resp: Resp = recv_raw(Self::post_to(
            crate::score_upload::UPLOAD_BASE,
            "/login",
            &json!({
                "email": email,
                "password": password,
                "clientVersion": env!("CARGO_PKG_VERSION"),
            }),
        ))
        .await?
        .json()
        .await?;
        Self::store_login(resp.id, resp.token, resp.refresh_token).await?;
        Ok(())
    }

    /// 获取本地服务器当前登录用户信息：GET {UPLOAD_BASE}/me（Bearer）。
    pub async fn get_me_local() -> Result<User> {
        Ok(recv_raw(Self::get_to(crate::score_upload::UPLOAD_BASE, "/me"))
            .await?
            .json()
            .await?)
    }

    /// 本地服务器使用自签证书，先兜底开启忽略证书校验并重建 client（与上传成绩路径一致）。
    async fn ensure_local_client() -> Result<()> {
        if !get_data().accept_invalid_cert {
            get_data_mut().accept_invalid_cert = true;
            save_data()?;
            set_access_token_sync(CLIENT_TOKEN.load().as_ref().clone().as_deref())?;
        }
        Ok(())
    }

    pub async fn fetch_terms(modified: Option<&str>) -> Result<Option<(String, String)>> {
        let mut req = CLIENT.load().get(format!("{API_URL}/terms/{}.txt", client_locale()));
        if let Some(modified) = modified {
            req = req.header(header::IF_MODIFIED_SINCE, header::HeaderValue::from_str(modified)?);
        }
        let resp = req.send().await?;
        if resp.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        if !resp.status().is_success() {
            bail!("failed to fetch terms: {:?}", resp.status());
        }
        let new_modified = resp
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|it| it.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("invalid last-modified header"))?;
        debug!("{new_modified} {modified:?}");
        if Some(new_modified.as_str()) == modified {

            return Ok(None);
        }
        let new_terms = resp.text().await?;
        Ok(Some((new_terms, new_modified)))
    }
}

#[must_use]
pub struct QueryBuilder<T> {
    queries: HashMap<Cow<'static, str>, Cow<'static, str>>,
    page: Option<u64>,
    suffix: &'static str,
    _phantom: PhantomData<T>,
}

impl<T: Object> QueryBuilder<T> {
    pub fn query(mut self, key: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) -> Self {
        self.queries.insert(key.into(), value.into());
        self
    }

    #[inline]
    pub fn order(self, order: impl Into<Cow<'static, str>>) -> Self {
        self.query("order", order)
    }

    #[inline]
    pub fn tags(self, tags: impl Into<Cow<'static, str>>) -> Self {
        self.query("tags", tags)
    }

    #[inline]
    pub fn search(self, search: impl Into<Cow<'static, str>>) -> Self {
        self.query("search", search)
    }

    #[inline]
    pub fn page_num(self, page_num: u64) -> Self {
        self.query("pageNum", page_num.to_string())
    }

    #[inline]
    pub fn suffix(mut self, suffix: &'static str) -> Self {
        self.suffix = suffix;
        self
    }

    pub fn page(mut self, page: u64) -> Self {
        self.page = Some(page);
        self
    }

    pub async fn send(mut self) -> Result<(Vec<T>, u64)> {
        self.queries.insert("page".into(), (self.page.unwrap_or(0) + 1).to_string().into());
        #[derive(Deserialize)]
        struct PagedResult<T> {
            count: u64,
            results: Vec<T>,
        }
        let res: PagedResult<T> = recv_raw(Client::get(format!("/{}{}", T::QUERY_PATH, self.suffix)).query(&self.queries))
            .await?
            .json()
            .await?;
        Ok((res.results, res.count))
    }
}
