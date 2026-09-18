use crate::scene::TEX_ICON_BACK;
use anyhow::Result;
use futures_util::join;
use macroquad::{
    prelude::{Image, Texture2D},
    texture::load_texture,
};
use prpr::ext::SafeTexture;
use std::cell::RefCell;
use tracing::warn;

/// 缺图标时的占位贴图（1×1 全透明）。
///
/// 以前 `Icons::new()` 里每个图标都是 `?`，**任何一个图标文件缺失都会让整个游戏起不来**
/// （报「初始化失败 / Couldn't load file assets/icons/xxx.png」）。资源是可以被裁剪的，
/// 少一个图标不该等于游戏打不开，所以改成缺哪个警告一句、用占位图顶上。
fn placeholder() -> SafeTexture {
    thread_local! {
        static PH: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    }
    PH.with(|it| {
        if let Some(tex) = it.borrow().as_ref() {
            return tex.clone();
        }
        let tex = SafeTexture::from(Texture2D::from_image(&Image {
            width: 1,
            height: 1,
            bytes: vec![0, 0, 0, 0],
        }));
        *it.borrow_mut() = Some(tex.clone());
        tex
    })
}

/// 加载单个图标；失败只警告并返回占位图。
async fn icon(path: &str) -> SafeTexture {
    match load_texture(path).await {
        Ok(tex) => SafeTexture::from(tex),
        Err(err) => {
            warn!("failed to load icon {path}: {err}; using placeholder");
            placeholder()
        }
    }
}

pub struct Icons {
    pub icon: SafeTexture,
    pub play: SafeTexture,
    pub medal: SafeTexture,
    pub respack: SafeTexture,
    pub msg: SafeTexture,
    pub settings: SafeTexture,
    pub back: SafeTexture,
    pub lang: SafeTexture,
    pub download: SafeTexture,
    pub user: SafeTexture,
    pub info: SafeTexture,
    pub delete: SafeTexture,
    pub menu: SafeTexture,
    pub edit: SafeTexture,
    pub ldb: SafeTexture,
    pub close: SafeTexture,
    pub search: SafeTexture,
    pub order: SafeTexture,
    pub filter: SafeTexture,
    pub r#mod: SafeTexture,
    pub star: SafeTexture,
    pub star_outline: SafeTexture,
    pub heart: SafeTexture,
    pub heart_outline: SafeTexture,
    pub cloud_none: SafeTexture,
    pub cloud_check: SafeTexture,
    pub plus: SafeTexture,
    pub select: SafeTexture,
    pub achievements: SafeTexture,

    #[cfg(feature = "hykb")]
    pub hykb: SafeTexture,

    pub r#abstract: SafeTexture,
}

impl Icons {
    pub async fn new() -> Result<Self> {
        // 并行加载所有图标，显著减少加载时间；单个缺失只退化成占位图（见 `icon`）
        let (
            icon,
            play,
            medal,
            respack,
            msg,
            settings,
            lang,
            download,
            user,
            info,
            delete,
            menu,
            edit,
            ldb,
            close,
            search,
            order,
            filter,
            r#mod,
            star,
            star_outline,
            heart,
            heart_outline,
            cloud_none,
            cloud_check,
            plus,
            select,
            achievements,
            abstract_tex,
        ) = join!(
            icon("icons/icon.png"),
            icon("icon_old(home)/resume.png"),
            icon("icons/medal.png"),
            icon("icons/respack.png"),
            icon("icons/message.png"),
            icon("icons/settings.png"),
            icon("icons/language.png"),
            icon("icons/download.png"),
            icon("icons/user.png"),
            icon("icons/info.png"),
            icon("icons/delete.png"),
            icon("icons/menu.png"),
            icon("icons/edit.png"),
            icon("icons/leaderboard.png"),
            icon("icon_old(home)/close.png"),
            icon("icons/search.png"),
            icon("icons/order.png"),
            icon("icons/filter.png"),
            icon("icons/mod.png"),
            icon("icons/star.png"),
            icon("icons/star_outline.png"),
            icon("icons/heart.png"),
            icon("icons/heart_outline.png"),
            icon("icons/cloud_none.png"),
            icon("icons/cloud_check.png"),
            icon("icons/plus.png"),
            icon("icons/select.png"),
            icon("icons/achievements.png"),
            icon("backgrounds/abstract.jpg"),
        );

        // hykb 图标单独加载（条件编译）
        #[cfg(feature = "hykb")]
        let hykb = icon("icons/hykb.png").await;

        Ok(Self {
            icon,
            play,
            medal,
            respack,
            msg,
            settings,
            // 返回图标更早由主场景装载，没装上也别炸
            back: TEX_ICON_BACK
                .with(|it| it.borrow().clone())
                .unwrap_or_else(placeholder),
            lang,
            download,
            user,
            info,
            delete,
            menu,
            edit,
            ldb,
            close,
            search,
            order,
            filter,
            r#mod,
            star,
            star_outline,
            heart,
            heart_outline,
            cloud_none,
            cloud_check,
            plus,
            select,
            achievements,

            #[cfg(feature = "hykb")]
            hykb,

            r#abstract: abstract_tex,
        })
    }
}
