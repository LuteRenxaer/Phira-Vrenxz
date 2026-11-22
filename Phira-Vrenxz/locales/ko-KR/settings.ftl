
label = 설정

item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
general = 일반
audio = 오디오
chart = 비트맵
debug = 디버그
about = 정보

item-lang = 언어
item-offline = 오프라인 모드
item-offline-sub = 오프라인 모드에서는 플레이 기록을 업로드할 수 없습니다.
item-server-status = 서버 상태
item-server-status-sub = 웹페이지로 이동하여 서버 상태를 확인하세요
check-status = 확인
item-mp = 멀티플레이어
item-mp-sub = 멀티플레이어 모드 활성화
item-mp-addr = 멀티플레이어 서버
item-mp-addr-sub = 서버 주소, '호스트:포트'
item-mp-addr-invalid = 잘못된 서버 주소
item-lowq = 저화질 모드
item-lowq-sub = UI가 느릴 때 이 기능을 활성화하세요.
item-insecure = 보안 모드 끄기
item-insecure-sub = 온라인 기능을 사용할 수 없는 경우 이를 활성화하십시오. 연결이 보안되지 않습니다!
item-enable-anys = Anys 활성화
item-enable-anys-sub = 연결 품질을 향상시키기 위해 Anys 게이트웨이를 사용합니다
item-anys-gateway = Anys 게이트웨이
item-anys-gateway-sub = Anys 게이트웨이 주소
item-anys-gateway-invalid = 잘못된 게이트웨이 주소입니다

item-adjust = 자동 시간 조정
item-adjust-sub = 음악과 비트맵을 동기화하기 위해 시간을 동적으로 조정합니다.
item-music = 음악 볼륨
item-sfx = 효과음 볼륨
item-bgm = BGM 볼륨
item-bgm-sub = Splash / Home background music
item-cali = 오프셋 조정

item-show-acc = 실시간 정확도 표시
item-dc-pause = 더블 클릭으로 일시 정지
item-dhint = 더블 힌트
item-dhint-sub = 선을 따라 동시에 터치하는 노트가 강조됩니다.
item-opt = 고성능 최적화
item-opt-sub = 성능을 향상시킬 수 있지만, 부정확한 동작을 유발할 수 있습니다.
item-use-keyboard = 키보드 사용
item-use-keyboard-sub = 키보드로 게임을 플레이할 수 있습니다. 활성화하면 점수가 업로드되지 않습니다.
item-prefer-reduced-motion = 애니메이션 줄이기
item-prefer-reduced-motion-sub = 애니메이션과 시각 효과를 줄입니다
item-speed = 노트 속도
item-note-size = 노트 크기

item-chart-debug = 비트맵 디버그 모드
item-chart-debug-sub = 라인 번호 및 방향 표시
item-touch-debug = 터치 디버그 모드
item-touch-debug-sub = 터치 지점 표시

load-cali-failed = 오디오를 로드하는데 실패했습니다.

about-content =
  Phira v{ $version }

  Phira는 Phigros에서 영감을 받은 비상업적 커뮤니티 중심의 리듬 게임입니다.

  BiliBili 계정: @Phira官方
  QQ 길드: r48eajexth
  Discord 서버: discord.gg/gqpR3bTSsP

  최신 소식을 받고 도움을 받으려면 QQ 길드 또는 Discord 서버에 가입하는 것을 권장합니다.

  스태프 목록 (사전순)
  개발
  { $development }

  운영
  { $operations }

  문서
  { $documentation }

  아트
  { $art }

  음악
  { $music }

  오디오
  { $audio }

  커뮤니티 관리
  { $community }

  현지화 기여자
  { $localization }

  그리고 많은 자원봉사 채보 리뷰어들에게 감사드립니다. 전체 목록은 https://phira.moe/staff 를 참조하세요.

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
item-home-bg = 내장 홈 배경
item-home-bg-default = 기본
item-home-bg-one = 배경 1
item-home-bg-two = 배경 2
item-home-bg-three = 배경 3
item-home-bg-old = 구버전 배경
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending

