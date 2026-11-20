
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
  *[other] _
}
action-failed = { $action ->
  [login] 登入敗矣
  [register] 註冊敗矣
  *[other] _
}

email-sent = 驗證信息已發至郵箱，請驗後登入
hykb-login-cancelled = 3839 Games login cancelled.
login-method-title = Choose login method
login-method-email = Log in with email
login-method-hykb = Log in with 3839 Games
login-method-recommended = Recommended
hykb-choice-title = Welcome to Phira
hykb-choice-register = Create new player data
hykb-choice-claim = Bind an existing Phira account
hykb-reg-name-prompt = Enter your username ({ $min }-{ $max } characters).
hykb-reg-name-confirm = Confirm
hykb-other-login-notice = Only an account bound to a 3839 Games account can be used to log in.
hykb-other-login-not-bound = This account is not bound to a 3839 Games account and cannot log in.
hykb-bind-required-title = 3839 Games binding required
hykb-bind-required = This account is not bound to a 3839 Games account yet. Bind one to log in.
hykb-bind-required-confirm = Bind 3839 Games
startup-select-language = Select Language
startup-tap-to-continue = Tap to start
startup-tap-to-skip = Tap to skip
startup-tip = Tip: { $tip }
startup-load-failed = Startup failed
startup-loading = Loading...
startup-init-failed = Initialization failed
startup-chart-resource = Chart Resources
startup-expansion-package = Expansion Package
startup-preparing-extract = Preparing to extract resources...
startup-extracting = Extracting { $desc }...
startup-extract-done = Extraction complete
startup-extract-done-partial = Extraction complete (partial failure)
srv-official = Official
srv-local = Local
hykb-choice-sub =
  This is your first time logging in with your 3839 Games account.
  If this is your first time playing, choose [Create new player data] to start.
  If you've played before and already have a Phira account, choose [Bind an existing Phira account] to link it.
