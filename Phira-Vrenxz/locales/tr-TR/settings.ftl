
label = AYARLAR

item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
general = Genel
audio = Ses
chart = Müzikler
debug = Hata Ayıklama
about = Bilgi

item-lang = Dil
item-offline = Çevrimdışı Mod
item-offline-sub = Çevrimdışı moddayken müzik ekleyemezsiniz
item-mp = Çoklu Oyunculu
item-mp-sub = Çok oyunculu modu devreye sok
item-mp-addr = Çok oyunculu sunucu
item-mp-addr-sub = Snucu adresi, 'host:port'
item-mp-addr-invalid = Geçersiz sunucu adresi
item-lowq = Düşük kaliteli mod
item-lowq-sub = Arayüz gecikmeliyse bunu seçin
item-insecure = Emniyetsiz Mod
item-insecure-sub = Çevrimiçi özellikleri kullanamıyorsanzı bunu açın. Bağlantınızı güvensiz hâle getirir!

item-adjust = Otomatik zaman ayarlama
item-adjust-sub = Müzikle ritim haritasını senkronize etmek içim zamanı dinamik olarak ayarla
item-music = Müzik sesi
item-sfx = Ses efektleri
item-bgm = Arkaplan müziği
item-bgm-sub = Splash / Home background music
item-cali = Dengeyi ayarla

item-show-acc = Doğruluğu gerçek zamanlı göster
item-dc-pause = Durdurmak için çift dokun
item-dhint = Aynı anda gelen notaları belirt
item-dhint-sub = Çizgilere aynı anda dokunan notalar belirtilir.
item-opt = Agresif optimizasyon
item-opt-sub = Performansı artırır ama istenmeyen davranışlara sebep olabilir
item-use-keyboard = Klavye Kullan
item-use-keyboard-sub = Oyun için klavye girişini etkinleştir. Etkinleştirildiğinde skorlar yüklenemez.
item-prefer-reduced-motion = Azaltılmış Hareket Tercih Et
item-prefer-reduced-motion-sub = Animasyonları ve görsel efektleri azalt
item-speed = Hız
item-note-size = Nota boyutu

item-chart-debug = Ritim haritası hata ayıklama modu
item-chart-debug-sub = Çizgi numarasını ve oryantasyonunu göster
item-touch-debug = Dokunma hata ayıklama modu
item-touch-debug-sub = Dokunulan noktaları göster.

load-cali-failed = Ses yüklenemedi

about-content =
  Phira v{ $version }

  Phira, Phigros'tan ilham alınmış ticari olmayan topluluk odaklı bir ritim oyunudur.

  BiliBili Hesabı: @Phira官方
  QQ Loncası: r48eajexth
  Discord Sunucusu: discord.gg/gqpR3bTSsP

  Canlı güncellemeler almak ve yardım almak için QQ loncasına veya Discord sunucusuna katılmanızı öneririz.

  Personel Listesi (alfabetik sıralı)
  Geliştirme
  { $development }

  Operasyonlar
  { $operations }

  Dokümantasyon
  { $documentation }

  Sanat
  { $art }

  Müzik
  { $music }

  Ses
  { $audio }

  Topluluk Yönetimi
  { $community }

  Yerelleştirme
  { $localization }

  Ve daha birçok gönüllü chart incelemecisi. Tam liste için https://phira.moe/staff adresine bakın

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
item-home-bg = Yerleşik Ana Ekran Arka Planı
item-home-bg-default = Varsayılan
item-home-bg-one = Arka Plan 1
item-home-bg-two = Arka Plan 2
item-home-bg-three = Arka Plan 3
item-home-bg-old = Eski Arka Plan
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending

custom-section-font = Yazı Tipi
item-font = Yazı Tipi
item-font-sub = Arayüzde kullanılan yazı tipini değiştir
item-import-font = Yazı tipi içe aktar
item-import-font-sub = Bir TTF/OTF yazı tipi dosyası içe aktarın ve istediğiniz zaman geçiş yapın
font-default = Varsayılan
font-import = İçe Aktar
font-switched = Yazı tipi değiştirildi
font-switch-failed = Yazı tipi değiştirilemedi
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

migrate-settings-item = Eski verileri senkronize et
migrate-settings-item-sub = Eski bir PhirLie / Phira-Vrenxz sürümünün data klasöründen içe aktar (otomatik bildirim yalnızca ilk senkronizasyondan önce görünür)
migrate-settings-now = Şimdi senkronize et

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
