login = Giriş Yap
login-sub = Aktif çevrimiçi topluluğumuzla etkileşmek için giriş yap
back-login = Girişe Dön
register = Kaydol
email = E-Posta Adresi
username = Kullanıcı Adı
password = Şifre
name-length-req = Kullanıcı adı uzunluğu { $min } ile { $max } karakter arasında olmalıdır
name-has-illegal-char = Kullanıcı adı geçersiz karakterler içeriyor
pwd-length-req = Şifre uzunluğu { $min } ile { $max } karakter arasında olmalıdır
illegal-email = Geçersiz E-Posta
action-success =
    { $action ->
        [login] Başarıyla giriş yapıldı
        [register] Başarıyla kaydolundu
       *[other] _
    }
action-failed =
    { $action ->
        [login] Giriş başarısız
        [register] Kayıt başarısız
       *[other] _
    }
email-sent = Bir doğrulama e-postası gönderildi. Lütfen hesabınızı doğrulayıp giriş yapın.

startup-select-language = Dil Seç
startup-tap-to-continue = Devam etmek için dokunun
startup-tap-to-skip = Tap to skip
startup-tip = İpucu: { $tip }

startup-load-failed = Başlatma başarısız
startup-loading = Yükleniyor...


srv-official = Resmi

srv-local = Yerel
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
