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
            heart_outline, cloud_none, cloud_check, plus, select, character, abstract_tex,
        ) = join!(
            load_texture("icon.png"),
            load_texture("icon_old(home)/resume.png"),
            load_texture("medal.png"),
            load_texture("respack.png"),
            load_texture("message.png"),
            load_texture("settings.png"),
            load_texture("language.png"),
            load_texture("download.png"),
            load_texture("user.png"),
            load_texture("info.png"),
            load_texture("delete.png"),
            load_texture("menu.png"),
            load_texture("edit.png"),
            load_texture("leaderboard.png"),
            load_texture("icon_old(home)/close.png"),
            load_texture("search.png"),
            load_texture("order.png"),
            load_texture("filter.png"),
            load_texture("mod.png"),
            load_texture("star.png"),
            load_texture("star_outline.png"),
            load_texture("heart.png"),
            load_texture("heart_outline.png"),
            load_texture("cloud_none.png"),
            load_texture("cloud_check.png"),
            load_texture("plus.png"),
            load_texture("select.png"),
            load_texture("skel_icon.png"),
            load_texture("abstract.jpg"),
        );

        // hykb 图标单独加载（条件编译）
        #[cfg(feature = "hykb")]
        let hykb = load_texture("hykb.png").await?;

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

            #[cfg(feature = "hykb")]
            hykb: hykb.into(),

            r#abstract: abstract_tex?.into(),
        })
    }
}
