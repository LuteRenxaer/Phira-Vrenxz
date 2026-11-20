multiplayer = Multiplayer
connect = Холбогдох
connect-must-login = Олон тоглогчийн горимд орохын тулд та нэвтэрсэн байх ёстой
connect-success = Амжилттай холбогдсон
connect-failed = Холбогдоход амжилтгүй боллоо
connect-authenticate-failed = Зөвшөөрөл олгогдоход амжилтгүй боллоо
reconnect = Дахин холбогдож байна...
create-room = Өрөө үүсгэх
create-room-success = Өрөө үүсгэгдсэн
create-room-failed = Өрөө үүсгэхэд амжилтгүй болов
create-invalid-id = Өрөөний ID нь үсэг, тоо, - (зураас), _ (доогуур зураас) зэргийг багтаасан 20-с илүүгүй тэмдэгтээс бүрдэнэ
join-room = Өрөө рүү орох
join-room-invalid-id = Буруу өрөөний ID
join-room-failed = Өрөө рүү ороход амжилтгүй боллоо
leave-room = Өрөөнөөс гарах
leave-room-failed = Өрөөнөөс гарахад амжилтгүй боллоо
disconnect = Таслах
request-start = Тоглоом Эхлүүлэх
request-start-no-chart = Та бийтмап сонгоогүй байна
request-start-failed = Тоглоом эхлүүлэхэд амжилтгүй боллоо
user-list = Хэрэглэгчид
lock-room =
    { $current ->
        [true] Өрөөг онгойлгох
       *[other] Өрөөг цоожлох
    }
cycle-room =
    { $current ->
        [true] Дугуйн горим
       *[other] Энгийн горим
    }
ready = Бэлэн болох
ready-failed = Бэлэн болоход амжилтгүй боллоо
cancel-ready = Цуцлах
room-id = Өрөөний ID: { $id }
download-failed = Бийтмапыг татахад амжилтгүй боллоо
lock-room-failed = Өрөөг цоожлоход амжилтгүй боллоо
cycle-room-failed = Өрөөний горимыг өөрчлөхөд амжилтгүй боллоо
chat-placeholder = Ямар нэг юм хэлээрэй...
chat-send = Илгээх
chat-empty = Мессеж хоосон байна
chat-sent = Илгээгдсэн
chat-send-failed = Мессеж илгээхэд амжилтгүй боллоо
select-chart-host-only = Зөвхөн өрөөний эзэн бийтмапуудыг сонгоно
select-chart-local = Дотоод бийтмапийг сонгож чадахгүй
select-chart-failed = Бийтмапыг сонгоход амжилтгүй боллоо
select-chart-not-now = Та яг одоо бийтмапыг сонгох боломжгүй
msg-create-room = `{ $user }` өрөөг үүсгэв
msg-join-room = `{ $user }` өрөө рүү орлоо
msg-leave-room = `{ $user }` өрөөнөөс гарлаа
msg-new-host = `{ $user }` шинэ өрөөний эзэн боллоо
msg-select-chart = Өрөөний эзэн `{ $user }` бийтмап `{ $chart }` (#{ $id }) сонголоо
msg-game-start = Өрөөний эзэн `{ $user }` тоглоомоо эхлүүлэв. Бусад тоглогчид бэлэн болоорой.
msg-ready = `{ $user }` бэлэн болов
msg-cancel-ready = `{ $user }` бэлэн болохоо болиулав
msg-cancel-game = `{ $user }` тоглоомоо болиуллаа
msg-start-playing = Game Start
msg-played =
    `{ $user }` тоглоод дууссан: { $score } ({ $accuracy }){ $full-combo ->
        [true] , full combo
       *[other] { "" }
    }
msg-game-end = Тоглоом дууслаа
msg-abort = `{ $user }` тоглоомоо зогсоолоо
msg-room-lock =
    { $lock ->
        [true] Өрөө цоожлогдсон
       *[other] Өрөө онгойлгогдсон
    }
msg-room-cycle =
    { $cycle ->
        [true] Өрөө дугуйн горим руу өөрчлөгдөв
       *[other] Өрөө энгийн горим руу өөрчлөгдөв
    }
mp-library = Чартууд
mp-chart-label = Чарт

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
