
label = 設定

general = 通用
audio = 音頻
chart = 譜面
debug = 調試
about = 關於

item-lang = 語言
item-fullscreen = 全屏之態
item-offline = 離線之態
item-offline-sub = 離線之態不可上傳成績
item-server-status = 伺服器之態
item-server-status-sub = 轉網頁以觀伺服器之態
check-status = 觀
item-mp = 多人同戲
item-mp-sub = 啓多人同戲
item-mp-addr = 多人同戲之伺服器
item-mp-addr-sub = 伺服器之位，「主機：端口」
item-mp-addr-invalid = 無效之伺服器位
item-lowq = 低畫質之態
item-lowq-sub = 議於畫面滯時啓之
item-clear-cache = 清緩存
item-cache-size-loading = 載中…
item-cache-size = 緩存之量：{ $size }
item-clear-cache-btn = 清
item-cache-cleared = 緩存已清
item-insecure = 不穩之態
item-insecure-sub = 當不可用線上功能時可試此。此將使卿之連接不穩！
item-enable-anys = 啓 Anys
item-enable-anys-sub = 用 Anys 關口以增連接之質
item-anys-gateway = Anys 關口
item-anys-gateway-sub = Anys 關口之位
item-anys-gateway-invalid = 無效之關口位

item-adjust = 自動齊時
item-adjust-sub = 自動調延遲以同步音樂與譜面
item-music = 音樂之聲
item-sfx = 音效之聲
item-bgm = BGM 之聲
item-cali = 調延遲
item-preferred-sample-rate = 首選採樣率
preferred-sample-rate-default = 系統默認
item-audio-buffer-size = 音頻緩衝之量

item-show-acc = 顯實時準度
item-show-avg-fps = 顯平均幀率
item-show-avg-fps-sub = 於結算之面顯平均幀率
item-ap-fc-indicator = AP/FC（全完美/全連）之指
item-ap-fc-indicator-sub = 用判定線之色顯全 AP/FC 之態
item-dc-pause = 雙擊暫停
item-dhint = 雙押之示
item-dhint-sub = 同時觸線之音符將被高亮
item-opt = 激進之優
item-opt-sub = 採激進之優化之策，增性能然或致部分譜面顯示有誤
item-use-keyboard = 用鍵盤戲
item-use-keyboard-sub = 啓後可用鍵盤戲，然成績不可上傳
item-prefer-reduced-motion = 減動畫之效
item-prefer-reduced-motion-sub = 減動畫與視覺之特效
item-speed = 速
item-note-size = 音符之巨

item-chart-debug = 譜面之調
item-chart-debug-sub = 顯判定線之號與向
item-touch-debug = 觸摸之調
item-touch-debug-sub = 戲中顯觸點

load-cali-failed = 音頻爲流螢所食

about-content =
  Phira-Firefly v{ $version }

  Phira-Firefly 乃一玩法基於 Phigros 之非商業社區音樂戲，以 Rust 所造，由小天是个小男娘基於開源之碼所改。

  BiliBili 之號：@Phira官方 & @小天不是小男娘
  QQ 頻道：r48eajexth

  議加入 QQ 頻道以獲最新消息與得助！

  工作人員之表（字典序）
  開發
  { $development }

  運維
  { $operations }

  文檔
  { $documentation }

  美術
  { $art }

  音樂
  { $music }

  音效
  { $audio }

  社區管理
  { $community }

  本地化
  { $localization }

  以及衆多志願譜面審核員！完整之表見 https://phira.moe/staff
