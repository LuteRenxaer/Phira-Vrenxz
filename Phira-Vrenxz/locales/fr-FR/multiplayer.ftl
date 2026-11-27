
multiplayer = Multijoueur

connect = Se connecter
connect-must-login = Vous devez vous connecter pour accéder au mode multijoueur.
connect-success = Connecté(e) avec succès.
connect-failed = Échec de connexion.
connect-authenticate-failed = Échec d'autorisation
reconnect = Reconnexion…

create-room = Créer une salle
create-room-success = Salle créée.
create-room-failed = Impossible de créer une salle.
create-invalid-id = L'ID de la salle ne contient pas plus de 20 caractères, dont des lettres, des chiffres, - (tiret) et _ (tiret du bas).

join-room = Rejoindre la salle
join-room-invalid-id = ID de salle invalide.
join-room-failed = Impossible de rejoindre la salle.

leave-room = Quitter la salle
leave-room-failed = Impossible de quitter la salle.

disconnect = Se déconnecter

request-start = Démarrer le jeu
request-start-no-chart = Vous n'avez pas encore sélectionné de partition.
request-start-failed = Impossible de démarrer le jeu.

user-list = Utilisateurs

lock-room = { $current ->
  [true] Déverrouiller la salle
  *[other] Verrouiller la salle
}
cycle-room = { $current ->
  [true] Mode cycle
  *[other] Mode normal
}

ready = Prêt(e)
ready-failed = Impossible de se préparer.

cancel-ready = Annuler

room-id = ID de salle: { $id }

download-failed = Échec du téléchargement de la partition

lock-room-failed = Impossible de verrouiller la salle
cycle-room-failed = Échec du changement de mode de salle

chat-placeholder = Dire quelque chose...
chat-send = Envoyer
chat-empty = Le message ne peut pas être vide.
chat-sent = Envoyé
chat-send-failed = Échec de l'envoi du message.

select-chart-host-only = Seul(e) l'hôte(sse) peut sélectionner la partition.
select-chart-local = Vous ne pouvez pas sélectionner la partition locale.
select-chart-failed = Impossible de sélectionner la partition.
select-chart-not-now = Vous ne pouvez pas sélectionner de partition maintenant.

msg-create-room = `{ $user }` a créé la salle.
msg-join-room = `{ $user }` a rejoint la salle.
msg-leave-room = `{ $user }` a quitté la salle.
msg-new-host = `{ $user }` est devenu(e) le(la) nouvel(le) hôte(sse) de la salle.
msg-select-chart = L'hôte(sse) `{ $user }` a sélectionné la partition `{ $chart }` (#{ $id }).
msg-game-start = L'hôte(sse) `{ $user }` a démarré le jeu.
msg-ready = `{ $user }` est prêt(e).
msg-cancel-ready = `{ $user }` a annulé l'état prêt.
msg-cancel-game = `{ $user }` a annulé le jeu.
msg-start-playing = Jeu démarré.
msg-played = `{ $user }` a fini de jouer à: { $score } ({ $accuracy }){ $full-combo ->
  [true] , full combo.
  *[other] {""}.
}
msg-game-end = Jeu terminé.
msg-abort = `{ $user }` a abandonné le jeu
msg-room-lock = { $lock ->
  [true] Salle verrouillée.
  *[other] Salle déverrouillée.
}
msg-room-cycle = { $cycle ->
  [true] La salle est passée en mode cycle.
  *[other] La salle est passée en mode normal.
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

