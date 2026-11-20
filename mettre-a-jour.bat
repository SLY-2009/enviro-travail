@echo off
rem Glisse le fichier .zip recu de Claude sur ce fichier : il copie tout au bon endroit puis envoie sur GitHub.
cd /d "%~dp0"
if "%~1"=="" (
  echo Glisse le fichier .zip recu sur ce fichier, ne le double-clique pas.
  pause
  exit /b
)
if not exist ".git" (
  echo Ce fichier doit etre dans le dossier du projet enviro-travail-app, a cote de package.json.
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
set N=0
for /r "%T%" %%f in (index.html bridge.js) do (
  copy /Y "%%f" "ui\" >nul
  echo   copie : %%~nxf  vers  ui\
  set /a N+=1
)
for /r "%T%" %%f in (main.rs google.rs) do (
  copy /Y "%%f" "src-tauri\src\" >nul
  echo   copie : %%~nxf  vers  src-tauri\src\
  set /a N+=1
)
for /r "%T%" %%f in (release.yml) do (
  if not exist ".github\workflows" mkdir ".github\workflows"
  copy /Y "%%f" ".github\workflows\" >nul
  echo   copie : %%~nxf  vers  .github\workflows\
  set /a N+=1
)
if %N%==0 (
  echo Aucun fichier reconnu dans ce zip : rien n'a ete envoye.
  pause
  exit /b
)
echo.
echo %N% fichier(s) copie(s). Envoi sur GitHub...
git add -A
git commit -m "MAJ %date% %time%"
git push
echo.
echo Envoye. La construction prend 15 a 25 minutes (onglet Actions du depot GitHub).
echo Ensuite, ouvre simplement l'application : elle se met a jour toute seule.
pause
