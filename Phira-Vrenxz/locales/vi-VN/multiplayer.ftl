
multiplayer = Nhiều người chơi

connect = Kết nối
connect-must-login = Bạn phải đăng nhập để sử dụng tính năng này
connect-success = Đã kết nối
connect-failed = Không thể kết nối
connect-authenticate-failed = Uỷ quyền thất bại
reconnect = Đang kết nối lại...

create-room = Tạo phòng
create-room-success = Đã tạo phòng
create-room-failed = Không thể tạo phòng
create-invalid-id = ID phòng phải dưới 20 ký tự, có thể chứa chữ, số, - và _

join-room = Vào phòng
join-room-invalid-id = Không tìm thấy ID phòng
join-room-failed = Không thể vào phòng

leave-room = Rời phòng
leave-room-failed = Không thể rời phòng

disconnect = Ngắt kết nối

request-start = Bắt đầu
request-start-no-chart = Bạn chưa chọn Chart
request-start-failed = Không thể bắt đầu

user-list = Người chơi

lock-room = { $current ->
  [true] Mở phòng
  *[other] Khoá phòng
}
cycle-room = { $current ->
  [true] Vòng lặp
  *[other] Bình thường
}

ready = Sẵn sàng
ready-failed = Không thể sẵn sàng

cancel-ready = Huỷ

room-id = ID phòng: { $id }

download-failed = Không thể tải về Chart

lock-room-failed = Không thể khoá phòng
cycle-room-failed = Không thể thay đổi chế độ

chat-placeholder = Hãy nói gì đó...
chat-send = Gửi
chat-empty = Tin nhắn trống
chat-sent = Đã gửi
chat-send-failed = Không thể gửi tin nhắn

select-chart-host-only = Chỉ chủ phòng mới được chọn Chart
select-chart-local = Không thể chọn Chart đã nhập
select-chart-failed = Không thể chọn Chart
select-chart-not-now = Bạn không thể chọn Chart lúc này

msg-create-room = `{ $user }` đã tạo phòng
msg-join-room = `{ $user }` đã vào phòng
msg-leave-room = `{ $user }` đã rời phòng
msg-new-host = `{ $user }` trở thành chủ phòng
msg-select-chart = Chủ phòng `{ $user }` đã chọn chart `{ $chart }` (#{ $id })
msg-game-start = Chủ phòng `{ $user }` đã bắt đầu, đợi bạn sẵn sàng!
msg-ready = `{ $user }` đã sẵn sàng
msg-cancel-ready = `{ $user }` huỷ sẵn sằng
msg-cancel-game = `{ $user }` huỷ trò chơi
msg-start-playing = Trò chơi bắt đầu!
msg-played = `{ $user }` đã xong: { $score } ({ $accuracy }){ $full-combo ->
  [true] , FC
  *[other] {""}
}
msg-game-end = Trò chơi kết thúc!
msg-abort = `{ $user }` đã buộc huỷ trò chơi
msg-room-lock = { $lock ->
  [true] Đã khoá phòng
  *[other] Đã mở khoá phòng
}
msg-room-cycle = { $cycle ->
  [true] Đã chuyển phòng về chế độ vòng lặp
  *[other] Đã chuyển phòng về chế độ bình thường
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

