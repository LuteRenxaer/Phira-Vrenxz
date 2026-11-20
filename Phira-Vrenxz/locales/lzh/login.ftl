
login = 登入
login-sub = 登入以入活躍之星穹列車
back-login = 返登入
register = 註冊

email = 電子郵箱
username = 用戶名
password = 密碼
forget-password = 忘密碼

name-length-req = 用戶名之長宜介於四至二十之間
name-has-illegal-char = 用戶名含非法之字
pwd-length-req = 密碼之長宜介於六至二十六之間
illegal-email = 郵箱不合法

action-success = { $action ->
  [login] 登入功矣
  [register] 註冊功矣
  [hykb-login] 3839 遊登入功矣
  *[other] _
}
action-failed = { $action ->
  [login] 登入敗矣
  [register] 註冊敗矣
  [hykb-login] 3839 遊登入敗矣
  *[other] _
}

email-sent = 驗證信息已發至郵箱，請驗後登入
hykb-login-cancelled = 3839 遊之登入已罷
login-method-title = 擇登入之法
login-method-email = 以郵箱登入
login-method-hykb = 以 3839 遊登入
login-method-recommended = 薦
hykb-choice-title = 歡迎至 Phira
hykb-choice-register = 新建玩家之檔
hykb-choice-claim = 綁既有 Phira 之帳
hykb-reg-name-prompt = 請輸入卿之名（{ $min }–{ $max } 字）。
hykb-reg-name-confirm = 確定
hykb-other-login-notice = 惟綁於 3839 遊之帳方可登入
hykb-other-login-not-bound = 此帳未綁 3839 遊，不可登入
hykb-bind-required-title = 須綁 3839 遊
hykb-bind-required = 此帳尚未綁 3839 遊，請綁之而後登入
hykb-bind-required-confirm = 綁 3839 遊
startup-select-language = 擇語
startup-tap-to-continue = 觸以始
startup-tap-to-skip = 觸以跳
startup-tip = 提示：{ $tip }
startup-load-failed = 開場載敗
startup-loading = 載中…
startup-init-failed = 初始化敗
startup-chart-resource = 譜面之資
startup-expansion-package = 擴充之包
startup-preparing-extract = 備解資源…
startup-extracting = 正在解 { $desc }…
startup-extract-done = 解畢
startup-extract-done-partial = 解畢（有失）
srv-official = 官方
srv-local = Local
hykb-choice-sub =
  This is your first time logging in with your 3839 Games account.
  If this is your first time playing, choose [Create new player data] to start.
  If you've played before and already have a Phira account, choose [Bind an existing Phira account] to link it.
