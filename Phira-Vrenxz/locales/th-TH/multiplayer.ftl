
multiplayer = Multiplayer

connect = เชื่อมต่อ
connect-must-login = คุณต้อง Login เพื่อเล่น Multiplayer
connect-success = เชื่อมต่อเสร็จสิ้น
connect-failed = เชื่อมต่อล้มเหลว
connect-authenticate-failed = การขออนุญาติล้มเหลว
reconnect = เชื่อมต่ออีกครั้ง…

create-room = สร้างห้อง
create-room-success = สร้างห้องเสร็จสิ้น
create-room-failed = ไม่สามารถสร้างห้องได้
create-invalid-id = ID ห้องไม่ควรเกิน 20 ตัว ต้องมีตัวอักษร หลังจากนี้จะไม่มีก็ได้ ตัวเลข, - (เครื่องขีด) และ _ (ขีดเส้นใต้)

join-room = เข้าร่วมห้อง
join-room-invalid-id = ID ไม่ถูกต้อง
join-room-failed = ไม่สามารถเข้าร่วมได้

leave-room = ออกห้อง
leave-room-failed = ไม่สามารถออกห้องได้

disconnect = ตัดการเชื่อมต่อ

request-start = เริ่มเกม
request-start-no-chart = คุณยังไม่ได้เลือก Chart
request-start-failed = ไม่สามารถเริ่มเกมได้

user-list = Users

lock-room = { $current ->
  [true] ปลดล็อคห้อง
  *[other] ล็อคห้อง
}
cycle-room = { $current ->
  [true] โหมด วน
  *[other] โหมด ธรรมดา
}

ready = พร้อม
ready-failed = ไม่สามารถพร้อมได้

cancel-ready = ยกเลิก

room-id = ID ห้อง: { $id }

download-failed = ไม่สามารถ Download chart ได้

lock-room-failed = ไม่สามารถล็อคห้องได้
cycle-room-failed = ไม่สามารถเปลี่ยนโหมดได้

chat-placeholder = ลองพูดอะไรสักอย่าง…
chat-send = ส่ง
chat-empty = ว่างเปล่า :v
chat-sent = ส่งเรียบร้อย
chat-send-failed = ไม่สามารถส่งข้อความได้

select-chart-host-only = มีแค่ Host เท่านั้นที่เลือก Chart ได้
select-chart-local = ไม่สามารถเลือก Chart จาก Local ได้
select-chart-failed = ไม่สามารถเลือก Chart ได้
select-chart-not-now = คุณไม่สามารถเลือก Chart ได้

msg-create-room = `{ $user }` ได้สร้างห้องแล้ว.
msg-join-room = `{ $user }` ได้เข้าห้องแล้ว.
msg-leave-room = `{ $user }` ได้ออกจากห้องแล้ว.
msg-new-host = `{ $user }` ได้เป็น Host คนใหม่แล้ว.
msg-select-chart = Host `{ $user }` ได้เลือก Chart `{ $chart }` (#{ $id })
msg-game-start = Host `{ $user }` ได้เริ่มเกมแล้ว. ผู้เล่นคนอื่คนจะพร้อมได้แล้ว.
msg-ready = `{ $user }` พร้อมแล้ว.
msg-cancel-ready = `{ $user }` ยกเลิกพร้อมแล้ว.
msg-cancel-game = `{ $user }` ยกเลิกเกมแล้ว.
msg-start-playing = เกมได้เริ่มแล้ว.
msg-played = `{ $user }` เล่นเสร็จแล้ว: { $score } ({ $accuracy }){ $full-combo ->
  [true] , full combo
  *[other] {""}
}
msg-game-end = เกมได้จบลงแล้ว.
msg-abort = `{ $user }` เกมได้ยกเลิกแล้ว.
msg-room-lock = { $lock ->
  [true] ห้องถูกล็อคแล้ว.
  *[other] ห้องถูกปลดล็อคแล้ว.
}
msg-room-cycle = { $cycle ->
  [true] ห้องถูกเปลี่ยนเป็นโหมดวนแล้ว.
  *[other] ห้องได้ถูกเปลี่ยนเป็นโหมดธรรมดาแล้ว.
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

