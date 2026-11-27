
label = 设置

item-ui-style = UI 风格
item-ui-style-sub = 切换界面整体观感；默认风格与旧版一致
item-ui-style-default = 默认
item-ui-style-xcsim = XC-SIM
general = 通用
audio = 音频
chart = 谱面
debug = Debug
about = 关于

item-lang = 语言
item-fullscreen = 全屏模式
item-offline = 离线模式
item-offline-sub = 在离线模式下将不能上传成绩
item-server-status = 服务器状态
item-server-status-sub = 转到网页查看服务器状态
check-status = 查看
item-mp = 多人游戏
item-mp-sub = 启用多人游戏
item-mp-addr = 多人游戏服务器
item-mp-addr-sub = 服务器地址，'主机:端口'
item-mp-addr-invalid = 无效的服务器地址
item-lowq = 低画质模式
item-lowq-sub = 建议在画面卡顿时启用
item-clear-cache = 清除缓存
item-cache-size-loading = 加载中…
item-cache-size = 缓存大小：{ $size }
item-clear-cache-btn = 清除
item-cache-cleared = 缓存已清除
item-insecure = 不安全模式
item-insecure-sub = 当无法使用在线功能时可尝试该功能。这会使得你的连接不安全！
item-enable-anys = 启用 Anys
item-enable-anys-sub = 使用 Anys 网关以提升连接质量
item-anys-gateway = Anys 网关
item-anys-gateway-sub = Anys 网关地址
item-anys-gateway-invalid = 无效的网关地址

section-basic = 基础设置
section-network = 网络设置
section-display = 显示设置
section-storage = 存储与重置

item-fxaa = FXAA 抗锯齿
item-fxaa-sub = 开启 FXAA 抗锯齿，提升画面质量
item-roman-numerals = 罗马数字
item-roman-numerals-sub = 使用罗马数字显示难度等级
item-chinese-numerals = 中文数字
item-chinese-numerals-sub = 使用中文数字显示难度等级
item-hud = HUD 自定义
item-hud-sub = 拖动调整主菜单、谱面库、设置等页面的组件位置与大小
item-hud-open = 管理
item-reset-settings = 恢复默认设置
item-reset-settings-sub = 将所有设置恢复为默认值
item-reset-settings-btn = 恢复
item-reset-settings-done = 设置已恢复默认

item-adjust = 自动对齐时间
item-adjust-sub = 自动调整延迟以同步音乐和谱面
item-music = 音乐音量
item-sfx = 音效音量
item-bgm = BGM 音量
item-bgm-sub = 开屏 / 主页背景音乐
item-cali = 调整延迟
item-preferred-sample-rate = 首选采样率
preferred-sample-rate-default = 系统默认
item-audio-buffer-size = 音频缓冲区大小

item-custom-bgm = 自定义背景音乐
item-custom-bgm-sub = 选择本地音频文件作为主界面 BGM（重启生效）
item-custom-bgm-default = 默认
item-custom-bgm-set = 自定义 BGM 已设置
item-custom-bgm-reset = 已恢复默认 BGM
item-reset-bgm = 恢复默认 BGM
item-reset-bgm-btn = 恢复

item-custom-startup-bgm = 自定义启动画面音乐
item-custom-startup-bgm-sub = 选择本地音频文件作为启动画面 BGM
item-custom-startup-bgm-default = 默认（login.mp3）
item-custom-startup-bgm-set = 启动画面 BGM 已设置
item-custom-startup-bgm-reset = 已恢复默认启动画面 BGM
item-reset-startup-bgm = 恢复默认启动画面 BGM
item-reset-startup-bgm-btn = 恢复

item-custom-bg = 自定义主页背景
item-custom-bg-sub = 选择本地图片作为主界面背景
item-custom-bg-default = 默认
item-custom-bg-set = 自定义背景已设置
item-custom-bg-reset = 已恢复默认背景
item-reset-bg = 恢复默认背景
item-reset-bg-btn = 恢复

item-show-acc = 显示实时准度
item-show-fps = 显示帧率
item-show-fps-sub = 游玩中实时显示 FPS

