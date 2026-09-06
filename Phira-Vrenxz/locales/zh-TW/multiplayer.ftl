multiplayer = 多人遊戲
connect = 連線
connect-must-login = 登入後才可進入多人遊戲
connect-success = 連線成功
server-welcome = Welcome Phira-V Server!
connect-failed = 連線失敗
connect-authenticate-failed = 身分驗證失敗
reconnect = 斷線重連中…
create-room = 建立房間
create-room-success = 房間已建立
create-room-failed = 建立房間失敗
create-invalid-id = 房間 ID 應由不多於 20 個大小寫英文字母、數字以及 -_ 組成
join-room = 加入房間
join-room-invalid-id = 房間 ID 無效
join-room-failed = 加入房間失敗
join-room-password-title = 輸入房間密碼
room-list = 公共房間
room-list-title = 公共房間（點擊加入）
room-list-loading = 載入中…
room-list-empty = 暫無可加入的房間
room-list-failed = 取得房間列表失敗
room-list-more = …還有更多房間，請直接輸入房間號
room-locked-tag = 🔒

set-password = 設定房間密碼
set-password-failed = 設定房間密碼失敗
clear-password = 清除房間密碼
kick-user = 踢出玩家(ID)
kick-user-failed = 踢出玩家失敗
kick-user-invalid-id = 無效的玩家 ID（請在玩家列表中查看 #ID）
transfer-host = 移交房主(ID)
transfer-host-failed = 移交房主失敗
transfer-host-invalid-id = 無效的玩家 ID（請在玩家列表中查看 #ID）

leave-room = 離開房間
leave-room-failed = 離開房間失敗
disconnect = 中斷連線
request-start = 開始遊戲
request-start-no-chart = 你尚未選擇譜面
request-start-failed = 開始遊戲失敗
user-list = 使用者列表
preview = 預覽譜面
preview-failed = 預覽失敗
preview-unavailable = 目前沒有可預覽的譜面
# 預覽譜面期間房主按了開始：回到房間後詢問是否準備
preview-interrupted-title = 房主要開始遊戲啦！
preview-interrupted-content = 房主要開始遊戲啦！是否準備？
preview-ready = 準備
preview-not-now = 暫不
lock-room =
    { $current ->
        [true] 解鎖房間
       *[other] 鎖定房間
    }
cycle-room =
    { $current ->
        [true] 循環模式
       *[other] 普通模式
    }
ready = 準備
ready-failed = 準備失敗
cancel-ready = 取消
room-id = 房間 ID：{ $id }
download-failed = 下載譜面失敗
lock-room-failed = 鎖定房間失敗
cycle-room-failed = 切換房間模式失敗
chat-placeholder = 說些什麼…
chat-send = 發送
chat-empty = 訊息內容不能為空
chat-sent = 已發送
chat-send-failed = 訊息發送失敗
select-chart-host-only = 只有房主可以選擇譜面
select-chart-local = 不能選擇本地譜面
select-chart-failed = 選擇譜面失敗
select-chart-not-now = 你現在不能選擇譜面
msg-create-room = `{ $user }` 建立了房間
msg-join-room = `{ $user }` 加入了房間
msg-leave-room = `{ $user }` 離開了房間
msg-new-host = `{ $user }` 成為了新的房主
msg-select-chart = 房主 `{ $user }` 選擇了譜面 `{ $chart }` (#{ $id })
msg-game-start = 房主 `{ $user }` 開始了遊戲，請其他玩家準備
msg-ready = `{ $user }` 已就緒
msg-cancel-ready = `{ $user }` 取消了準備
msg-cancel-game = `{ $user }` 取消了遊戲
msg-start-playing = 遊戲開始
msg-played =
    `{ $user }` 結束了遊玩：{ $score } ({ $accuracy }){ $full-combo ->
        [true] ，全連
       *[other] { "" }
    }
msg-game-end = 遊戲結束
msg-abort = `{ $user }` 放棄了遊戲
msg-kicked-me = 你已被房主移出房間
msg-kicked = `{ $user }` 已被房主移出房間
msg-room-results = 本局結算：{ $n } 位玩家完成，點擊查看排名
results-title = 對局結算
results-aborted = 中途放棄
msg-room-lock =
    { $lock ->
        [true] 房間已鎖定
       *[other] 房間已解鎖
    }
msg-room-cycle =
    { $cycle ->
        [true] 房間已切換為循環模式
       *[other] 房間已切換為普通模式
    }

# —— 新版面板 UI ——
mp-connect-hint = 連線伺服器後，即可與好友一起遊玩
mp-close-hint = 點擊面板外任意處關閉
mp-lobby-connected = 已連線到伺服器
mp-lobby-not-room = 尚未進入房間
mp-room-tag = 房間 #{ $id }
mp-player-count = 玩家（{ $n }）
mp-you = 我
mp-watching = 觀戰
mp-ready-tag = 已就緒
mp-host = 房主
mp-state-choose = 選擇譜面中…
mp-state-chosen = 已選譜面 #{ $id }
mp-state-local = 本地譜面分享中
mp-state-wait = 等待開始
mp-state-playing = 對局進行中
mp-locked-tag = 已鎖定
mp-cycle-tag = 循環模式
mp-n-players = { $n } 名玩家
mp-chat-caption = 房間訊息
mp-msg-none = 暫無訊息
mp-manage-title = 管理 { $name }
mp-manage-transfer = 設為房主
mp-manage-kick = 移出房間
mp-manage-cancel = 取消
user-list-hint = 點擊空白處關閉
room-list-tap-hint = 點擊房間行加入 · 點擊空白處關閉
mp-room-locked = 🔒 需要密碼
mp-room-counts = { $players } 名玩家 / { $spectators } 名觀戰

# Phira-Vrenxz 本地譜面（補齊與簡中/繁英一致）
mp-server-no-local-chart = 此伺服器不支援本地譜面
mp-syncing-chart = 正在同步譜面...
mp-sync-failed = 譜面同步失敗: { $err }
