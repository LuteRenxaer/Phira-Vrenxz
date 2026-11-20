# 旧版本数据检测 / 同步（见 Phira-Vrenxz/src/migrate.rs）
# 注意：值里不要出现空行（Fluent 里空行会打断 pattern），段落之间用单个换行即可。

migrate-title = 察得舊版之數據

migrate-message =
    察得舊版「{ $name }」之數據目錄：
    { $path }
    其中本地譜面 { $charts } 張，帳號{ $login }，末改於 { $time }。
    欲以舊數同步至當下之版乎？（同步畢須重啓遊戲）

migrate-login-yes = 已登入
migrate-login-no = 未登入

migrate-sync = 同步舊數
migrate-skip = 逕入遊戲
migrate-cancel = 取消
migrate-none = 未察得舊版之數據

migrate-syncing-title = 正在同步舊數

migrate-syncing-message =
    正在以舊數錄入當下之版，勿閉遊戲…
    已錄 { $files } 文（{ $size }）

migrate-done-title = 同步畢

migrate-done-message =
    舊數已同步至當下之版，共 { $files } 文（{ $size }）。
    當下之版原 data.json 已備為 { $backup }。
    請重啓遊戲：此番所讀仍是舊數，重啓方生效。
    此後不復自提示；欲再同步他舊安裝，可於「設定 → 儲存與重置 → 同步舊版之數據」中手動同步。

migrate-done-exit = 退出遊戲
migrate-done-later = 稍後自啓

migrate-fail-title = 同步敗

migrate-fail-message =
    同步舊數之時有誤：{ $error }
    亦可手將舊版之 data 目錄全錄入當下之 data 目錄。

migrate-ok = 知矣

migrate-settings-item = 同步舊版之數據
migrate-settings-item-sub = 自舊版 PhirLie／Phira-Vrenxz 之 data 目錄導入（僅初同步前自提示）
migrate-settings-now = 即刻同步
