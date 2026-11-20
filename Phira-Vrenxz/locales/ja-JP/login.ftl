login = ログイン
login-sub = ログインしてアクティブオンラインコミュニティに参加する
back-login = ログインに戻る
register = 登録
email = メールアドレス
username = ユーザー名
password = パスワード
name-length-req = 名前は{ $min }〜{ $max }字以内にしてください
name-has-illegal-char = ユーザー名に不正な文字が含まれています
pwd-length-req = パスワードは{ $min }〜{ $max }字以内にしてください
illegal-email = 無効なメールアドレスです
action-success =
    { $action ->
        [login] ログインに成功しました
        [register] 登録に成功しました
       *[other] _
    }
action-failed =
    { $action ->
        [login] ログインに失敗しました
        [register] 登録に失敗しました
       *[other] _
    }
email-sent = 確認メールを送信しました

startup-select-language = 言語を選択
startup-tap-to-continue = タップして続行
startup-tap-to-skip = Tap to skip
startup-tip = ヒント：{ $tip }

startup-load-failed = 起動に失敗しました
startup-loading = 読み込み中...


startup-init-failed = 初期化に失敗

srv-official = 公式

srv-local = ローカル
forget-password = Forgot password?
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
startup-chart-resource = Chart Resources
startup-expansion-package = Expansion Package
startup-preparing-extract = Preparing to extract resources...
startup-extracting = Extracting { $desc }...
startup-extract-done = Extraction complete
startup-extract-done-partial = Extraction complete (partial failure)
hykb-choice-sub =
  This is your first time logging in with your 3839 Games account.
  If this is your first time playing, choose [Create new player data] to start.
  If you've played before and already have a Phira account, choose [Bind an existing Phira account] to link it.