item-show-avg-fps = 显示平均帧率
item-show-avg-fps-sub = 在结算界面显示平均帧率
item-ap-fc-indicator = AP/FC（全完美/全连）指示器
item-ap-fc-indicator-sub = 使用判定线颜色显示全 AP/FC 状态
item-dc-pause = 双击暂停
item-dhint = 双押提示
item-dhint-sub = 同时触线的音符将会被高亮
item-perf = 性能优化档位
item-perf-sub = 档位统一接管屏幕外剔除 / 粒子 / 垂直同步等；选“自定义”后可手动逐项控制
item-perf-off = 无优化
item-perf-off-sub = 关闭额外优化：负载统计不并行、无屏幕外剔除、不启用低清渲染
item-perf-light = 少量优化
item-perf-light-sub = 开启屏幕外剔除与低清/粒子密度自适应（负载统计仍串行）
item-perf-medium = 中等优化
item-perf-medium-sub = 现有优化的一半力度：剔除 + 低清 + 负载统计并行（更高阈值）
item-perf-full = 完全优化
item-perf-full-sub = 现有全部优化：屏幕外剔除 + 低清自适应 + 负载统计并行
item-perf-ultra = 完全积极
item-perf-ultra-sub = 完全剔除 + 粒子削减（保留 hit_fx、连续减半）+ 强制关闭垂直同步
item-perf-custom = 自定义优化
item-perf-custom-sub = 手动控制下方各项
item-perf-custom-metrics = 负载统计并行
item-perf-custom-metrics-sub = 使用多核并行统计每帧 Note 负载
item-perf-custom-cull = 屏幕外剔除
item-perf-custom-cull-sub = 跳过屏幕外的 Note 渲染
item-perf-custom-fx = 粒子削减
item-perf-custom-fx-sub = 保留 hit_fx 主粒子，去掉碎屑并每两次发射减一次
item-perf-custom-vsync = 强制关闭垂直同步
item-perf-custom-vsync-sub = 帧率不再被刷新率限制
item-perf-custom-lowres = 低清 Note 阈值
item-perf-custom-lowres-sub = 屏幕上 Note 数 ≥ 该值时改用低分辨率贴图
item-perf-custom-fxdensity = 打击特效密度阈值
item-perf-custom-fxdensity-sub = 即将击打的 Note 数超过该值时关闭打击特效
item-opt = 激进优化
item-opt-sub = 采用激进的优化策略，提升性能但可能导致部分谱面显示出错
item-use-keyboard = 使用键盘游玩
item-use-keyboard-sub = 开启后可以使用键盘进行游戏，但成绩无法上传
item-particle = 粒子效果
item-particle-sub = 开启游戏中的粒子特效
item-disable-effect = 禁用特效
item-disable-effect-sub = 禁用所有视觉特效以提升性能
item-interactive = 交互模式
item-interactive-sub = 音符与判定线的交互效果
item-prefer-reduced-motion = 减少动画效果
item-prefer-reduced-motion-sub = 减少动画和视觉特效
item-startup-screen = 显示启动画面
item-startup-screen-sub = 进入主页前显示启动画面（含语言选择）
item-speed = 速度
item-note-size = 音符大小

item-chart-debug = 谱面调试
item-chart-debug-sub = 显示判定线编号和朝向
item-touch-debug = 触摸调试
item-touch-debug-sub = 游玩过程中显示触摸点

item-full-screen-judge = 全屏判定
item-full-screen-judge-sub = 启用全屏判定（实验性功能）

item-arcaca-judgement = Arcaea 判定模式
item-arcaca-judgement-sub = 启用后使用 Arcaea 计分（满分10000000+物量），禁用成绩上传
item-fnf-judgement = FNF 判定模式
item-fnf-judgement-sub = 启用后使用 FNF 计分（Sick350/Good200/Bad100），禁用成绩上传

item-combo-text = Combo 显示文本
item-combo-text-default = COMBO
item-combo-text-edit = 修改
item-watermark = 自定义水印
item-watermark-default = Phira-Vrenxz
item-watermark-edit = 修改

load-cali-failed = 加载音频失败

about-content =
  Phira v{ $version }

  Phira 是一款玩法基于 Phigros 的非商业社区音乐游戏，使用 Rust 开发。

  BiliBili 账号：@Phira官方
  QQ 频道：r48eajexth

  推荐加入 QQ 频道以获取最新消息和获得帮助！

  工作人员名单（字典序）
  开发
  { $development }

  运维
  { $operations }

  文档
  { $documentation }

  美术
  { $art }

  音乐
  { $music }

  音效
  { $audio }

  社区管理
  { $community }

  本地化
  { $localization }

  以及许多志愿谱面审核员！完整列表参见 https://phira.moe/staff


