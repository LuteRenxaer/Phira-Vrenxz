login = Login
login-sub = Login เพื่อใช้โหมด Online
back-login = กลับไป Login
register = สมัคร
email = Email
username = ชื่อผู้ใช้
password = รหัสผ่าน
name-length-req = ชื่อควรยาว { $min }-{ $max } ตัว
name-has-illegal-char = ชื่อผู้ใช้มีตัวที่ไม่อนุญาตให้ใช้
pwd-length-req = รหัสผ่านควรยาว { $min }-{ $max } ตัว
illegal-email = Illegal email
action-success =
    { $action ->
        [login] Login เรียบร้อย
        [register] สมัครเรียบร้อย
       *[other] _
    }
action-failed =
    { $action ->
        [login] ไม่สามารถ login ได้
        [register] ไม่สามารถสมัครได้
       *[other] _
    }
email-sent = รหัสยืนยันถูกส่งไปยัง email เรียบร้อย, กรุณายืนยันตัวตนเพื่อ login
forget-password = ลืมรหัสผ่าน?

startup-select-language = เลือกภาษา
startup-tap-to-continue = แตะเพื่อดำเนินการต่อ
startup-tap-to-skip = Tap to skip
startup-tip = เคล็ดลับ: { $tip }

startup-load-failed = เริ่มต้นล้มเหลว
startup-loading = กำลังโหลด...


srv-official = ทางการ

srv-local = ท้องถิ่น
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
