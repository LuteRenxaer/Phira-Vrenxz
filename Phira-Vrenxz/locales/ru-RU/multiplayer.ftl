multiplayer = Мультиплеер
connect = Подключиться
connect-must-login = Войдите в аккаунт, чтобы использовать мультиплеер
connect-success = Подключение успешно
connect-failed = Ошибка подключения
connect-authenticate-failed = Ошибка авторизации
reconnect = Переподключаемся…
create-room = Создать комнату
create-room-success = Комната создана
create-room-failed = Ошибка создания комнаты
create-invalid-id = ID комнаты может быть длиной не более 20 символов, и может состоять из строчных и прописных букв, цифр, тире(-) и нижнего подчëркивания.
join-room = Присоединиться
join-room-invalid-id = Неверный ID комнаты
join-room-failed = Ошибка при попытке присоединиться в комнату
leave-room = Покинуть
leave-room-failed = Ошибка при попытке покинуть комнату
disconnect = Отсоединиться
request-start = Начать
request-start-no-chart = Вы не выбрали чарт
request-start-failed = Ошибка при запуске игры
user-list = Игроки
lock-room =
    { $current ->
        [true] Закрыть комнату
       *[other] Открыть комнату
    }
cycle-room =
    { $current ->
        [true] По очереди
       *[other] Стандарт
    }
ready = Готов
ready-failed = Ошибка при попытке готовности
cancel-ready = Отмена
room-id = ID комнаты: { $id }
download-failed = Загрузка чарта не удалась.
lock-room-failed = Ошибка при попытке закрыть комнату
cycle-room-failed = Ошибка при попытке сменить режим комнаты
chat-placeholder = Скажите что нибудь…
chat-send = Отправить
chat-empty = Пустое сообщение
chat-sent = Отправлено
chat-send-failed = Ошибка при отправке
select-chart-host-only = Только хост может выбирать чарт
select-chart-local = Нельзя выбрать локальный чарт
select-chart-failed = Ошибка при выборе чарта
select-chart-not-now = Вы не можете пока что выбрать чарт.
msg-create-room = `{ $user }` создал комнату
msg-join-room = `{ $user }` присоединился к комнате
msg-leave-room = `{ $user }` покинул комнату
msg-new-host = `{ $user }` стал новым хостом
msg-select-chart = Хост `{ $user }` выбрал чарт `{ $chart }` (#{ $id })
msg-game-start = Хост `{ $user }` начал игру. Остальные игроки, нажмите на кнопку, когда будете готовы.
msg-ready = `{ $user }` готов
msg-cancel-ready = `{ $user }` отменил готовность
msg-cancel-game = `{ $user }` отменил игру
msg-start-playing = Раунд начался
msg-played =
    `{ $user }` завершил играть: { $score } ({ $accuracy }){ $full-combo ->
        [true] , Фулл Комбо
       *[other] { "" }
    }
msg-game-end = Раунд окончен
msg-abort = `{ $user }` вышел посреди раунда
msg-room-lock =
    { $lock ->
        [true] Комната закрыта
       *[other] Комната открыта
    }
msg-room-cycle =
    { $cycle ->
        [true] Включëн режим "По очереди".Теперь роль хоста передаëтся другому игроку после окончания раунда
       *[other] Включëн режим "Классика" Теперь хост не меняется после окончания раунда
    }
mp-library = Карты
mp-chart-label = Карта

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
