
multiplayer = Çok Oyunculu

connect = Bağlan
connect-must-login = Çok oyunculu moda katılmak için giriş yapmalısınız
connect-success = Bağlantı başarılı
connect-failed = Bağlanılamadı
connect-authenticate-failed = Giriş başarısız
reconnect = Yeniden bağlanılıyor...

create-room = Oda Yarat
create-room-success = Oda yaratıldı
create-room-failed = Oda yaratılamadı
create-invalid-id = Oda ID'si, 20 karakterden uzun olmayacak şekilde harf, sayı, kısa çizgi (-) ve alt çizgi (_) içerebilir

join-room = Bir Odaya Katıl
join-room-invalid-id = Geçersiz Oda ID'si
join-room-failed = Odaya katılınamadı

leave-room = Odadan Ayrıl
leave-room-failed = Odadan ayrılınamadı

disconnect = Bağlantıyı Kes

request-start = Oyuna Başla
request-start-no-chart = Bir müzik seçmediniz
request-start-failed = Oyun başlatılamadı

user-list = Kullanıcılar

lock-room = { $current ->
  [true] Odanın kilidini aç
  *[other] Odayı kilitle
}
cycle-room = { $current ->
  [true] Dönen mod
  *[other] Normal mod
}

ready = Hazır
ready-failed = Hazırlanma başarısız

cancel-ready = İptal

room-id = Oda ID'si: { $id }

download-failed = Müzik indirilemedi

lock-room-failed = Oda kilitlenemedi
cycle-room-failed = Oda modu değiştirilemedi

chat-placeholder = Bir şey yazın...
chat-send = Gönder
chat-empty = Yaprak bile kıpırdamıyor
chat-sent = Gönderildi
chat-send-failed = Mesaj gönderilemedi

select-chart-host-only = Müziği yalnızca oda sahibi seçebilir
select-chart-local = Yerel müzik seçilemiyor
select-chart-failed = Müzik seçilemedi
select-chart-not-now = Şu anda müzik seçemezsiniz

msg-create-room = `{ $user }` odayı yarattı
msg-join-room = `{ $user }` odaya katıldı
msg-leave-room = `{ $user }` odadan ayrıldı
msg-new-host = `{ $user }` yeni oda sahibi
msg-select-chart = Oda sahibi `{ $user }`, `{ $chart }` (#{ $id }) müziğini seçti
msg-game-start = Oda sahibi `{ $user }` oyunu başlatıyor. Oyuncular hazırlansın.
msg-ready = `{ $user }` hazır
msg-cancel-ready = `{ $user }` artık hazır değil
msg-cancel-game = `{ $user }` oyunu iptal etti
msg-start-playing = Oyun başladı
msg-played = `{ $user }` bitirdi: { $score } ({ $accuracy }){ $full-combo ->
  [true] , full kombo
  *[other] {""}
}
msg-game-end = Oyun bitti
msg-abort = `{ $user }` oyunu iptal etti
msg-room-lock = { $lock ->
  [true] Oda kilitlendi
  *[other] Odanın kilidi açıldı
}
msg-room-cycle = { $cycle ->
  [true] Oda dönen moda geçirildi
  *[other] Oda normal moda geçirildi
}
mp-library = Chartlar
mp-chart-label = Chart

server-welcome = Welcome Phira-V Server!
join-room-password-title = Enter room password
room-list = Public Rooms
room-list-title = Public Rooms
room-list-loading = Loading…
room-list-empty = No joinable rooms.
room-list-failed = Failed to load rooms.
room-list-more = …more rooms available, enter room ID directly.
room-locked-tag = 🔒
set-password = Set Room Password
set-password-failed = Failed to set room password.
clear-password = Clear Room Password
kick-user = Kick Player (ID)
kick-user-failed = Failed to kick player.
kick-user-invalid-id = Invalid player ID (see #ID in player list).
transfer-host = Transfer Host (ID)
transfer-host-failed = Failed to transfer host.
transfer-host-invalid-id = Invalid player ID (see #ID in player list).
preview = Preview Chart
preview-failed = Failed to preview.
preview-unavailable = No chart available to preview.
preview-interrupted-title = The host is starting the game!
preview-interrupted-content = The host is starting the game! Ready to join?
preview-ready = Ready
preview-not-now = Not now
preview-started-title = The host has started the game
preview-started-content = The host started this round while you were previewing the chart. You can play together again next round.
preview-started-ok = Got it
msg-kicked-me = You have been removed from the room by the host.
msg-kicked = `{ $user }` has been removed from the room by the host.
msg-select-local-chart = `{ $user }` selected a local chart: { $chart }
msg-send-chart = `{ $user }` started sharing the chart
msg-download-ready = `{ $user }` finished downloading the chart
msg-player-paused = `{ $user }` paused the game
msg-player-resumed = `{ $user }` resumed the game
msg-room-results = Round finished: { $n } players.
mp-server-no-local-chart = This server does not support local charts
mp-syncing-chart = Syncing chart...
mp-sync-failed = Chart sync failed: { $err }
mp-connect-hint = Connect to a server to play with friends
mp-connecting = Connecting…
mp-server = Server: { $addr }
mp-back = Back
mp-refresh = Refresh
mp-manage-hint = Tap a player row to manage that player
mp-lobby-connected = Connected to server
mp-lobby-not-room = Not in a room yet
mp-room-tag = Room #{ $id }
mp-player-count = Players ({ $n })
mp-you = You
mp-ready-tag = Ready
mp-host = Host
mp-state-choose = Choosing chart…
mp-state-chosen = Chart selected #{ $id }
mp-state-local = Sharing a local chart
mp-state-wait = Waiting to start
mp-state-playing = Playing
mp-locked-tag = Locked
mp-cycle-tag = Cycle mode
mp-n-players = { $n } players
mp-chat-caption = Chat & Log
mp-msg-none = No messages yet
mp-manage-title = Manage { $name }
mp-manage-transfer = Make host
mp-manage-kick = Kick out
mp-manage-cancel = Cancel
mp-room-locked = 🔒 Password
