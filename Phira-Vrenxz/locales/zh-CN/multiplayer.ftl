
multiplayer = 多人游戏

connect = 连接
connect-must-login = 登录后才能进入多人游戏
connect-success = 连接成功
server-welcome = Welcome Phira-V Server!
connect-failed = 连接失败
connect-authenticate-failed = 鉴权失败
reconnect = 断线重连中…

create-room = 创建房间
create-room-success = 房间已创建
create-room-failed = 创建房间失败
create-invalid-id = 房间 ID 由不多于 20 个大小写英文字母、数字以及 -_ 组成

join-room = 加入房间
join-room-invalid-id = 无效的房间 ID
join-room-failed = 加入房间失败
join-room-password-title = 输入房间密码
room-list = 公共房间
room-list-title = 公共房间
room-list-loading = 加载中…
room-list-empty = 暂无可加入的房间
room-list-failed = 获取房间列表失败
room-list-more = …还有更多房间，请直接输入房间号
room-locked-tag = 🔒

set-password = 设置房间密码
set-password-failed = 设置房间密码失败
clear-password = 清除房间密码
kick-user = 踢出玩家(ID)
kick-user-failed = 踢出玩家失败
kick-user-invalid-id = 无效的玩家 ID（在玩家列表中查看 #ID）
transfer-host = 移交房主(ID)
transfer-host-failed = 移交房主失败
transfer-host-invalid-id = 无效的玩家 ID（在玩家列表中查看 #ID）

leave-room = 离开房间
leave-room-failed = 离开房间失败

disconnect = 断开连接

request-start = 开始游戏
request-start-no-chart = 你还没有选择谱面
request-start-failed = 开始游戏失败

user-list = 用户列表
preview = 预览谱面
preview-failed = 预览失败
preview-unavailable = 当前没有可预览的谱面
# 预览谱面期间房主点了开始：回到房间后询问是否准备
preview-interrupted-title = 房主要开始游戏啦！
preview-interrupted-content = 房主要开始游戏啦！是否准备？
preview-ready = 准备
preview-not-now = 暂不
preview-started-title = 房主已经开始游戏了
preview-started-content = 你还在预览谱面时房主开始了这一局。本局结束后可以继续一起游玩。
preview-started-ok = 知道了
# 观战
spectate = 观战
spectate-title = 观战中
spectate-none = 暂无玩家在游玩
spectate-waiting = 等待中…
spectate-watch = 同步观战
spectate-exit = 退出观战
spectate-no-chart = 当前没有可观看的谱面
spectate-joined = 已进入观战
spectate-chart = 正在游玩：{ $chart }

lock-room = { $current ->
  [true] 解锁房间
  *[other] 锁定房间
}
cycle-room = { $current ->
  [true] 循环模式
  *[other] 普通模式
}

ready = 准备
ready-failed = 准备失败

cancel-ready = 取消

room-id = 房间 ID：{ $id }

download-failed = 下载谱面失败

lock-room-failed = 锁定房间失败
cycle-room-failed = 切换房间模式失败

chat-placeholder = 说些什么…
chat-send = 发送
chat-empty = 消息不能为空
chat-sent = 已发送
chat-send-failed = 消息发送失败

select-chart-host-only = 只有房主可以选择谱面
select-chart-local = 不能选择本地谱面
select-chart-failed = 选择谱面失败
select-chart-not-now = 你现在不能选择谱面

msg-create-room = `{ $user }` 创建了房间
msg-join-room = `{ $user }` 加入了房间
msg-leave-room = `{ $user }` 离开了房间
msg-new-host = `{ $user }` 成为了新的房主
msg-select-chart = 房主 `{ $user }` 选择了谱面 `{ $chart }` (#{ $id })
msg-game-start = 房主 `{ $user }` 开始了游戏，请其他玩家准备
msg-ready = `{ $user }` 已就绪
msg-cancel-ready = `{ $user }` 取消了准备
msg-cancel-game = `{ $user }` 取消了游戏
msg-start-playing = 游戏开始
msg-played = `{ $user }` 结束了游玩：{ $score } ({ $accuracy }){ $full-combo ->
  [true] ，全连
  *[other] {""}
}
msg-game-end = 游戏结束
msg-abort = `{ $user }` 放弃了游戏
msg-kicked-me = 你已被房主移出房间
msg-kicked = `{ $user }` 已被房主移出房间
# 本地谱面分享 / 暂停同步（观战）
msg-select-local-chart = `{ $user }` 选择了本地谱面: { $chart }
msg-send-chart = `{ $user }` 开始分享谱面
msg-download-ready = `{ $user }` 谱面下载完成
msg-player-paused = `{ $user }` 暂停了游戏
msg-player-resumed = `{ $user }` 继续了游戏
msg-room-results = 本局结算：{ $n } 位玩家完成，点击查看排名
results-title = 对局结算
results-aborted = 中途放弃
msg-room-lock = { $lock ->
  [true] 房间已锁定
  *[other] 房间已解锁
}
msg-room-cycle = { $cycle ->
  [true] 房间已切换为循环模式
  *[other] 房间已切换为普通模式
}

# Phira-Vrenxz multiplayer
mp-server-no-local-chart = 当前服务器不支持本地谱面
mp-syncing-chart = 正在同步谱面...
mp-sync-failed = 谱面同步失败: { $err }

# —— 整屏分页 UI ——
mp-connect-hint = 连接服务器后，即可与好友一起游玩
mp-connecting = 连接中…
mp-server = 服务器：{ $addr }
mp-back = 返回
mp-refresh = 刷新
mp-manage-hint = 点击玩家行可管理该玩家
mp-lobby-connected = 已连接到服务器
mp-lobby-not-room = 尚未进入房间
mp-room-tag = 房间 #{ $id }
mp-player-count = 玩家（{ $n }）
mp-you = 我
mp-watching = 观战
mp-ready-tag = 已就绪
mp-host = 房主
mp-state-choose = 选择谱面中…
mp-state-chosen = 已选谱面 #{ $id }
mp-state-local = 本地谱面分享中
mp-state-wait = 等待开始
mp-state-playing = 对局进行中
mp-locked-tag = 已锁定
mp-cycle-tag = 循环模式
mp-n-players = { $n } 名玩家
mp-chat-caption = 聊天&日志
mp-chart-label = 谱面
mp-msg-none = 暂无消息
mp-manage-title = 管理 { $name }
mp-manage-transfer = 设为房主
mp-manage-kick = 移出房间
mp-manage-cancel = 取消
room-list-tap-hint = 点击房间行加入 · 右侧「观战」围观对局
mp-room-locked = 🔒 需密码
mp-room-counts = { $players } 名玩家 / { $spectators } 名观战
mp-spectate-hint = 点击玩家行选择同步观战的目标
spectate-best = 最高分 { $score }
spectate-syncing = 同步中
mp-library = 谱面库
