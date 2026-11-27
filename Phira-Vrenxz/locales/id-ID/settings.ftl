
label = Settings

item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
general = Umum
audio = Suara
chart = Chart
debug = Debug
about = Tentang

item-lang = Bahasa
item-offline = Mode Offline
item-offline-sub = Anda tidak bisa mengupload skor anda di Mode Offline.
item-mp = Multiplayer
item-mp-sub = Hidupkan mode multiplayer
item-mp-addr = Multiplayer server
item-mp-addr-sub = Server address, 'host:port'
item-mp-addr-invalid = Server address tidak valid
item-lowq = Mode Kualitas Rendah
item-lowq-sub = Aktifkan ini jika UI nge-lag
item-insecure = Mode insecure
item-insecure-sub = Aktifkan ini jika Anda tidak dapat menggunakan fungsi online. Membuat koneksi Anda tidak aman!

item-adjust = Penyesuaian Waktu Otomatis
item-adjust-sub = Menyesuaikan waktu secara dinamis untuk menyinkronkan musik dan chart.
item-music = Volume musik
item-sfx = Volume efek suara
item-bgm = Volume BGM
item-bgm-sub = Splash / Home background music
item-cali = Penyesuaian offset

item-show-acc = Tunjukkan akurasi waktu nyata
item-dc-pause = Tekan dua kali untuk pause
item-dhint = Double hint
item-dhint-sub = Note-note yang perlu ditekan bersamaan akan bersinar.
item-opt = Optimizasi agresif
item-opt-sub = Meningkatkan performa, tapi akan mungkin merusak beberapa fungsi aplikasi
item-use-keyboard = Gunakan Keyboard
item-use-keyboard-sub = Aktifkan input keyboard untuk bermain. Skor tidak dapat diupload jika diaktifkan.
item-prefer-reduced-motion = Prefer Gerakan Berkurang
item-prefer-reduced-motion-sub = Kurangi animasi dan efek visual
item-speed = Kecepatan
item-note-size = Ukuran note

load-cali-failed = Gagal memuat audio

about-content =
  Phira v{ $version }

  Phira adalah game ritme berbasis komunitas non-komersial, terinspirasi oleh Phigros.

  Akun BiliBili: @Phira官方
  QQ Guild: r48eajexth
  Server Discord: discord.gg/gqpR3bTSsP

  Kami menyarankan untuk bergabung dengan QQ Guild atau server Discord untuk mendapatkan pembaruan langsung dan menerima bantuan.

  Daftar Staf (diurutkan secara leksikografis)
  Pengembangan
  { $development }

  Operasi
  { $operations }

  Dokumentasi
  { $documentation }

  Seni
  { $art }

  Musik
  { $music }

  Audio
  { $audio }

  Manajemen Komunitas
  { $community }

  Lokalisasi
  { $localization }

  Dan banyak lagi peninjau chart sukarela. Untuk daftar lengkap, silakan merujuk ke https://phira.moe/staff

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
item-home-bg = Latar Belakang Bawaan
item-home-bg-default = Bawaan
item-home-bg-one = Latar 1
item-home-bg-two = Latar 2
item-home-bg-three = Latar 3
item-home-bg-old = Latar lama
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending

custom-section-font = Font
item-font = Font
item-font-sub = Ganti font yang digunakan antarmuka
item-import-font = Impor font
item-import-font-sub = Impor file font TTF/OTF dan ganti kapan saja
font-default = Default
font-import = Impor
font-switched = Font diganti
font-switch-failed = Gagal mengganti font
item-fullscreen = Fullscreen Mode
item-server-status = Server Status
item-server-status-sub = Open the server status page in your browser.
check-status = Open
item-clear-cache = Clear Cache
item-cache-size-loading = Loading…
item-cache-size = Cache size: { $size }
item-clear-cache-btn = Clear
item-cache-cleared = Cache cleared
item-enable-anys = Enable Anys
item-enable-anys-sub = Use an Anys gateway to improve network stability.
item-anys-gateway = Anys Gateway
item-anys-gateway-sub = Use a custom Anys gateway address.
item-anys-gateway-invalid = Invalid gateway address.
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

migrate-settings-item = Sinkronisasi data lama
migrate-settings-item-sub = Impor dari folder data versi PhirLie / Phira-Vrenxz yang lebih lama (pemberitahuan otomatis hanya muncul sebelum sinkronisasi pertama)
migrate-settings-now = Sinkronisasi sekarang

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
sx-account-switch = Ganti akun
sx-account-add = Masuk dengan akun lain
sx-account-current = Saat ini
sx-account-switching = Mengganti akun…
sx-account-switched = Akun diganti
sx-account-switch-failed = Gagal mengganti akun
sx-account-hint = Ketuk akun untuk berganti, × menghapus dari daftar
