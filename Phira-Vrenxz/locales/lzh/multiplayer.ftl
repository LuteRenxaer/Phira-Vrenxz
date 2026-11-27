
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
server-welcome = 歡迎至 Phira-V 伺服
join-room-password-title = 輸入房之密語
room-list = 公開之房
room-list-title = 公開之房
room-list-loading = Loading…
room-list-empty = 無可入之房。
room-list-failed = 房之錄載敗。
room-list-more = …more rooms available, enter room ID directly.
room-locked-tag = 🔒
set-password = 設房之密語
set-password-failed = 設房之密語敗。
clear-password = 除房之密語
kick-user = 逐客（編號）
kick-user-failed = 逐客敗。
kick-user-invalid-id = 編號無效（見玩家錄中 #編號）。
transfer-host = 易主（編號）
transfer-host-failed = 易主敗。
transfer-host-invalid-id = 編號無效（見玩家錄中 #編號）。
preview = 預覽譜面
preview-failed = 預覽敗。
preview-unavailable = 無可預覽之譜。
preview-interrupted-title = 房主將啓戲矣！
preview-interrupted-content = 房主將啓戲矣！欲入乎？
preview-ready = Ready
preview-not-now = 且緩
preview-started-title = 房主已啓戲
preview-started-content = 卿方預覽之際，房主已啓此局。下局可再同戲。
preview-started-ok = Got it
msg-kicked-me = 卿已被房主逐出此房。
msg-kicked = `{ $user }` 已被房主逐出此房。
msg-select-local-chart = `{ $user }` 擇本地譜面：{ $chart }
msg-send-chart = `{ $user }` 始共譜面
msg-download-ready = `{ $user }` 已下載譜面
msg-player-paused = `{ $user }` 已暫其戲
msg-player-resumed = `{ $user }` 已續其戲
msg-room-results = 此局已畢：玩家 { $n } 人。
mp-server-no-local-chart = 此伺服器不容本地譜面
mp-syncing-chart = 正在同步譜面…
mp-sync-failed = 譜面同步敗：{ $err }
mp-connect-hint = 連伺服器以與友同戲
mp-connecting = Connecting…
mp-server = 伺服：{ $addr }
mp-back = Back
mp-refresh = 刷新
mp-manage-hint = 按玩家之行以理其人
mp-lobby-connected = 已連伺服器
mp-lobby-not-room = 尚未入房
mp-room-tag = 房 #{ $id }
mp-player-count = 玩家（{ $n }）
mp-you = You
mp-ready-tag = Ready
mp-host = Host
mp-state-choose = Choosing chart…
mp-state-chosen = 已擇譜 #{ $id }
mp-state-local = 共本地譜面
mp-state-wait = 待始
mp-state-playing = 戲中
mp-locked-tag = Locked
mp-cycle-tag = 循環之式
mp-n-players = 玩家 { $n } 人
mp-chat-caption = 閒談與誌
mp-chart-label = Chart
mp-msg-none = 尚無消息
mp-manage-title = 理 { $name }
mp-manage-transfer = 使之為主
mp-manage-kick = 逐出
mp-manage-cancel = Cancel
mp-room-locked = 🔒 Password
mp-library = 曲庫
mp-chat-title = Chat
mp-log-title = Log

