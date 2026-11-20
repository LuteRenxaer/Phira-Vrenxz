login = Нэвтрэх
login-sub = Идэвхтэй онлайн тоглоомчдын бүлэгт оролцохын тулд нэвтэрнэ үү
back-login = Нэвтрэх хэсэг рүү буцах
register = Бүртгүүлэх
email = Имэйл хаяг
username = Хэрэглэгчийн нэр
password = Нууц үг
name-length-req = Хэрэглэгчийн нэрийн урт { $min }-{ $max } тэмдэгт байх ёстой
name-has-illegal-char = Хэрэглэгчийн нэрэнд дэмжигдээгүй үсэг байна
pwd-length-req = Нууц үгийн урт { $min }-{ $max } тэмдэгт байх ёстой
illegal-email = Имэйл хаяг буруу байна
action-success =
    { $action ->
        [login] Амжилттай нэвтэрсэн
        [register] Амжилттай бүртгүүлсэн
       *[other] _
    }
action-failed =
    { $action ->
        [login] Нэвтрэхэд амжилтгүй боллоо
        [register] Бүртгүүлэхэд амжилтгүй боллоо
       *[other] _
    }
email-sent = Баталгаажуулах имэйл илгээгдсэн. Та баталгаажуулаад нэвтэрнэ үү.
forget-password = Нууц үг мартсан уу?

startup-select-language = Хэл сонгох
startup-tap-to-continue = Үргэлжлүүлэхийн тулд товч дарна уу
startup-tap-to-skip = Tap to skip
startup-tip = Зөвлөмж: { $tip }

startup-load-failed = Эхлүүлэлт амжилтгүй боллоо
startup-loading = Ачаалж байна...


srv-official = Албан

srv-local = Орон нутаг
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