custom-section-appearance = 界面外观
custom-ui-scale = UI 比例
custom-show-score = 显示分数
custom-show-combo = 显示连击
custom-show-acc = 显示准确率
custom-accent-color = 主题色
custom-edit = 修改
custom-section-character = 角色立绘
custom-character = 当前角色
custom-character-intro = 角色简介
custom-character-no-intro = 暂无简介
custom-character-switch = 切换角色
custom-character-switch-btn = 切换
custom-section-ui-position = UI 位置
custom-score-x = 分数水平偏移
custom-score-y = 分数垂直偏移
custom-combo-x = 连击水平偏移
custom-combo-y = 连击垂直偏移
custom-section-text = 自定义文本
custom-watermark = 自定义水印
custom-combo-text = 自定义连击文本
custom-autoplay-text = Autoplay 显示文字
custom-section-background = 自定义背景
custom-select-bg = 选择背景图片
custom-reset-bg = 重置背景
custom-reset = 重置
custom-default-bg = 默认背景
custom-section-music = 自定义音乐
custom-home-bgm = 开屏 / 主页背景音乐
custom-reset-home-bgm = 重置开屏 / 主页音乐
custom-startup-bgm = 开屏背景音乐
custom-reset-startup-bgm = 重置开屏音乐
custom-default-music = 默认音乐
custom-section-crash = 自定义崩溃
custom-crash-title = 崩溃标题
custom-crash-code = 崩溃代码
custom-crash-reason = 崩溃原因
custom-trigger-crash = 触发自定义崩溃
custom-trigger-crash-sub = 使用上述设置触发崩溃
custom-trigger = 触发
custom-not-set = 未设置
custom-current-code = 当前代码: { $code }
custom-bg-set = 背景已设置
custom-bg-reset = 已重置背景
custom-bgm-set = 背景音乐已设置
custom-bgm-reset = 已重置背景音乐
custom-startup-bgm-set = 开屏音乐已设置
custom-startup-bgm-reset = 已重置开屏音乐
old-home-restart = 重启后生效
home-ui = 主页 UI
old-home-style = 旧主页风格
play-button-x = 游玩按钮 X
play-button-y = 游玩按钮 Y
menu-buttons-x = 菜单按钮 X
menu-buttons-y = 菜单按钮 Y
item-crash-btn = 整活 - 崩溃按钮
item-crash-btn-sub = 点击后触发崩溃页面(错误代码951)
item-crash = 崩溃
custom-tab = 自定义

tutorial = 新手教程
tutorial-desc = 点击进入内置教学谱面
tutorial-loading = 加载中...
tutorial-start = 开始教程
tutorial-load-failed = 加载新手教程失败

about-update-log =
  版本号：V1.3.3
  更新内容：
  1.修复一点点bug
  2.又加了一点点神秘的设置
  3.pc端有神秘的Debug东西

  原版开发：Prpr,Phira
  运营维护：Lute_Rencai
  测试人员：
  洋洋酱～
  几何邻屿
  DVD
  开发:
  Lute_Rencai
  几何邻屿
  bug质检员:
  几何邻屿
  Jungle
  美术:
  几何邻屿
  Lute_Rencai
  Blue Archive

  如果你是从其他地方下载,请加入
  Phira-Vrenxz/Phi Launcher官方群
  qq:1103288774

# Phira-Vrenxz settings
settings-console-log = 控制台日志
settings-console-log-desc = 开启后显示 cmd 控制台窗口，用于查看调试日志
settings-vsync = 垂直同步
settings-vsync-desc = 开启后帧率限制为显示器刷新率，关闭可提升帧率
settings-open = 打开

# Built-in home background (Phira-Vrenxz): which bundled background the home page uses
item-home-bg = 内置主页背景
item-home-bg-default = 默认
item-home-bg-one = 背景一
item-home-bg-two = 背景二
item-home-bg-three = 背景三
item-home-bg-old = 旧版背景
item-home-bg-loginbg = WhatdoyouwantmorethanaHappyending

custom-section-font = 字体
item-font = 字体
item-font-sub = 切换界面使用的字体
item-import-font = 导入字体
item-import-font-sub = 导入 TTF/OTF 字体文件，导入后可随时切换
font-default = 默认
font-import = 导入
font-switched = 字体已切换
font-switch-failed = 字体切换失败

