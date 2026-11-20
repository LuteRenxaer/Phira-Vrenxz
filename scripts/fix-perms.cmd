@echo off
setlocal
set "ROOT=F:\RustProject\phirLie"
set "LOG=%ROOT%\tmp\fix-perms.log"
if not exist "%ROOT%\tmp" mkdir "%ROOT%\tmp"
echo ==== fix-perms %DATE% %TIME% ==== > "%LOG%"

echo [1/4] clear read-only attribute >> "%LOG%"
attrib -R /S /D "%ROOT%\*" >> "%LOG%" 2>&1

echo [2/4] reset ACL to inherited defaults >> "%LOG%"
icacls "%ROOT%" /reset /T /C /Q >> "%LOG%" 2>&1

echo [3/4] restore Medium integrity label >> "%LOG%"
icacls "%ROOT%" /setintegritylevel (OI)(CI)M /T /C /Q >> "%LOG%" 2>&1

echo [4/4] disable GamePP autostart task >> "%LOG%"
schtasks /Change /TN "GamePlusPlus" /Disable >> "%LOG%" 2>&1

echo ==== done ==== >> "%LOG%"
type "%LOG%"
endlocal
