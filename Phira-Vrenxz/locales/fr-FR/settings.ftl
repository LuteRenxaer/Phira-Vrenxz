
label = PARAMÈTRES

item-ui-style = UI Style
item-ui-style-sub = Switch the overall UI look; the default style matches previous versions
item-ui-style-default = Default
item-ui-style-xcsim = XC-SIM
general = Général
audio = Audio
chart = Partition
debug = Déboguage
about = Informations

item-lang = Langue
item-offline = Mode hors-ligne
item-offline-sub = Vous ne pourrez pas publier vos notes en mode hors-ligne
item-server-status = État du serveur
item-server-status-sub = Accéder à la page Web pour vérifier l'état du serveur
check-status = Vérifier
item-mp = Multijoueur
item-mp-sub = Activer le mode multijoueur
item-mp-addr = Serveur multijoueur
item-mp-addr-sub = Adresse du serveur, «hôte:port»
item-mp-addr-invalid = Adresse du serveur invalide
item-lowq = Mode basse qualité
item-lowq-sub = Réduire la qualité de l'IU pour augmentant les performances.
item-clear-cache = Vider le cache
item-cache-size-loading = Chargement...
item-cache-size = Taille du cache : { $size }
item-clear-cache-btn = Vider
item-cache-cleared = Cache vidé
item-insecure = Connexion non sécurisée
item-insecure-sub = Permettre aux anciens appareils d'utiliser les fonctionnalités en ligne.
item-enable-anys = Activer Anys
item-enable-anys-sub = Utiliser une passerelle Anys pour améliorer la stabilité du réseau.
item-anys-gateway = Passerelle Anys
item-anys-gateway-sub = Utiliser une adresse de passerelle Anys personnalisée.
item-anys-gateway-invalid = Adresse de passerelle invalide.

item-adjust = Ajustement automatique du temps
item-adjust-sub = Cela ajuste le temps dynamiquement pour synchroniser la musique et la partition.
item-music = Volume de la musique
item-sfx = Volume des effets sonores
item-bgm = Volume de la musique de fond
item-bgm-sub = Splash / Home background music
item-cali = Ajuster le décalage

item-show-acc = Afficher la précision en temps réel
item-dc-pause = Double-appui pour mettre en pause
item-dhint = Indication d'appui simultané
item-dhint-sub = Les notes qui touchent la ligne simultanément seront mises en surbrillance.
item-opt = Optimisation agressive
item-opt-sub = Améliore les performances, mais peux provoquer des comportements incorrects
item-use-keyboard = Utiliser le clavier
item-use-keyboard-sub = Activer l'entrée clavier pour le jeu. Les scores ne peuvent pas être téléchargés si activé.
item-prefer-reduced-motion = Préférer mouvement réduit
item-prefer-reduced-motion-sub = Réduire les animations et les effets visuels
item-speed = Vitesse
item-note-size = Taille des notes

item-chart-debug = Mode de débogage des partitions
item-chart-debug-sub = Afficher le numéro et l'orientation des lignes de jugement
item-touch-debug = Mode de débogage tactile
item-touch-debug-sub = Afficher les points de contact

load-cali-failed = Échec du chargement de l'audio

about-content =
  Phira v{ $version }

  Phira est un jeu de rythme communautaire et non commercial, inspiré par Phigros.

  Compte BiliBili : @Phira官方
  Chaîne QQ : r48eajexth
  Serveur Discord : discord.gg/gqpR3bTSsP

  Nous vous recommandons de rejoindre la chaîne QQ ou le serveur Discord pour obtenir des mises à jour en direct et recevoir de l'aide.

  Liste du personnel (triés par ordre alphabétique)
  Développement
  { $development }

  Opérations
  { $operations }

  Documentation
  { $documentation }

  Art
  { $art }

  Musique
  { $music }

  Audio
  { $audio }

  Gestion de la communauté
  { $community }

  Localisation
  { $localization }

  Et beaucoup d'autres réviseurs de charts bénévoles. Pour une liste complète, veuillez consulter https://phira.moe/staff

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
item-home-bg = Fond d'accueil intégré
item-home-bg-default = Par défaut
item-home-bg-one = Fond 1
item-home-bg-two = Fond 2
item-home-bg-three = Fond 3
item-home-bg-old = Ancien fond
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending

custom-section-font = Police
item-font = Police
item-font-sub = Changer la police utilisée par l’interface
item-import-font = Importer une police
item-import-font-sub = Importer un fichier de police TTF/OTF et y basculer à tout moment
font-default = Par défaut
font-import = Importer
font-switched = Police changée
font-switch-failed = Échec du changement de police
item-fullscreen = Fullscreen Mode
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

migrate-settings-item = Synchroniser les données anciennes
migrate-settings-item-sub = Importer depuis le dossier data d’une ancienne version de PhirLie / Phira-Vrenxz (l’invite automatique n’apparaît qu’avant la première synchronisation)
migrate-settings-now = Synchroniser maintenant

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