migrate-settings-item = 同步旧版本数据
migrate-settings-item-sub = 从旧版 PhirLie / Phira-Vrenxz 的 data 目录导入（自动提示只在第一次同步前出现）
migrate-settings-now = 立即同步

# Character customization (Phira-Vrenxz)
custom-character-name = 角色名字
custom-character-illust = 角色立绘
custom-character-no-illust = 暂无立绘
custom-character-reset = 重置角色自定义
custom-character-reset-done = 已重置角色自定义
custom-character-illust-set = 角色立绘已更换
custom-character-illust-failed = 更换角色立绘失败

visual = 视觉
color-palette-title = 调色板
color-palette-done = 完成
not-logged-in = 未登录
tab-general = 通用
tab-account = 账号
tab-audio = 音频
tab-chart = 谱面
tab-visual = 谱面UI
tab-custom = 自定义
tab-debug = 调试
tab-plugin = 插件
tab-about = 关于
row-delete = 删除
custom-select = 选择
custom-not-selected = 未选择
item-autoplay-text-default = 自动游玩
profile-id = ID：{ $id }
profile-id-none = ID：--

item-indicator-enabled = 准度指示器
item-indicator-enabled-sub = 开启后显示底部中央误差刻度条
item-indicator-delay = 准度指示器延迟
item-indicator-delay-sub = 冰与火：±220ms；Phigros：±80ms Perfect / ±160ms Good
item-indicator-color = 指示器颜色
item-indicator-color-sub = 冰与火：内置固定色；Phira：取皮肤 yml
item-fps-pos = FPS 位置
item-fps-pos-sub = 左上 / 右上 / 左下 / 右下
item-fps-limit = 帧率上限
item-fps-limit-sub = 0 = 不限制；建议 60/120/144

