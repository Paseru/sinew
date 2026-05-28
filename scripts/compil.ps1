# compil.ps1 - Compiles only the NSIS (.exe) bundle and copies it to OneDrive Desktop

Write-Host "=== 1. Lancement de la compilation Tauri (NSIS uniquement) ===" -ForegroundColor Cyan
npx tauri build -b nsis
if ($LASTEXITCODE -ne 0) {
    Write-Error "La compilation Tauri a échoué."
    Exit $LASTEXITCODE
}

Write-Host "=== 2. Recherche de l'installateur compilé ===" -ForegroundColor Cyan
$nsisDir = Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle\nsis"
if (-not (Test-Path $nsisDir)) {
    $nsisDir = Resolve-Path "src-tauri/target/release/bundle/nsis" -ErrorAction SilentlyContinue
}

if (-not $nsisDir -or -not (Test-Path $nsisDir)) {
    Write-Error "Impossible de trouver le dossier de bundle NSIS : $nsisDir"
    Exit 1
}

$exeFiles = Get-ChildItem -Path $nsisDir -Filter "*.exe"
if ($exeFiles.Count -eq 0) {
    Write-Error "Aucun fichier .exe n'a été trouvé dans $nsisDir"
    Exit 1
}

# Get the most recent exe or the first one
$exeFile = $exeFiles | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Write-Host "Trouvé : $($exeFile.FullName)" -ForegroundColor Green

Write-Host "=== 3. Copie vers le Bureau OneDrive ===" -ForegroundColor Cyan
$desktopPath = [Environment]::GetFolderPath([Environment+SpecialFolder]::Desktop)
if (-not $desktopPath) {
    $desktopPath = Join-Path $env:USERPROFILE "OneDrive\Bureau"
}

if (-not (Test-Path $desktopPath)) {
    $desktopPath = Join-Path $env:USERPROFILE "Desktop"
}

Write-Host "Destination : $desktopPath" -ForegroundColor Green

$destFile = Join-Path $desktopPath $exeFile.Name
Copy-Item -Path $exeFile.FullName -Destination $destFile -Force

Write-Host "=== Succès ! ===" -ForegroundColor Green
Write-Host "L'installateur a été copié avec succès sur le bureau :" -ForegroundColor Green
Write-Host $destFile -ForegroundColor Yellow
