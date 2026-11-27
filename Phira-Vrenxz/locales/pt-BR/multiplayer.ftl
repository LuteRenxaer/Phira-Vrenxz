
multiplayer = Multiplayer

connect = Conectar
connect-must-login = Você deve fazer login para entrar no modo multijogador
connect-success = Conectado com sucesso
connect-failed = Falhou ao conectar
connect-authenticate-failed = Falha na autorização
reconnect = Reconectando…

create-room = Criar sala
create-room-success = Sala criada
create-room-failed = Falha ao criar sala
create-invalid-id = O ID da sala consiste em no máximo 20 caracteres, incluindo letras, números, - (dash) e _ (underscore)

join-room = Juntar-se à sala
join-room-invalid-id = ID de  sala inválido
join-room-failed = Falha ao entrar na sala

leave-room = Sair da sala
leave-room-failed = Falha ao sair da sala

disconnect = desconectar

request-start = Começar o jogo
request-start-no-chart = Você não selecionou um gráfico
request-start-failed = Falha ao iniciar o jogo

user-list = Usuários

lock-room = { $current ->
  [true] Desbloquear sala
  *[other] bloquear sala
}
cycle-room = { $current ->
  [true] Modo de ciclismo
  *[other] Modo normal
}

ready = Preparar
ready-failed = Falha ao se preparar

cancel-ready = Cancelar

room-id = ID da sala: { $id }

download-failed = Falha ao baixar o gráfico

lock-room-failed = Falha ao bloquear a sala
cycle-room-failed = Falha ao alterar o modo de sala

chat-placeholder = Dizer alguma coisa…
chat-send = Send
chat-empty = A mensagem está vazia
chat-sent = Enviar
chat-send-failed = Falha ao enviar mensagem

select-chart-host-only = Somente o anfitrião pode selecionar o gráfico
select-chart-local = Não é possível selecionar o gráfico local
select-chart-failed = Falha ao selecionar o gráfico
select-chart-not-now = Você não pode selecionar o gráfico agora

msg-create-room = `{ $user }` criou a sala
msg-join-room = `{ $user }` entrou na sala
msg-leave-room = `{ $user }` deixou a sala
msg-new-host = `{ $user }` tornou-se o novo anfitrião
msg-select-chart = O anfitrião `{ $user }` selecionou gráfico  `{ $chart }` (#{ $id })
msg-game-start = O anfitrião `{ $user }` começou o jogo. Outros jogadores devem se preparar .
msg-ready = `{ $user }` está preparado
msg-cancel-ready = `{ $user }` cancelou
msg-cancel-game = `{ $user }` cancelou o jogo
msg-start-playing = jogo iniciado
msg-played = `{ $user }` terminou de jogar : { $score } ({ $accuracy }){ $full-combo ->
  [true] , combo completo
  *[other] {""}
}
msg-game-end = O jogo terminou
msg-abort = `{ $user }` abortou o jogo
msg-room-lock = { $lock ->
  [true] Sala bloqueada
  *[other] Sala desbloqueada
}
msg-room-cycle = { $cycle ->
  [true] Sala alterada para modo de ciclismo
  *[other] Sala alterada para modo normal
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

