
label = SETTINGS

item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
general = General
audio = Audio
chart = Chart
debug = Debug
about = Info

item-lang = Language
item-fullscreen = Fullscreen Mode
item-offline = Offline Mode
item-offline-sub = Disable all online functionality.
item-server-status = Server Status
item-server-status-sub = Open the server status page in your browser.
check-status = Open
item-mp = Multiplayer
item-mp-sub = Enable multiplayer functionality.
item-mp-addr = Multiplayer Server
item-mp-addr-sub = Connect to a custom multiplayer server.
item-mp-addr-invalid = Invalid server address.
item-lowq = Low Resolution Mode
item-lowq-sub = Lower the quality of the UI, increasing peformance.
item-clear-cache = Clear Cache
item-cache-size-loading = Loading…
item-cache-size = Cache size: { $size }
item-clear-cache-btn = Clear
item-cache-cleared = Cache cleared
item-insecure = Insecure Connection
item-insecure-sub = Enable old devices to use online functionality.
item-enable-anys = Enable Anys
item-enable-anys-sub = Use an Anys gateway to improve network stability.
item-anys-gateway = Anys Gateway
item-anys-gateway-sub = Use a custom Anys gateway address.
item-anys-gateway-invalid = Invalid gateway address.

item-adjust = Automatic Time Adjustment
item-adjust-sub = Adjust the audio and chart offset dynamically.
item-music = Music Volume
item-sfx = SFX Volume
item-bgm = BGM Volume
item-bgm-sub = Splash / Home background music
item-cali = Adjust Offset
item-preferred-sample-rate = Preferred Sample Rate
preferred-sample-rate-default = System Default
item-audio-buffer-size = Audio Buffer Size

item-show-acc = Real-Time Accuracy
item-show-fps = Show FPS
item-show-fps-sub = Show realtime FPS while playing

item-show-avg-fps = Show AVG FPS
item-show-avg-fps-sub = Display the average FPS on the results screen.
item-ap-fc-indicator = AP/FC Indicator
item-ap-fc-indicator-sub = Use line color to indicate AP/FC status.
item-dc-pause = Double-Tap to Pause
item-dhint = Simultaneous Hint
item-dhint-sub = Highlight notes that are meant to be hit at the same time.
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
item-opt = Chart Optimization
item-opt-sub = Significantly increase peformance while playing. (If unintended behavior arises, disable this.)
item-use-keyboard = Use Keyboard
item-use-keyboard-sub = Enable keyboard input for gameplay. Scores cannot be uploaded when enabled.
item-prefer-reduced-motion = Prefer Reduced Motion
item-prefer-reduced-motion-sub = Reduce animations and visual effects
item-startup-screen = Show Startup Screen
item-startup-screen-sub = Show the startup screen before the main menu (includes language selection)
item-speed = Speed
item-note-size = Note Size

item-chart-debug = Show Line ID
item-chart-debug-sub = Display the IDs and orientation of lines.
item-touch-debug = Show Touch Points
item-touch-debug-sub = Display user touch points.

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

load-cali-failed = Failed to load calibration audio.

about-content =
  Phira v{ $version }

  Phira is a non-commercial community-driven rhythm game inspired by Phigros.

  BiliBili Account: @Phira官方
  QQ Guild: r48eajexth
  Discord Server: discord.gg/gqpR3bTSsP

  We recommend joining either the QQ guild or the Discord server to get live updates and receive assistance.

  Staff List (sorted lexicographically)
  Development
  { $development }

  Operations
  { $operations }

  Documentation
  { $documentation }

  Art
  { $art }

  Music
  { $music }

  Audio
  { $audio }

  Community Management
  { $community }

  Localization
  { $localization }

  And many more voluntary chart reviewers. For a full list please refer to https://phira.moe/staff .

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
settings-console-log = Console Log
settings-console-log-desc = Show cmd console window for debug logs when enabled
settings-vsync = VSync
settings-vsync-desc = Limit FPS to monitor refresh rate when enabled, disable for higher FPS
settings-open = Open

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
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
not-logged-in = Not Logged In
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

item-indicator-enabled = Accuracy Indicator
item-indicator-enabled-sub = Show the timing error bar at the bottom center.
item-indicator-delay = Accuracy Indicator Delay
item-indicator-delay-sub = A Dance of Fire and Ice: ±220ms; Phigros: ±80ms Perfect / ±160ms Good
item-indicator-color = Indicator Color
item-indicator-color-sub = A Dance of Fire and Ice: fixed built-in colors; Phira: use skin YML
item-fps-pos = FPS Position
item-fps-pos-sub = Top left / Top right / Bottom left / Bottom right
item-fps-limit = FPS Limit
item-fps-limit-sub = 0 = unlimited; 60/120/144 recommended

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
sx-account-switch = Switch account
sx-account-add = Sign in to another account
sx-account-current = Current
sx-account-switching = Switching account…
sx-account-switched = Account switched
sx-account-switch-failed = Failed to switch account