custom-section-font = 글꼴
item-font = 글꼴
item-font-sub = 인터페이스에 사용할 글꼴 전환
item-import-font = 글꼴 가져오기
item-import-font-sub = TTF/OTF 글꼴 파일을 가져와 언제든 전환
font-default = 기본
font-import = 가져오기
font-switched = 글꼴이 전환되었습니다
font-switch-failed = 글꼴 전환 실패
item-fullscreen = Fullscreen Mode
item-clear-cache = Clear Cache
item-cache-size-loading = Loading…
item-cache-size = Cache size: { $size }
item-clear-cache-btn = Clear
item-cache-cleared = Cache cleared
item-preferred-sample-rate = Preferred Sample Rate
preferred-sample-rate-default = System Default
item-audio-buffer-size = Audio Buffer Size
item-show-fps = Show FPS
item-show-fps-sub = Show realtime FPS while playing
item-show-avg-fps = Show AVG FPS
item-show-avg-fps-sub = Display the average FPS on the results screen.
item-ap-fc-indicator = AP/FC Indicator
item-ap-fc-indicator-sub = Use line color to indicate AP/FC status.
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
custom-home-bgm = Home BGM
custom-reset-home-bgm = Reset Home BGM
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

migrate-settings-item = 이전 데이터 동기화
migrate-settings-item-sub = 이전 버전 PhirLie / Phira-Vrenxz의 data 폴더에서 가져오기(자동 안내는 최초 동기화 전에만 표시됨)
migrate-settings-now = 지금 동기화

