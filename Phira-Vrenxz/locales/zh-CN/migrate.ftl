# 旧版本数据检测 / 同步（见 Phira-Vrenxz/src/migrate.rs）
# 注意：值里不要出现空行（Fluent 里空行会打断 pattern），段落之间用单个换行即可。

migrate-title = 发现旧版本数据

migrate-message =
    检测到旧版本「{ $name }」的数据目录：
    { $path }
    其中有 { $charts } 张本地谱面，账号{ $login }，最后修改于 { $time }。
    要把这份旧数据同步到当前版本吗？（同步完成后需要重新启动游戏）

migrate-login-yes = 已登录
migrate-login-no = 未登录

migrate-sync = 同步旧数据
migrate-skip = 直接进入游戏
migrate-cancel = 取消
migrate-none = 没有发现旧版本数据

migrate-syncing-title = 正在同步旧数据

migrate-syncing-message =
    正在把旧版数据复制到当前版本，请不要关闭游戏…
    已复制 { $files } 个文件（{ $size }）

migrate-done-title = 同步完成

migrate-done-message =
    旧版数据已同步到当前版本，共 { $files } 个文件（{ $size }）。
    当前版本原来的 data.json 已备份为 { $backup }。
    请重新启动游戏：这次运行读到的仍然是旧数据，重启后才会生效。
    以后不会再自动提示；需要再同步别的旧安装时，可以在「设置 → 存储与重置 → 同步旧版本数据」里手动同步。

migrate-done-exit = 退出游戏
migrate-done-later = 稍后自己重启

migrate-fail-title = 同步失败

migrate-fail-message =
    同步旧版数据时出错：{ $error }
    也可以手动把旧版本的 data 目录整个复制到当前版本的 data 目录。

migrate-ok = 知道了

migrate-settings-item = 同步旧版本数据
migrate-settings-item-sub = 从旧版 PhirLie / Phira-Vrenxz 的 data 目录导入（自动提示只在第一次同步前出现）
migrate-settings-now = 立即同步
