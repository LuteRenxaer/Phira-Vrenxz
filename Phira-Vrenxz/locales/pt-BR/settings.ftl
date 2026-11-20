
label = CONFIGURAÇÕES

item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
general = Geral
audio = Audio
chart = Beatmap
debug = Depurar
about = Informações

item-lang = Linguagem
item-offline = Modo offline
item-offline-sub = Você não pode fazer upload de recordes reproduzidos no modo off-line
item-mp = Multijogador
item-mp-sub = Ativar o modo multijogador
item-mp-addr = Servidor multijogador
item-mp-addr-sub = Endereço do servidor, 'host:porta'
item-mp-addr-invalid = Endereço de servidor inválido
item-lowq = Modo de baixa qualidade
item-lowq-sub = Habilite isto se a UI estiver lenta
item-insecure = Modo inseguro
item-insecure-sub = Habilite isto se você não puder usar funcionalidades online. Torna sua conexão insegura!

item-adjust = Ajuste automático de tempo
item-adjust-sub = Ajusta o tempo dinamicamente para sincronizar a música e o beatmap
item-music = Volume da música
item-sfx = Volume do efeito sonoro
item-bgm = Volume da música de fundo
item-bgm-sub = Splash / Home background music
item-cali = Ajustar deslocamento

item-show-acc = Mostre precisão em tempo real
item-dc-pause = Toque duas vezes para pausar
item-dhint = Destaque de notas simultâneas
item-dhint-sub = Notas que tocam a linha simultaneamente será destacada
item-opt = Otimização agressiva
item-opt-sub = Melhora o desempenho, mas pode causar comportamento incorreto
item-use-keyboard = Usar Teclado
item-use-keyboard-sub = Ativar entrada de teclado para jogar. Pontuações não podem ser enviadas quando ativado.
item-prefer-reduced-motion = Preferir movimento reduzido
item-prefer-reduced-motion-sub = Reduzir animações e efeitos visuais
item-speed = Velocidade
item-note-size = Tamanho da nota

item-chart-debug = Modo de depuração de Beatmap
item-chart-debug-sub = Exibir número de linha e orientação
item-touch-debug = Mostrar toques na tela
item-touch-debug-sub = Irá exibir toques na tela

load-cali-failed = Falha ao carregar áudio

about-content =
  Phira v{ $version }

  Phira é um jogo de ritmo não comercial voltado para a comunidade, inspirado em Phigros.

  Conta BiliBili: @Phira官方
  Guilda QQ: r48eajexth
  Servidor Discord: discord.gg/gqpR3bTSsP

  Recomendamos entrar na guilda QQ ou no servidor Discord para obter atualizações ao vivo e receber assistência.

  Lista de Funcionários
  Desenvolvimento
  { $development }

  Operações
  { $operations }

  Documentação
  { $documentation }

  Arte
  { $art }

  Música
  { $music }

  Áudio
  { $audio }

  Gestão da Comunidade
  { $community }

  Localização
  { $localization }

  E muitos outros revisores de charts voluntários. Para uma lista completa, consulte https://phira.moe/staff

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
item-home-bg = Plano de fundo interno da tela inicial
item-home-bg-default = Padrão
item-home-bg-one = Fundo 1
item-home-bg-two = Fundo 2
item-home-bg-three = Fundo 3
item-home-bg-old = Fundo antigo
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending

custom-section-font = Fonte
item-font = Fonte
item-font-sub = Alterar a fonte usada pela interface
item-import-font = Importar fonte
item-import-font-sub = Importe um arquivo de fonte TTF/OTF e alterne a qualquer momento
font-default = Padrão
font-import = Importar
font-switched = Fonte alterada
font-switch-failed = Falha ao alterar a fonte
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

migrate-settings-item = Sincronizar dados antigos
migrate-settings-item-sub = Importar da pasta data de uma versão antiga do PhirLie / Phira-Vrenxz (o aviso automático aparece apenas antes da primeira sincronização)
migrate-settings-now = Sincronizar agora

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
