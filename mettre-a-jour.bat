@echo off
rem Glisse le zip recu de Claude sur ce fichier. Il copie ce qui a change puis envoie sur GitHub.
cd /d "%~dp0"
if "%~1"=="" (
  echo Glisse le fichier .zip sur ce fichier, ne le double-clique pas.
  pause
  exit /b
)
if not exist ".git" (
  echo Ce fichier doit etre dans le dossier enviro-travail-app, a cote de package.json.
  pause
  exit /b
)
set "T=%TEMP%\maj-enviro"
if exist "%T%" rmdir /s /q "%T%"
powershell -NoProfile -Command "Expand-Archive -LiteralPath '%~1' -DestinationPath '%T%' -Force"
if not exist "%T%" (
  echo Impossible d'ouvrir ce zip.
  pause
  exit /b
)
set FOUND=0
set CHG=0
set NAT=0
call :cp app.html "ui\app.html" 0
call :cp loader-index.html "ui\index.html" 1
call :cp main.rs "src-tauri\src\main.rs" 1
call :cp google.rs "src-tauri\src\google.rs" 1
call :cp release.yml ".github\workflows\release.yml" 1
if %FOUND%==0 (
  echo Aucun fichier reconnu dans ce zip : rien n'a ete envoye.
  pause
  exit /b
)
echo.
if %CHG%==0 (
  echo Tout etait deja a jour : rien a envoyer.
  pause
  exit /b
)
echo %CHG% fichier^(s^) modifie^(s^). Envoi sur GitHub...
git add -A
git commit -m "MAJ %date% %time%"
git push
echo.
if %NAT%==0 (
  echo Envoye. PAS de construction a attendre : ferme puis rouvre l'application.
) else (
  echo Envoye. Le code de l'application a change : construction de 15 a 25 minutes
  echo ^(onglet Actions sur GitHub^). Ensuite ouvre simplement l'application.
)
pause
exit /b

:cp
set "SRC="
for /r "%T%" %%f in (%~1) do if exist "%%f" set "SRC=%%f"
if not defined SRC exit /b
set /a FOUND+=1
if exist "%~2" (
  fc /b "%SRC%" "%~2" >nul 2>&1
  if not errorlevel 1 (
    echo   deja a jour : %~1
    exit /b
  )
)
for %%d in ("%~2") do if not exist "%%~dpd" mkdir "%%~dpd"
copy /Y "%SRC%" "%~2" >nul
echo   copie : %~1
set /a CHG+=1
if "%~3"=="1" set /a NAT+=1
exit /b
