
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
item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
item-bgm-sub = Splash / Home background music
item-show-fps = Show FPS
item-show-fps-sub = Show realtime FPS while playing
item-perf = Performance Profile
item-perf-sub = The profile takes over culling / particles / vsync. Choose Custom to control each item manually.
item-perf-off = No Optimization
item-perf-off-sub = Extra optimizations off: serial note-load counting, no off-screen culling, low-res never used.
item-perf-light = Light
item-perf-light-sub = Off-screen culling + low-res / FX-density adaptation (note counting stays serial).
item-perf-medium = Medium
item-perf-medium-sub = Half of the full optimization: culling + low-res + parallel note counting (higher threshold).
item-perf-full = Full
item-perf-full-sub = All current optimizations: culling + low-res adaptation + parallel note counting.
item-perf-ultra = Ultra
item-perf-ultra-sub = Full culling + particle reduction (keeps hit_fx, halves successive bursts) + forces vsync off.
item-perf-custom = Custom
item-perf-custom-sub = Manually control the items below.
item-perf-custom-metrics = Parallel note counting
item-perf-custom-metrics-sub = Count per-frame note load on all cores.
item-perf-custom-cull = Off-screen culling
item-perf-custom-cull-sub = Skip rendering notes outside the screen.
item-perf-custom-fx = Particle reduction
item-perf-custom-fx-sub = Keep hit_fx main particle, drop debris and halve successive bursts.
item-perf-custom-vsync = Force vsync off
item-perf-custom-vsync-sub = Frame rate is no longer limited by the refresh rate.
item-perf-custom-lowres = Low-res note threshold
item-perf-custom-lowres-sub = Use low-res textures when visible notes ≥ this value.
item-perf-custom-fxdensity = Hit-FX density threshold
item-perf-custom-fxdensity-sub = Disable hit effects when upcoming notes exceed this value.
item-startup-screen = Show Startup Screen
item-startup-screen-sub = Show the startup screen before the main menu (includes language selection)
item-full-screen-judge = Full Screen Judge
item-full-screen-judge-sub = Enable judgment on entire screen (experimental)
item-arcaca-judgement = Arcaea Judgment Mode
item-arcaca-judgement-sub = Use Arcaea scoring (max 10000000+ note count), disable score upload
item-fnf-judgement = FNF Judgment Mode
item-fnf-judgement-sub = Use FNF scoring (Sick350/Good200/Bad100), disable score upload
item-combo-text = Combo Display Text
item-combo-text-default = COMBO
item-combo-text-edit = Edit
item-watermark = Custom Watermark
item-watermark-default = Phira-Vrenxz
item-watermark-edit = Edit
custom-section-appearance = Appearance
custom-ui-scale = UI Scale
custom-show-score = Show Score
custom-show-combo = Show Combo
custom-show-acc = Show Accuracy
custom-accent-color = Accent Color
custom-edit = Edit
custom-section-character = Character Stand
custom-character = Current Character
custom-character-intro = Character Intro
custom-character-no-intro = No intro available
custom-character-switch = Switch Character
custom-character-switch-btn = Switch
custom-section-ui-position = UI Position
custom-score-x = Score X Offset
custom-score-y = Score Y Offset
custom-combo-x = Combo X Offset
custom-combo-y = Combo Y Offset
custom-section-text = Custom Text
custom-watermark = Custom Watermark
custom-combo-text = Custom Combo Text
custom-autoplay-text = Autoplay Display Text
custom-section-background = Custom Background
custom-select-bg = Select Background Image
custom-reset-bg = Reset Background
custom-reset = Reset
custom-default-bg = Default Background
custom-section-music = Custom Music
custom-home-bgm = Splash / Home BGM
custom-reset-home-bgm = Reset Splash / Home BGM
custom-startup-bgm = Startup BGM
custom-reset-startup-bgm = Reset Startup BGM
custom-default-music = Default Music
custom-section-crash = Custom Crash
custom-crash-title = Crash Title
custom-crash-code = Crash Code
custom-crash-reason = Crash Reason
custom-trigger-crash = Trigger Custom Crash
custom-trigger-crash-sub = Trigger crash with above settings
custom-trigger = Trigger
custom-not-set = Not set
custom-current-code = Current code: { $code }
custom-bg-set = Background set
custom-bg-reset = Background reset
custom-bgm-set = BGM set
custom-bgm-reset = BGM reset
custom-startup-bgm-set = Startup BGM set
custom-startup-bgm-reset = Startup BGM reset
old-home-restart = Restart to apply
home-ui = Home UI
old-home-style = Old Home Style
play-button-x = Play Button X
play-button-y = Play Button Y
menu-buttons-x = Menu Buttons X
menu-buttons-y = Menu Buttons Y
item-crash-btn = Fun - Crash Button
item-crash-btn-sub = Click to trigger crash page (error code 951)
item-crash = Crash
custom-tab = Custom
section-basic = Basic
section-network = Network
section-display = Display
section-storage = Storage & Reset
item-fxaa = FXAA Anti-Aliasing
item-fxaa-sub = Enable FXAA anti-aliasing to improve image quality
item-roman-numerals = Roman Numerals
item-roman-numerals-sub = Use Roman numerals to display difficulty level
item-chinese-numerals = Chinese Numerals
item-chinese-numerals-sub = Use Chinese numerals to display difficulty level
item-hud = HUD Customization
item-hud-sub = Drag the components of the main menu, library and settings to reposition and resize them
item-hud-open = Manage
item-reset-settings = Reset to Default
item-reset-settings-sub = Restore all settings to default values
item-reset-settings-btn = Reset
item-reset-settings-done = Settings reset to default
item-custom-bgm = Custom Background Music
item-custom-bgm-sub = Select a local audio file as home BGM (restart to apply)
item-custom-bgm-default = Default
item-custom-bgm-set = Custom BGM set
item-custom-bgm-reset = Default BGM restored
item-reset-bgm = Reset Default BGM
item-reset-bgm-btn = Reset
item-custom-startup-bgm = Custom Startup Music
item-custom-startup-bgm-sub = Select a local audio file as startup BGM
item-custom-startup-bgm-default = Default (login.mp3)
item-custom-startup-bgm-set = Startup BGM set
item-custom-startup-bgm-reset = Default startup BGM restored
item-reset-startup-bgm = Reset Default Startup BGM
item-reset-startup-bgm-btn = Reset
item-custom-bg = Custom Home Background
item-custom-bg-sub = Select a local image as home background
item-custom-bg-default = Default
item-custom-bg-set = Custom background set
item-custom-bg-reset = Default background restored
item-reset-bg = Reset Default Background
item-reset-bg-btn = Reset
item-particle = Particle Effects
item-particle-sub = Enable particle effects in gameplay
item-disable-effect = Disable Effects
item-disable-effect-sub = Disable all visual effects to improve performance
item-interactive = Interactive Mode
item-interactive-sub = Interaction effects between notes and judgment line
tutorial = Tutorial
tutorial-desc = Tap to enter the built-in tutorial chart
tutorial-loading = Loading...
tutorial-start = Start Tutorial
tutorial-load-failed = Failed to load tutorial
settings-console-log = Console Log
settings-console-log-desc = Show cmd console window for debug logs when enabled
settings-vsync = VSync
settings-vsync-desc = Limit FPS to monitor refresh rate when enabled, disable for higher FPS
settings-open = Open
item-home-bg = Built-in Home Background
item-home-bg-default = Default
item-home-bg-one = Background 1
item-home-bg-two = Background 2
item-home-bg-three = Background 3
item-home-bg-old = Legacy Background
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending
custom-section-font = Font
item-font = Font
item-font-sub = Switch the font used by the UI
item-import-font = Import font
item-import-font-sub = Import a TTF/OTF font file and switch to it anytime
font-default = Default
font-import = Import
font-switched = Font switched
font-switch-failed = Failed to switch font
migrate-settings-item = Sync legacy data
migrate-settings-item-sub = Import from an older PhirLie / Phira-Vrenxz data folder (the auto prompt only appears before the first sync)
migrate-settings-now = Sync now
custom-character-name = Character Name
custom-character-illust = Character Illustration
custom-character-no-illust = No illustration
custom-character-reset = Reset Customization
custom-character-reset-done = Character customization reset
custom-character-illust-set = Illustration updated
custom-character-illust-failed = Failed to update illustration
visual = Visual
color-palette-title = Color Palette
color-palette-done = Done
not-logged-in = Not Logged In
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
tab-general = General
tab-account = Account
tab-audio = Audio
tab-chart = Chart
tab-visual = Chart UI
tab-custom = Custom
tab-debug = Debug
tab-plugin = Plugins
tab-about = About
row-delete = Delete
custom-select = Select
custom-not-selected = Not selected
item-autoplay-text-default = Autoplay
profile-id = ID: { $id }
profile-id-none = ID: --
