
multiplayer = 多人同戲

connect = 連接
connect-must-login = 登入後方可入多人同戲
connect-success = 連接功矣
connect-failed = 連接敗矣
connect-authenticate-failed = 鑑權敗矣
reconnect = 斷線重連中…

create-room = 創室
create-room-success = 室已創
create-room-failed = 創室敗矣
create-invalid-id = 室號由不多於二十個大小寫英文字母、數字及 -_ 組成

join-room = 入室
join-room-invalid-id = 無效之室號
join-room-failed = 入室敗矣

leave-room = 離室
leave-room-failed = 離室敗矣

disconnect = 斷連

request-start = 始戲
request-start-no-chart = 卿尚未擇譜
request-start-failed = 始戲敗矣

user-list = 用戶之表

lock-room = { $current ->
  [true] 解室之鎖
  *[other] 鎖室
}
cycle-room = { $current ->
  [true] 循環之態
  *[other] 常態
}

ready = 備
ready-failed = 備敗矣

cancel-ready = 取消

room-id = 室號：{ $id }

download-failed = 下譜敗矣

lock-room-failed = 鎖室敗矣
cycle-room-failed = 更室態敗矣

chat-placeholder = 言些什麼…
chat-send = 發
chat-empty = 消息不可空
chat-sent = 已發
chat-send-failed = 消息發送敗矣

select-chart-host-only = 唯室主可擇譜
select-chart-local = 不可擇本地之譜
select-chart-failed = 擇譜敗矣
select-chart-not-now = 卿今不可擇譜

msg-create-room = 「{ $user }」創室
msg-join-room = 「{ $user }」入室
msg-leave-room = 「{ $user }」離室
msg-new-host = 「{ $user }」爲新室主
msg-select-chart = 室主「{ $user }」擇譜「{ $chart }」(#{ $id })
msg-game-start = 室主「{ $user }」始戲，請他玩家備
msg-ready = 「{ $user }」已備
msg-cancel-ready = 「{ $user }」取消備
msg-cancel-game = 「{ $user }」取消戲
msg-start-playing = 戲始
msg-played = 「{ $user }」戲畢：{ $score } ({ $accuracy }){ $full-combo ->
  [true] ，全連
  *[other] {""}
}
msg-game-end = 戲終
msg-abort = 「{ $user }」棄戲
msg-room-lock = { $lock ->
  [true] 室已鎖
  *[other] 室已解鎖
}
msg-room-cycle = { $cycle ->
  [true] 室已更爲循環之態
  *[other] 室已更爲常態
}
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
mp-chart-label = Chart
mp-msg-none = No messages yet
mp-manage-title = Manage { $name }
mp-manage-transfer = Make host
mp-manage-kick = Kick out
mp-manage-cancel = Cancel
mp-room-locked = 🔒 Password
mp-library = Chart Library