# Character customization (Phira-Vrenxz)
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
custom-section-character = Character Stand
custom-character = Current Character
custom-character-intro = Character Intro
custom-character-no-intro = No intro available
custom-character-switch = Switch Character
custom-character-switch-btn = Switch
item-hud = HUD Customization
item-hud-sub = Drag the components of the main menu, library and settings to reposition and resize them
item-hud-open = Manage
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
# Phira-Vrenxz settings extra localization (no hardcoded Chinese)
sx-logout = Log Out
sx-audio-troubleshooting = Audio Troubleshooting
sx-watch-tutorial = Watch Tutorial
sx-open-source-licenses = Open Source Licenses
sx-privacy-policy = Privacy Policy
sx-about-footer = PHIRA-VRENXZ  v{ $version }  ·  Third-party unofficial client
sx-logged-out = Logged out
sx-audio-tip = Audio issue: check your system output device and in-game volume first
sx-privacy-note = This client does not collect any personal information
sx-plugin-import-failed = Plugin import failed
sx-upload-title = Score Upload
sx-upload-body = Enable score upload?
sx-upload-warning = If enabled, your scores will be uploaded and you accept the risk of being banned.
sx-cancel = Cancel
sx-confirm-enable = Enable
sx-upload-score = Score Upload
sx-upload-score-sub = Upload scores to the server when enabled; you accept the risk of being banned
sx-lan = LAN Multiplayer
sx-lan-sub = Experimental: host/join on the same LAN, with chart transfer and ready-up
sx-enter = Enter
sx-data-transfer = Data Transfer
sx-data-transfer-sub = Export/import .pfd packages, or transfer across devices over LAN
sx-adofai = A Dance of Fire and Ice
sx-pos-tl = Top Left
sx-pos-tr = Top Right
sx-pos-bl = Bottom Left
sx-pos-br = Bottom Right
sx-builtin = Built-in
sx-custom = Custom
sx-hp-max = Custom HP Limit
sx-hp-max-sub = Used when chart HP is set to Custom
sx-hp-bad-only = Custom HP loses only on Bad/Miss
sx-hp-bad-only-sub = When off, any non-Perfect hit loses 1 HP
sx-record-replay = Record Local Replay
sx-record-replay-sub = Save the touch timeline to replays/ after playing
sx-exercise-auto = Exercise Auto Speed-Up
sx-exercise-auto-sub = Increase note speed automatically after each Exercise round
sx-exercise-step = Speed-Up per Round
sx-exercise-step-sub = Multiplier added each time auto speed-up triggers
sx-auto-retry = Auto Retry Count
sx-auto-retry-sub = Restart before the failure point after HP fail; 0 disables
sx-retry-count = { $n } times
sx-retry-rewind = Retry Rewind
sx-retry-rewind-sub = Seconds before the failure point to restart from
sx-judge-preset = Judgment Preset
sx-judge-preset-sub = Built-in / Phigros / Phira / Custom windows
sx-judge-perfect = Perfect Window
sx-judge-perfect-sub = Custom Perfect judgment range (ms)
sx-judge-good = Good Window
sx-judge-good-sub = Custom Good judgment range (ms)
sx-judge-bad = Bad Window
sx-judge-bad-sub = Custom Bad judgment range (ms)
sx-judge-late = Late Hit Compensation
sx-judge-late-sub = Milliseconds subtracted from Late errors
sx-judge-pp = Perfect+ Threshold
sx-judge-pp-sub = Perfects within this error count as Perfect+; 0 disables
sx-judge-detailed = Detailed Judgment
sx-judge-detailed-sub = Split Good into Great / Ok / Meh stats
sx-judge-great = Great Threshold
sx-judge-ok = Ok Threshold
sx-judge-meh = Meh Threshold
sx-noise-enabled = Noise Field
sx-noise-enabled-sub = Hide the chart noise field and stop it from blocking touches
sx-noise-color = Noise Color
sx-noise-color-sub = Hex color, e.g. 7A003A
sx-noise-demo = Noise Style
sx-noise-demo-sub = Phigros-style uses a noise texture; Solid uses low-poly color blocks
sx-noise-demo-phigros = Phigros Style
sx-noise-demo-solid = Solid Blocks
sx-noise-glitch = Noise Glitch
sx-noise-glitch-sub = Add glitch displacement to noise field edges
sx-noise-opacity = Noise Opacity
sx-dynamic-bg = Dynamic Background
sx-dynamic-bg-sub = Artwork-color fluid; Dynamic Brightness follows music energy
sx-off = Off
sx-static-brightness = Static Brightness
sx-dynamic-brightness = Dynamic Brightness
sx-watermark = Version Watermark
sx-watermark-sub = Show game version info in the top-left corner
sx-chart-info = Difficulty and Chart Name
sx-chart-info-sub = Show difficulty and chart name in results and gameplay
sx-replay-chart-missing = Chart for this replay was not found. Re-import it.
sx-replay-deleted = Replay deleted
sx-play = Play
sx-edit = Edit
sx-edit-name = Edit: { $name }
sx-delete-name = Delete: { $name }
sx-replay-cleared = Replays cleared
sx-replay-exported = Replays exported to export/replays
sx-note-debug = Note Debug
sx-note-debug-sub = Show line ID / time / type of notes near the current time
sx-combo-debug = Combo Text Debug
sx-combo-debug-sub = Show Combo text debug info during gameplay
sx-replay-folder = Open Replay Folder
sx-replay-folder-sub = View replays/ in File Explorer
sx-replay-clear = Clear Replays
sx-replay-clear-sub = Delete all JSON replays under replays/
sx-clear = Clear
sx-replay-export = Export Replays
sx-replay-export-sub = Copy to export/replays
sx-export = Export
sx-keydisplay-missing = Key display plugin is not installed
sx-plugins-rescanned = Rescanned plugin folder ({ $n } plugins)
sx-plugin-deleted = Plugin deleted: { $name }
sx-plugin-import = Import Plugin
sx-plugin-import-sub = Supports .zip / .zips / .pvn plugin packages; refreshes automatically
sx-select-file = Select File
sx-plugin-rescan = Rescan Plugin Folder
sx-plugin-rescan-sub = Click here after manually adding plugins to the plugins/ folder
sx-plugin-entry-error =  · Entry error
sx-toggle-on = On
sx-toggle-off = Off
sx-no-plugins = No plugins yet
sx-no-plugins-sub = Click "Import Plugin" above and choose a .zip / .zips / .pvn package
sx-home-video = Home Dynamic Background
sx-home-video-reset = Clear Dynamic Background
sx-home-video-reset-done = Home dynamic background cleared
sx-home-video-set = Home dynamic background imported
sx-plugin-imported = Imported plugin: { $name } v{ $version }
