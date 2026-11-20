@echo off
rem Envoie les modifications sur GitHub : la construction et la mise a jour se font ensuite toutes seules.
cd /d "%~dp0"
git add -A
git commit -m "MAJ %date% %time%"
git push
echo.
echo Envoye. La construction prend 15 a 25 minutes (onglet Actions du depot GitHub).
echo Ensuite, relance l'application : elle se met a jour toute seule.
pause