# Phira-Vrenxz settings extra localization (no hardcoded Chinese)
sx-logout = 登出
sx-audio-troubleshooting = 音频问题疑难解答
sx-watch-tutorial = 观看教程
sx-open-source-licenses = 开源许可证
sx-privacy-policy = 隐私政策
sx-about-footer = PHIRA-VRENXZ  v{ $version }  ·  第三方非官方客户端
sx-logged-out = 已退出登录
sx-audio-tip = 音频问题：请先确认系统输出设备与游戏内音量
sx-privacy-note = 本客户端不收集任何个人信息
sx-plugin-import-failed = 插件导入失败
sx-upload-title = 上传成绩
sx-upload-body = 是否打开上传成绩？
sx-upload-warning = 打开后被封后果自负。
sx-cancel = 取消
sx-confirm-enable = 确认开启
sx-upload-score = 上传成绩
sx-upload-score-sub = 开启后成绩将上传至服务器，被封后果自负
sx-lan = 局域网联机
sx-lan-sub = 实验性：同一局域网内建房/加入，支持传谱与准备
sx-enter = 进入
sx-data-transfer = 数据传输
sx-data-transfer-sub = 导出/导入 .pfd 数据包，或经局域网跨设备传输
sx-adofai = 冰与火
sx-pos-tl = 左上
sx-pos-tr = 右上
sx-pos-bl = 左下
sx-pos-br = 右下
sx-builtin = 内置
sx-custom = 自定义
sx-hp-max = 自定义 HP 上限
sx-hp-max-sub = 谱面 HP 选择“自定义”时使用
sx-hp-bad-only = 自定义 HP 只扣 Bad/Miss
sx-hp-bad-only-sub = 关闭时非 Perfect 都会扣 1
sx-record-replay = 记录本地回放
sx-record-replay-sub = 游玩结束后把触点时间轴保存到 replays/
sx-exercise-auto = 练习自动提速
sx-exercise-auto-sub = Exercise 每轮结束自动提高流速
sx-exercise-step = 每轮提速
sx-exercise-step-sub = 自动提速每次增加的倍率
sx-auto-retry = 自动重试次数
sx-auto-retry-sub = HP 失败后自动从失败点前重开；0 为关闭
sx-retry-count = { $n } 次
sx-retry-rewind = 重开回退
sx-retry-rewind-sub = 自动重试时从失败前多少秒开始
sx-judge-preset = 判定预设
sx-judge-preset-sub = 内置 / Phigros / Phira / 自定义窗口
sx-judge-perfect = Perfect 判定窗口
sx-judge-perfect-sub = 自定义 Perfect 判定范围（毫秒）
sx-judge-good = Good 判定窗口
sx-judge-good-sub = 自定义 Good 判定范围（毫秒）
sx-judge-bad = Bad 判定窗口
sx-judge-bad-sub = 自定义 Bad 判定范围（毫秒）
sx-judge-late = 晚按补偿
sx-judge-late-sub = 从 Late 误差中扣除的毫秒数
sx-judge-pp = Perfect+ 阈值
sx-judge-pp-sub = 误差不超过该值的 Perfect 计为 Perfect+；0 关闭
sx-judge-detailed = 细致判定
sx-judge-detailed-sub = Good 再分为 Great / Ok / Meh 统计
sx-judge-great = Great 阈值
sx-judge-ok = Ok 阈值
sx-judge-meh = Meh 阈值
sx-noise-enabled = 噪域开关
sx-noise-enabled-sub = 关闭后谱面噪域既不显示也不拦截触摸
sx-noise-color = 噪域颜色
sx-noise-color-sub = 十六进制色值，例如 7A003A
sx-noise-demo = 噪域演示
sx-noise-demo-sub = 仿Phigros使用噪点纹理；纯色块为低配色块
sx-noise-demo-phigros = 仿Phigros
sx-noise-demo-solid = 纯色块
sx-noise-glitch = 噪域 Glitch
sx-noise-glitch-sub = 启用后噪域边缘带故障错位效果
sx-noise-opacity = 噪域透明度
sx-dynamic-bg = 动态背景
sx-dynamic-bg-sub = 曲绘代表色流体；动态亮度会跟随音乐能量
sx-off = 关闭
sx-static-brightness = 静态亮度
sx-dynamic-brightness = 动态亮度
sx-watermark = 版本水印
sx-watermark-sub = 左上角显示游戏版本信息
sx-chart-info = 难度和谱面名称
sx-chart-info-sub = 结算与游戏内显示难度与谱面名
sx-replay-chart-missing = 找不到回放对应谱面，请重新导入
sx-replay-deleted = 回放已删除
sx-play = 播放
sx-edit = 编辑
sx-edit-name = 编辑：{ $name }
sx-delete-name = 删除：{ $name }
sx-replay-cleared = 回放已清空
sx-replay-exported = 回放已导出到 export/replays
sx-note-debug = 音符调试
sx-note-debug-sub = 显示当前时间附近音符的线号/ID/时间/类型
sx-combo-debug = Combo 文本调试
sx-combo-debug-sub = 在游玩界面显示 Combo 文本调试信息
sx-replay-folder = 打开回放目录
sx-replay-folder-sub = 在资源管理器中查看 replays/
sx-replay-clear = 清空回放
sx-replay-clear-sub = 删除 replays/ 下全部 JSON 回放
sx-clear = 清空
sx-replay-export = 导出回放
sx-replay-export-sub = 复制到 export/replays
sx-export = 导出
sx-keydisplay-missing = 按键显示插件未安装
sx-plugins-rescanned = 已重扫插件目录（{ $n } 个插件）
sx-plugin-deleted = 已删除插件：{ $name }
sx-plugin-import = 导入插件
sx-plugin-import-sub = 支持 .zip / .zips / .pvn 插件包，导入后自动刷新
sx-select-file = 选择文件
sx-plugin-rescan = 重扫插件目录
sx-plugin-rescan-sub = 手动往 plugins/ 目录放过插件后点这里
sx-plugin-entry-error =  · 入口异常
sx-toggle-on = 开
sx-toggle-off = 关
sx-no-plugins = 还没有插件
sx-no-plugins-sub = 点上面的「导入插件」选 .zip / .zips / .pvn 包
sx-home-video = 主页动态背景
sx-home-video-reset = 清除动态背景
sx-home-video-reset-done = 已清除主页动态背景
sx-home-video-set = 主页动态背景已导入
sx-plugin-imported = 已导入插件：{ $name } v{ $version }
sx-account-switch = 切换账号
sx-account-add = 登录其他账号
sx-account-current = 当前
sx-account-switching = 正在切换账号…
sx-account-switched = 已切换账号
sx-account-switch-failed = 切换账号失败
sx-account-hint = 点账号直接切换，× 从列表移除
sx-apply = 确定

