@echo off
title Sinew Dev Environment
cd /d "%~dp0"
echo ==================================================
echo   DEMARRAGE DE L'ENVIRONNEMENT DE DEV SINEW
echo ==================================================
echo.

if not exist node_modules (
    echo [1/3] Installation des dependances npm...
    call npm install
    if errorlevel 1 (
        echo [ERREUR] Impossible d'installer les dependances npm.
        pause
        exit /b 1
    )
) else (
    echo [1/3] Dependances npm deja installees.
)

echo.
echo [2/3] Preparation des sidecars...
call npm run prepare-sidecars
if errorlevel 1 (
    echo [ATTENTION] La preparation des sidecars a signale une erreur. Tentative de continuation...
)

echo.
echo [3/3] Lancement de l'application en mode dev (Tauri)...
call npm run tauri dev

if errorlevel 1 (
    echo.
    echo [ERREUR] L'application s'est arretee avec une erreur.
    pause
)
