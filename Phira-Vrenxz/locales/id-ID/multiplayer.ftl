
multiplayer = Multiplayer

connect = Hubungkan
connect-must-login = Anda harus login untuk masuk ke mode multiplayer
connect-success = Berhasil terhubung
connect-failed = Gagal terhubung
connect-authenticate-failed = Otorisasi gagal
reconnect = Menghubungkan...

create-room = Buat room
create-room-success = Room dibuat
create-room-failed = Gagal membuat room
create-invalid-id = ID room terdiri dari tidak lebih dari 20 karakter, termasuk huruf, angka, - (tanda hubung) dan _ (garis bawah)

join-room = Bergabung ke Room
join-room-invalid-id = ID room tidak valid
join-room-failed = Gagal bergabung ke dalam room

leave-room = Meninggalkan Room
leave-room-failed = Gagal meninggalkan room

disconnect = Memutuskan

request-start = Mulai permainan
request-start-no-chart = Anda belum memilih chart
request-start-failed = Gagal memulai permainan

user-list = Pemain

lock-room = { $current ->
  [true] Room terbuka
  *[other] Room terkunci
}
cycle-room = { $current ->
  [true] Mode cycling
  *[other] Mode normal
}

ready = Siap
ready-failed = Gagal bersiap

cancel-ready = Batal

room-id = Room ID: { $id }

download-failed = Gagal mengunduh chart

lock-room-failed = Gagal mengunci room
cycle-room-failed = Gagal mengubah mode room

chat-placeholder = Katakan sesuatu...
chat-send = Kirim
chat-empty = Pesan kosong
chat-sent = Terkirim
chat-send-failed = Gagal mengirim pesan

select-chart-host-only = Hanya host yang dapat memilih chart
select-chart-local = Tidak dapat memilih chart lokal
select-chart-failed = Gagal untuk memilih chart
select-chart-not-now = Anda tidak dapat memilih chart sekarang

msg-create-room = `{ $user }` membuat room
msg-join-room = `{ $user }` bergabung ke dalam room
msg-leave-room = `{ $user }` keluar dari room
msg-new-host = `{ $user }` menjadi host baru
msg-select-chart = Host `{ $user }`memilih chart `{ $chart }` (#{ $id })
msg-game-start = Host `{ $user }` memulai permainan. Pemain lain harus segera bersiap.
msg-ready = `{ $user }` siap
msg-cancel-ready = `{ $user }` membatalkan siap
msg-cancel-game = `{ $user }` membatalkan permainan
msg-start-playing = Permainan dimulai
msg-played = `{ $user }` selesai bermain: { $score } ({ $accuracy }){ $full-combo ->
  [true] , full combo
  *[other] {""}
}
msg-game-end = Permainan berakhir
msg-abort = `{ $user }` membatalkan permainan
msg-room-lock = { $lock ->
  [true] Room terkunci
  *[other] Room terbuka
}
msg-room-cycle = { $cycle ->
  [true] Room berubah menjadi mode cycling
  *[other] Room berubah menjadi mode normal
}
mp-library = Charts
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
mp-chat-title = Chat
mp-log-title = Log

