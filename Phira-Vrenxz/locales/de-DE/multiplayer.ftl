multiplayer = Mehrspieler
connect = Verbinden
connect-must-login = Du musst eingeloggt sein, um den Mehrspieler-Modus zu nutzen
connect-success = Verbindung erfolgreich.
connect-failed = Verbindung fehlgeschlagen.
connect-authenticate-failed = Autorisierung fehlgeschlagen.
reconnect = Verbindung wird wiederhergestellt…
create-room = Raum erstellen
create-room-success = Raum erstellt
create-room-failed = Raum konnte nicht erstellt werden
create-invalid-id = Eine Raum-ID darf maximal 20 Zeichen enthalten und nur „A-Z“, „a-z“, „0-9“, „-“, „_“ nutzen
join-room = Raum beitreten
join-room-invalid-id = Ungültige Raum-ID
join-room-failed = Beitritt zum Raum fehlgeschlagen
leave-room = Raum verlassen
leave-room-failed = Raum konnte nicht verlassen werden
disconnect = Trennen
request-start = Spiel starten
request-start-no-chart = Wähle zuerst ein Online-Level aus
request-start-failed = Spiel konnte nicht gestartet werden
user-list = Benutzerliste
lock-room =
    { $current ->
        [true] Raum entsperren
       *[other] Raum sperren
    }
cycle-room =
    { $current ->
        [true] Round-Robin Modus
       *[other] Normalmodus
    }
ready = Bereit
ready-failed = Bereitmeldung fehlgeschlagen.
cancel-ready = Abbrechen
room-id = Raum-ID: { $id }
download-failed = Level konnte nicht heruntergeladen werden
lock-room-failed = Raum konnte nicht gesperrt werden
cycle-room-failed = Raummodus konnte nicht geändert werden
chat-placeholder = Nachricht eingeben…
chat-send = Senden
chat-empty = Nachricht ist leer
chat-sent = Gesendet
chat-send-failed = Nachricht konnte nicht gesendet werden
select-chart-host-only = Nur der Host kann Levels auswählen
select-chart-local = Du kannst nur Online-Levels auswählen
select-chart-failed = Level konnte nicht ausgewählt werden
select-chart-not-now = Du kannst gerade kein Level auswählen
msg-create-room = `{ $user }` hat den Raum erstellt
msg-join-room = `{ $user }` ist dem Raum beigetreten
msg-leave-room = `{ $user }` hat den Raum verlassen
msg-new-host = `{ $user }` ist der neue Host
msg-select-chart = Der Host `{ $user }` hat das Level `{ $chart }` (#{ $id }) ausgewählt
msg-game-start = Der Host `{ $user }` hat das Spiel gestartet
msg-ready = `{ $user }` ist bereit
msg-cancel-ready = `{ $user }` ist nicht mehr bereit
msg-cancel-game = `{ $user }` hat das Spiel abgebrochen
msg-start-playing = Spiel gestartet.
msg-played =
    `{ $user }` hat Level abgeschlossen: { $score } ({ $accuracy }){ $full-combo ->
        [true] , Full Combo.
       *[other] { "" }.
    }
msg-game-end = Spiel beendet.
msg-abort = `{ $user }` hat das Spiel aufgegeben
msg-room-lock =
    { $lock ->
        [true] Raum gesperrt.
       *[other] Raum entsperrt.
    }
msg-room-cycle =
    { $cycle ->
        [true] Raummodus geändert zu Round-Robin.
       *[other] Raummodus geändert zu Normal.
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
