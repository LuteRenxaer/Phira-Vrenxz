login = Login
login-sub = Login untuk menggunakan fitur-fitur online dan berinteraksi dengan player-player lain
back-login = Balik ke Login
register = Register
email = Email
username = Username
password = Password
name-length-req = Panjang username harus di antara { $min } dan { $max } karakter.
name-has-illegal-char = Username mengandung karakter illegal.
pwd-length-req = Panjang password harus di antara { $min } dan { $max } karakter.
illegal-email = Email tidak benar.
action-success =
    { $action ->
        [login] Log in sukses
        [register] Register sukses
       *[other] _
    }
action-failed =
    { $action ->
        [login] Gagal untuk log in
        [register] Gagal untuk register
       *[other] _
    }
email-sent = Email verifikasi telah dikirim, mohon cek email anda untuk memverifikasi.

startup-select-language = Pilih Bahasa
startup-tap-to-continue = Ketuk untuk lanjutkan
startup-tap-to-skip = Tap to skip
startup-tip = Tips: { $tip }

startup-load-failed = Gagal memulai
startup-loading = Memuat...


srv-official = Resmi

srv-local = Lokal
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
startup-init-failed = Initialization failed
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
