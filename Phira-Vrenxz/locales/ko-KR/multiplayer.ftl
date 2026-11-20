
multiplayer = 멀티플레이어

connect = 연결하기
connect-must-login = 멀티플레이어 모드에 들어가려면 로그인해야 합니다.
connect-success = 연결 성공
connect-failed = 연결 실패
connect-authenticate-failed = 인증 실패
reconnect = 다시 연결 중...

create-room = 방 생성
create-room-success = 방이 생성되었습니다.
create-room-failed = 방 생성 실패
create-invalid-id = 방 ID는 문자, 숫자, -(바), _(언더바)를 포함하여 20자 이하로 구성되어야 합니다.

join-room = 방 참여
join-room-invalid-id = 잘못된 방 ID
join-room-failed = 방 참여 실패

leave-room = 방 나가기
leave-room-failed = 방 나가기 실패

disconnect = 연결 종료

request-start = 게임 시작 요청
request-start-no-chart = 차트를 선택하지 않았습니다.
request-start-failed = 게임 시작 실패

user-list = 유저 목록

lock-room = { $current ->
  [true] 방 잠금 해제
  *[other] 방 잠금
}
cycle-room = { $current ->
  [true] 순환 모드
  *[other] 일반 모드
}

ready = 준비 완료
ready-failed = 준비 실패

cancel-ready = 준비 취소

room-id = 방 ID: { $id }

download-failed = 차트 다운로드 실패

lock-room-failed = 방 잠금 실패
cycle-room-failed = 방 모드 변경 실패

chat-placeholder = 메시지 입력...
chat-send = 보내기
chat-empty = 메시지가 비어 있습니다.
chat-sent = 보냄
chat-send-failed = 메시지 전송 실패

select-chart-host-only = 호스트만 차트를 선택할 수 있습니다.
select-chart-local = 로컬 차트를 선택할 수 없습니다.
select-chart-failed = 차트 선택 실패
select-chart-not-now = 현재 차트를 선택할 수 없습니다.

msg-create-room = `{ $user }` 님이 방을 생성했습니다.
msg-join-room = `{ $user }` 님이 방에 참여했습니다.
msg-leave-room = `{ $user }` 님이 방을 나갔습니다.
msg-new-host = `{ $user }` 님이 새로운 호스트가 되었습니다.
msg-select-chart = 호스트 `{ $user }` 님이 차트 `{ $chart }` (#{ $id })를 선택했습니다.
msg-game-start = 호스트 `{ $user }` 님이 게임을 시작했습니다. 다른 플레이어들은 준비하세요.
msg-ready = `{ $user }` 님이 준비되었습니다.
msg-cancel-ready = `{ $user }` 님이 준비 취소했습니다.
msg-cancel-game = `{ $user }` 님이 게임을 취소했습니다.
msg-start-playing = 게임 시작
msg-played = `{ $user }` 님이 플레이 완료: { $score } ({ $accuracy }){ $full-combo ->
  [true] , 풀 콤보
  *[other] {""}
}
msg-game-end = 게임 종료
msg-abort = `{ $user }` 님이 게임을 중단했습니다.
msg-room-lock = { $lock ->
  [true] 방이 잠겼습니다.
  *[other] 방이 잠금 해제되었습니다.
}
msg-room-cycle = { $cycle ->
  [true] 방이 순환 모드로 변경되었습니다.
  *[other] 방이 일반 모드로 변경되었습니다.
}
mp-library = 채보
mp-chart-label = 채보

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
