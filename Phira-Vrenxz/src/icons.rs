use crate::scene::TEX_ICON_BACK;
use anyhow::Result;
use futures_util::join;
use macroquad::texture::load_texture;
use prpr::ext::SafeTexture;

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
    pub character: SafeTexture,
    pub achievements: SafeTexture,

    #[cfg(feature = "hykb")]
    pub hykb: SafeTexture,

    pub r#abstract: SafeTexture,
}

impl Icons {
    pub async fn new() -> Result<Self> {
        // 并行加载所有图标，显著减少加载时间
        let (
            icon, play, medal, respack, msg, settings, lang, download, user, info, delete,
            menu, edit, ldb, close, search, order, filter, r#mod, star, star_outline, heart,
            heart_outline, cloud_none, cloud_check, plus, select, character, achievements, abstract_tex,
        ) = join!(
            load_texture("icons/icon.png"),
            load_texture("icon_old(home)/resume.png"),
            load_texture("icons/medal.png"),
            load_texture("icons/respack.png"),
            load_texture("icons/message.png"),
            load_texture("icons/settings.png"),
            load_texture("icons/language.png"),
            load_texture("icons/download.png"),
            load_texture("icons/user.png"),
            load_texture("icons/info.png"),
            load_texture("icons/delete.png"),
            load_texture("icons/menu.png"),
            load_texture("icons/edit.png"),
            load_texture("icons/leaderboard.png"),
            load_texture("icon_old(home)/close.png"),
            load_texture("icons/search.png"),
            load_texture("icons/order.png"),
            load_texture("icons/filter.png"),
            load_texture("icons/mod.png"),
            load_texture("icons/star.png"),
            load_texture("icons/star_outline.png"),
            load_texture("icons/heart.png"),
            load_texture("icons/heart_outline.png"),
            load_texture("icons/cloud_none.png"),
            load_texture("icons/cloud_check.png"),
            load_texture("icons/plus.png"),
            load_texture("icons/select.png"),
            load_texture("icons/skel_icon.png"),
            load_texture("icons/achievements.png"),
            load_texture("backgrounds/abstract.jpg"),
        );

        // hykb 图标单独加载（条件编译）
        #[cfg(feature = "hykb")]
        let hykb = load_texture("icons/hykb.png").await?;

        Ok(Self {
            icon: icon?.into(),
            play: play?.into(),
            medal: medal?.into(),
            respack: respack?.into(),
            msg: msg?.into(),
            settings: settings?.into(),
            lang: lang?.into(),
            back: TEX_ICON_BACK.with(|it| it.borrow().clone().unwrap()),
            download: download?.into(),
            user: user?.into(),
            info: info?.into(),
            delete: delete?.into(),
            menu: menu?.into(),
            edit: edit?.into(),
            ldb: ldb?.into(),
            close: close?.into(),
            search: search?.into(),
            order: order?.into(),
            filter: filter?.into(),
            r#mod: r#mod?.into(),
            star: star?.into(),
            star_outline: star_outline?.into(),
            heart: heart?.into(),
            heart_outline: heart_outline?.into(),
            cloud_none: cloud_none?.into(),
            cloud_check: cloud_check?.into(),
            plus: plus?.into(),
            select: select?.into(),
            character: character?.into(),
            achievements: achievements?.into(),

            #[cfg(feature = "hykb")]
            hykb: hykb.into(),

            r#abstract: abstract_tex?.into(),
        })
    }
}
