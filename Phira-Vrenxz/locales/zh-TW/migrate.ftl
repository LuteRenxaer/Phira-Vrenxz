# 舊版本資料偵測 / 同步（見 Phira-Vrenxz/src/migrate.rs）

migrate-title = 發現舊版本資料

migrate-message =
    偵測到舊版本「{ $name }」的資料目錄：
    { $path }
    其中有 { $charts } 張本機譜面，帳號{ $login }，最後修改於 { $time }。
    要把這份舊資料同步到目前版本嗎？（同步完成後需要重新啟動遊戲）

migrate-login-yes = 已登入
migrate-login-no = 未登入

migrate-sync = 同步舊資料
migrate-skip = 直接進入遊戲
migrate-cancel = 取消
migrate-none = 沒有發現舊版本資料

migrate-syncing-title = 正在同步舊資料

migrate-syncing-message =
    正在把舊版資料複製到目前版本，請不要關閉遊戲…
    已複製 { $files } 個檔案（{ $size }）

migrate-done-title = 同步完成

migrate-done-message =
    舊版資料已同步到目前版本，共 { $files } 個檔案（{ $size }）。
    目前版本原本的 data.json 已備份為 { $backup }。
    請重新啟動遊戲：這次執行讀到的仍然是舊資料，重啟後才會生效。
    之後不會再自動提示；需要再同步其他舊安裝時，可以在「設定 → 儲存與重設 → 同步舊版本資料」裡手動同步。

migrate-done-exit = 結束遊戲
migrate-done-later = 稍後自己重啟

migrate-fail-title = 同步失敗

migrate-fail-message =
    同步舊版資料時發生錯誤：{ $error }
    也可以手動把舊版本的 data 目錄整個複製到目前版本的 data 目錄。

migrate-ok = 知道了

migrate-settings-item = 同步舊版本資料
migrate-settings-item-sub = 從舊版 PhirLie / Phira-Vrenxz 的 data 目錄匯入（自動提示只在第一次同步前出現）
migrate-settings-now = 立即同步