item-ui-style = 界面之風
item-ui-style-sub = 易整體界面之貌；默認之風與舊版相類
item-ui-style-default = 默認
item-ui-style-xcsim = XC-SIM
item-bgm-sub = 開場與主頁之樂
item-show-fps = 顯幀率
item-show-fps-sub = 戲中實時顯幀率
item-perf = 性能之檔
item-perf-sub = 檔位統攝剔除、粒子與垂直同步；擇「自定」則可逐項手調
item-perf-off = 不優化
item-perf-off-sub = 關諸般增益：逐幀串行計音、不剔屏外之物、不用低分辨率
item-perf-light = Light
item-perf-light-sub = 剔屏外之物，並低分辨率與特效密度自適應（計音仍串行）
item-perf-medium = Medium
item-perf-medium-sub = 全量之半：剔除、低分辨率、並行計音（閾值較高）
item-perf-full = Full
item-perf-full-sub = 現有諸優化俱開：剔除、低分辨率自適應、並行計音
item-perf-ultra = Ultra
item-perf-ultra-sub = 全剔除、減粒子（留 hit_fx，連發減半），並強關垂直同步
item-perf-custom = Custom
item-perf-custom-sub = 以下諸項由卿自調
item-perf-custom-metrics = 並行計音
item-perf-custom-metrics-sub = 以諸核並行計每幀之音
item-perf-custom-cull = 剔屏外之物
item-perf-custom-cull-sub = 屏外之音不復繪
item-perf-custom-fx = 減粒子
item-perf-custom-fx-sub = 留 hit_fx 主粒，去碎屑，連發減半
item-perf-custom-vsync = 強關垂直同步
item-perf-custom-vsync-sub = 幀率不復受限於顯示器之刷新
item-perf-custom-lowres = 低分辨率之閾
item-perf-custom-lowres-sub = Use low-res textures when visible notes ≥ this value.
item-perf-custom-fxdensity = 打擊特效密度之閾
item-perf-custom-fxdensity-sub = 將至之音逾此數則不繪打擊特效
item-startup-screen = 顯開場之幕
item-startup-screen-sub = 主菜單前顯開場幕（含擇語）
item-full-screen-judge = 全屏判定
item-full-screen-judge-sub = 全屏皆可判定（試行）
item-arcaca-judgement = Arcaea 判定之式
item-arcaca-judgement-sub = 用 Arcaea 計分（上限 10000000 加音數），並禁上傳成績
item-fnf-judgement = FNF 判定之式
item-fnf-judgement-sub = 用 FNF 計分（Sick350／Good200／Bad100），並禁上傳成績
item-combo-text = 連擊之文
item-combo-text-default = COMBO
item-combo-text-edit = Edit
item-watermark = 自定水印
item-watermark-default = Phira-Vrenxz
item-watermark-edit = Edit
custom-section-appearance = 外觀
custom-ui-scale = 界面之度
custom-show-score = 顯分數
custom-show-combo = 顯連擊
custom-show-acc = 顯準確
custom-accent-color = 主色
custom-edit = Edit
custom-section-character = 角色立繪
custom-character = 當前角色
custom-character-intro = 角色之介
custom-character-no-intro = 尚無介文
custom-character-switch = 易角色
custom-character-switch-btn = Switch
custom-section-ui-position = 界面之位
custom-score-x = 分數橫偏
custom-score-y = 分數縱偏
custom-combo-x = 連擊橫偏
custom-combo-y = 連擊縱偏
custom-section-text = 自定之文
custom-watermark = 自定水印
custom-combo-text = 自定連擊之文
custom-autoplay-text = 自動演示之文
custom-section-background = 自定背景
custom-select-bg = 擇背景之圖
custom-reset-bg = 復默認背景
custom-reset = Reset
custom-default-bg = 默認背景
custom-section-music = 自定之樂
custom-home-bgm = 開場與主頁之樂
custom-reset-home-bgm = 復默認開場與主頁之樂
custom-startup-bgm = 開場之樂
custom-reset-startup-bgm = 復默認開場之樂
custom-default-music = 默認之樂
custom-section-crash = 自定崩壞
custom-crash-title = 崩壞之題
custom-crash-code = 崩壞之碼
custom-crash-reason = 崩壞之由
custom-trigger-crash = 觸自定崩壞
custom-trigger-crash-sub = 依上設觸發崩壞
custom-trigger = 觸
custom-not-set = 未設
custom-current-code = 當下之碼：{ $code }
custom-bg-set = 背景已設
custom-bg-reset = 背景已復
custom-bgm-set = 樂已設
custom-bgm-reset = 樂已復
custom-startup-bgm-set = 開場之樂已設
custom-startup-bgm-reset = 開場之樂已復
old-home-restart = 重啓方生效
home-ui = 主頁界面
old-home-style = 舊主頁之風
play-button-x = 開始鍵橫偏
play-button-y = 開始鍵縱偏
menu-buttons-x = 菜單鍵橫偏
menu-buttons-y = 菜單鍵縱偏
item-crash-btn = 趣—崩壞之鍵
item-crash-btn-sub = 按之則入崩壞之頁（錯碼 951）
item-crash = Crash
custom-tab = Custom
section-basic = Basic
section-network = 網絡
section-display = 顯示
section-storage = 儲存與重置
item-fxaa = FXAA 抗鋸
item-fxaa-sub = 啓 FXAA 抗鋸以增畫質
item-roman-numerals = 羅馬數字
item-roman-numerals-sub = 以羅馬數字示難度
item-chinese-numerals = 漢字數字
item-chinese-numerals-sub = 以漢字示難度
item-hud = HUD 自定
item-hud-sub = 可拖主頁、曲庫與設定之件，以易其位與大小
item-hud-open = Manage
item-reset-settings = 復默認
item-reset-settings-sub = 諸設俱復默認
item-reset-settings-btn = Reset
item-reset-settings-done = 諸設已復默認
item-custom-bgm = 自定主頁之樂
item-custom-bgm-sub = 擇本地音文為主頁之樂（重啓生效）
item-custom-bgm-default = 默認
item-custom-bgm-set = 自定之樂已設
item-custom-bgm-reset = 已復默認之樂
item-reset-bgm = 復默認之樂
item-reset-bgm-btn = Reset
item-custom-startup-bgm = 自定開場之樂
item-custom-startup-bgm-sub = 擇本地音文為開場之樂
item-custom-startup-bgm-default = 默認（login.mp3）
item-custom-startup-bgm-set = 開場之樂已設
item-custom-startup-bgm-reset = 已復默認開場之樂
item-reset-startup-bgm = 復默認開場之樂
item-reset-startup-bgm-btn = Reset
item-custom-bg = 自定主頁之景
item-custom-bg-sub = 擇本地之圖為主頁之景
item-custom-bg-default = 默認
item-custom-bg-set = 自定之景已設
item-custom-bg-reset = 已復默認之景
item-reset-bg = 復默認之景
item-reset-bg-btn = Reset
item-particle = 粒子之效
item-particle-sub = 戲中啓粒子之效
item-disable-effect = 關特效
item-disable-effect-sub = 關諸視效以增性能
item-interactive = 交互之態
item-interactive-sub = 音與判定線之交感
tutorial = 教程
tutorial-desc = 按之則入內置教程譜
tutorial-loading = 載中…
tutorial-start = 始教程
tutorial-load-failed = 教程載敗
settings-console-log = 控制台之誌
settings-console-log-desc = 開則顯 cmd 控制台之窗，以觀調試之誌
settings-vsync = VSync
settings-vsync-desc = 開則幀率限於顯示器之刷新，關則可增幀率
settings-open = Open
item-home-bg = 內置主頁之景
item-home-bg-default = 默認
item-home-bg-one = 景一
item-home-bg-two = 景二
item-home-bg-three = 景三
item-home-bg-old = 舊景
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending
custom-section-font = Font
item-font = Font
item-font-sub = 易界面之字體
item-import-font = 導入字體
item-import-font-sub = 導入 TTF／OTF 字體，隨時可易
font-default = 默認
font-import = Import
font-switched = 字體已易
font-switch-failed = 字體易敗
migrate-settings-item = 同步舊版之數據
migrate-settings-item-sub = 自舊版 PhirLie／Phira-Vrenxz 之 data 目錄導入（僅初同步前自提示）
migrate-settings-now = 即刻同步
custom-character-name = 角色之名
custom-character-illust = 角色之繪
custom-character-no-illust = 尚無繪
custom-character-reset = 復角色之設
custom-character-reset-done = 角色之設已復
custom-character-illust-set = 繪已更
custom-character-illust-failed = 繪更敗
visual = Visual
color-palette-title = 調色之盤
color-palette-done = Done
not-logged-in = 未登入
about-update-log =
  Version: V1.3.3
  Update notes:
  1. Fixed a few bugs
  2. Added some secret settings
  3. PC has secret Debug stuff

  Original development: Prpr, Phira
  Operations & maintenance: Lute_Rencai
  Testers:
  Yangyangjiang~
  Jihe Linyu
  DVD
  Development:
  Lute_Rencai
  Jihe Linyu
  Bug QA:
  Jihe Linyu
  Jungle
  Art:
  Jihe Linyu
  Lute_Rencai
  Blue Archive

  If you downloaded this from elsewhere, please join the
  Phira-Vrenxz/Phi Launcher official group
  QQ: 1103288774

# Phira-Vrenxz settings
tab-general = 通用
tab-account = 帳號
tab-audio = Audio
tab-chart = Chart
tab-visual = 譜面界面
tab-custom = Custom
tab-debug = Debug
tab-plugin = 插件
tab-about = About
row-delete = Delete
custom-select = Select
custom-not-selected = 未擇
item-autoplay-text-default = 自動演示
profile-id = 編號：{ $id }
profile-id-none = ID: --
sx-account-switch = 切換帳號
sx-account-add = 登入他帳
sx-account-current = 當下
sx-account-switching = 正切換帳號…
sx-account-switched = 已切換帳號
sx-account-switch-failed = 切換帳號未成
