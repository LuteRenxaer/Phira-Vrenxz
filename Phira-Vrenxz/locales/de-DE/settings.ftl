
label = EINSTELLUNGEN
item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
general = Allgemein
audio = Audio
chart = Level
debug = Debug
about = Info
item-lang = Sprache
item-offline = Offline Modus
item-offline-sub = Alle Online-Funktionen deaktivieren
item-server-status = Serverstatus
item-server-status-sub = Öffne die Serverstatus-Seite im Browser
check-status = Öffnen
item-mp = Mehrspieler
item-mp-sub = Mehrspielerfunktion aktivieren
item-mp-addr = Mehrspieler-Server
item-mp-addr-sub = Mit einem eigenen Mehrspieler-Server verbinden
item-mp-addr-invalid = Ungültige Serveradresse
item-lowq = Niedrige Auflösung
item-lowq-sub = Reduziert die UI-Qualität für bessere Performance
item-clear-cache = Cache leeren
item-cache-size-loading = Lädt…
item-cache-size = Cache-Größe: { $size }
item-clear-cache-btn = Leeren
item-cache-cleared = Cache geleert
item-insecure = Unsichere Verbindung
item-insecure-sub = Alte Geräte dürfen Online-Funktionen verwenden!
item-enable-anys = Anys aktivieren
item-enable-anys-sub = Nutze ein Anys-Gateway für stabilere Netzwerkverbindungen
item-anys-gateway = Anys-Gateway
item-anys-gateway-sub = Eigene Anys-Gateway-Adresse verwenden
item-anys-gateway-invalid = Ungültige Gateway-Adresse
item-adjust = Automatische Zeitkorrektur
item-adjust-sub = Audio- und Level-Offset dynamisch anpassen
item-music = Musiklautstärke
item-sfx = SFX-Lautstärke
item-bgm = Hintergrundmusik-Lautstärke
item-bgm-sub = Splash / Home background music
item-cali = Offset anpassen
item-show-acc = Genauigkeit in Echtzeit
item-dc-pause = Doppeltippen zum Pausieren
item-dhint = Gleichzeitiger Hinweis
item-dhint-sub = Hebt Noten hervor, die gleichzeitig getroffen werden müssen
item-opt = Level-Optimierung
item-opt-sub = Spiel-Performance stark verbessern. (Bei unerwartetem Verhalten deaktivieren.)
item-use-keyboard = Tastatur verwenden
item-use-keyboard-sub = Tastatureingabe für das Spiel aktivieren. Ergebnisse können bei Aktivierung nicht hochgeladen werden
item-prefer-reduced-motion = Reduzierte Bewegung bevorzugen
item-prefer-reduced-motion-sub = Animationen und visuelle Effekte reduzieren
item-speed = Geschwindigkeit
item-note-size = Notengröße
item-chart-debug = Linien-ID anzeigen
item-chart-debug-sub = IDs und Ausrichtung der Linien anzeigen
item-touch-debug = Berührungspunkte anzeigen
item-touch-debug-sub = Zeigt die Nutzer-Touchpunkte an
load-cali-failed = Kalibrierungs-Audio konnte nicht geladen werden
about-content =
    Phira v{ $version }

    Phira ist ein nicht-kommerzielles, von der Community getragenes Rhythmusspiel, dessen Gameplay auf Phigros basiert und das mit Rust entwickelt wurde.

    Bilibili-Konto: @Phira官方
    QQ-Kanal: r48eajexth

    Wir empfehlen, entweder der QQ-Gilde oder dem Discord-Server beizutreten, um Live-Updates zu erhalten und Unterstützung zu bekommen!

    Teamliste (alphabetische Reihenfolge)
    Entwicklung
    { $development }

    Betrieb
    { $operations }

    Dokumentation
    { $documentation }

    Grafik
    { $art }

    Musik
    { $music }

    Soundeffekte
    { $audio }

    Community-Management
    { $community }

    Lokalisierung
    { $localization }

    Sowie viele freiwillige Level-Prüfer! Die vollständige Liste findest du unter https://phira.moe/staff
item-fullscreen = Vollbildmodus
item-preferred-sample-rate = Bevorzugte Abtastrate
preferred-sample-rate-default = Systemstandard
item-audio-buffer-size = Audio-Puffergröße
item-show-avg-fps = Durchschnittliche Bildrate anzeigen
item-show-avg-fps-sub = Zeige die durchschnittliche Bildrate auf dem Ergebnisbildschirm an
item-ap-fc-indicator = AP/FC-Anzeige (All Perfect / Full Combo)
item-ap-fc-indicator-sub = Verwende die Farbe der Bewertungslinie, um den Status „Full AP/FC“ anzuzeigen

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
item-home-bg = Integrierter Home-Hintergrund
item-home-bg-default = Standard
item-home-bg-one = Hintergrund 1
item-home-bg-two = Hintergrund 2
item-home-bg-three = Hintergrund 3
item-home-bg-old = Alter Hintergrund
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending

custom-section-font = Schriftart
item-font = Schriftart
item-font-sub = Schriftart für die Benutzeroberfläche wechseln
item-import-font = Schriftart importieren
item-import-font-sub = Eine TTF/OTF-Schriftdatei importieren und jederzeit wechseln
font-default = Standard
font-import = Importieren
font-switched = Schriftart gewechselt
font-switch-failed = Schriftartwechsel fehlgeschlagen
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

migrate-settings-item = Alte Daten synchronisieren
migrate-settings-item-sub = Aus dem data-Ordner einer älteren PhirLie / Phira-Vrenxz-Version importieren (die automatische Meldung erscheint nur vor der ersten Synchronisierung)
migrate-settings-now = Jetzt synchronisieren

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
