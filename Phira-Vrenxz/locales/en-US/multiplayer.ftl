
multiplayer = Multiplayer

connect = Connect
connect-must-login = You must login to access multiplayer functionality.
connect-success = Connected successfully.
server-welcome = Welcome Phira-V Server!
connect-failed = Failed to connect.
connect-authenticate-failed = Authorization failed.
reconnect = Reconnecting…

create-room = Create Room
create-room-success = Room created.
create-room-failed = Failed to create room.
create-invalid-id = A Room ID should be 20 characters max, and only contain A-Z, a-z, 0-9, -, and _.

join-room = Join Room
join-room-invalid-id = Invalid room ID.
join-room-failed = Failed to join room.
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

leave-room = Leave Room
leave-room-failed = Failed to leave room.

disconnect = Disconnect

request-start = Start Game
request-start-no-chart = Select an online chart first.
request-start-failed = Failed to start.

user-list = Users
preview = Preview Chart
preview-failed = Failed to preview.
preview-unavailable = No chart available to preview.
# After the host starts the game while you were previewing: ask whether to get ready
preview-interrupted-title = The host is starting the game!
preview-interrupted-content = The host is starting the game! Ready to join?
preview-ready = Ready
preview-not-now = Not now
preview-started-title = The host has started the game
preview-started-content = The host started this round while you were previewing the chart. You can play together again next round.
preview-started-ok = Got it
# Spectating
spectate = Spectate
spectate-title = Spectating
spectate-none = No one is playing right now.
spectate-waiting = Waiting…
spectate-watch = Sync Spectate
spectate-exit = Exit Spectate
spectate-no-chart = No chart available to watch.
spectate-joined = Spectating
spectate-chart = Now playing: { $chart }

lock-room = { $current ->
  [true] Unlock Room
  *[other] Lock Room
}
cycle-room = { $current ->
  [true] Cycle Mode
  *[other] Normal Mode
}

ready = Ready
ready-failed = Failed to get ready.

cancel-ready = Cancel

room-id = Room ID: { $id }

download-failed = Failed to download chart.

lock-room-failed = Failed to lock room.
cycle-room-failed = Failed to change room mode.

chat-placeholder = Type a message...
chat-send = Send
chat-empty = Message is empty.
chat-sent = Sent
chat-send-failed = Failed to send message.

select-chart-host-only = Only the host can select charts.
select-chart-local = You can only select online charts.
select-chart-failed = Failed to select chart.
select-chart-not-now = You can't select chart now.

msg-create-room = `{ $user }` created the room.
msg-join-room = `{ $user }` joined the room.
msg-leave-room = `{ $user }` left the room.
msg-new-host = `{ $user }` became the new host.
msg-select-chart = The host `{ $user }` selected chart `{ $chart }` (#{ $id }).
msg-game-start = The host `{ $user }` started the game.
msg-ready = `{ $user }` is ready.
msg-cancel-ready = `{ $user }` canceled being ready.
msg-cancel-game = `{ $user }` cancelled the game.
msg-start-playing = Game started.
msg-played = `{ $user }` finished playing: { $score } ({ $accuracy }){ $full-combo ->
  [true] , full combo.
  *[other] {""}.
}
msg-game-end = Game ended.
msg-abort = `{ $user }` aborted the game.
msg-kicked-me = You have been removed from the room by the host.
msg-kicked = `{ $user }` has been removed from the room by the host.
# Local chart sharing / pause sync (spectating)
msg-select-local-chart = `{ $user }` selected a local chart: { $chart }
msg-send-chart = `{ $user }` started sharing the chart
msg-download-ready = `{ $user }` finished downloading the chart
msg-player-paused = `{ $user }` paused the game
msg-player-resumed = `{ $user }` resumed the game
msg-room-results = Round finished: { $n } players. Tap to view ranking.
results-title = Round Results
results-aborted = Aborted
msg-room-lock = { $lock ->
  [true] Room locked.
  *[other] Room unlocked.
}
msg-room-cycle = { $cycle ->
  [true] Room mode changed to Cycle.
  *[other] Room mode changed to Normal.
}

# Phira-Vrenxz multiplayer
mp-server-no-local-chart = This server does not support local charts
mp-syncing-chart = Syncing chart...
mp-sync-failed = Chart sync failed: { $err }

# —— full-screen paged UI ——
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
mp-watching = Watching
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
mp-chat-caption = Room chat
mp-msg-none = No messages yet
mp-manage-title = Manage { $name }
mp-manage-transfer = Make host
mp-manage-kick = Kick out
mp-manage-cancel = Cancel
room-list-tap-hint = Tap a row to join · tap Spectate to watch
mp-room-locked = 🔒 Password
mp-room-counts = { $players } players / { $spectators } watching
mp-spectate-hint = Tap a player row to pick the sync-watch target
spectate-best = Best { $score }
spectate-syncing = Synced
