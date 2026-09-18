# Legacy data detection / sync (see Phira-Vrenxz/src/migrate.rs)

migrate-title = Legacy data found

migrate-message =
    Found data from an older version ("{ $name }") at:
    { $path }
    It contains { $charts } local charts, account: { $login }, last modified { $time }.
    Sync this legacy data into the current version? (The game must be restarted afterwards.)

migrate-login-yes = signed in
migrate-login-no = not signed in

migrate-sync = Sync legacy data
migrate-skip = Enter the game
migrate-cancel = Cancel
migrate-none = No legacy data found

migrate-syncing-title = Syncing legacy data

migrate-syncing-message =
    Copying the legacy data into the current version, please do not close the game...
    { $files } files copied ({ $size })

migrate-done-title = Sync complete

migrate-done-message =
    The legacy data has been synced into the current version: { $files } files ({ $size }).
    The previous data.json was backed up as { $backup }.
    Please restart the game: this run still uses the old data, it takes effect after a restart.
    You will not be prompted again; to sync another old installation later, use Settings -> Storage -> Sync legacy data.

migrate-done-exit = Quit game
migrate-done-later = Restart later

migrate-fail-title = Sync failed

migrate-fail-message =
    Failed to sync the legacy data: { $error }
    You can also copy the old version's data folder into the current version's data folder manually.

migrate-ok = Got it

migrate-settings-item = Sync legacy data
migrate-settings-item-sub = Import from an older PhirLie / Phira-Vrenxz data folder (the auto prompt only appears before the first sync)
migrate-settings-now = Sync now
