
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
