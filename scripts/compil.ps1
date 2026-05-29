# compil.ps1 - Compiles only the NSIS (.exe) bundle and copies it to OneDrive Desktop

Write-Host "=== 1. Lancement de la compilation Tauri (NSIS uniquement) ===" -ForegroundColor Cyan
npx tauri build -b nsis
if ($LASTEXITCODE -ne 0) {
    Write-Error "La compilation Tauri a echoue."
    Exit $LASTEXITCODE
}

Write-Host "=== 2. Recherche de l'installateur compile ===" -ForegroundColor Cyan
$searchPaths = @(
    "C:\Users\julie\AppData\Local\Temp\sinew-cargo-target\release\bundle\nsis",
    "target\release\bundle\nsis",
    "src-tauri\target\release\bundle\nsis",
    (Join-Path $PSScriptRoot "..\target\release\bundle\nsis"),
    (Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle\nsis")
)

$nsisDir = $null
foreach ($path in $searchPaths) {
    if ($path -and (Test-Path $path)) {
        $nsisDir = $path
        Write-Host "Dossier d'installateurs trouve : $nsisDir" -ForegroundColor Green
        break
    }
}

if (-not $nsisDir) {
    Write-Error "Impossible de trouver le dossier de bundle NSIS dans les chemins de recherche."
    Exit 1
}

$exeFiles = Get-ChildItem -Path $nsisDir -Filter "*.exe"
if ($exeFiles.Count -eq 0) {
    Write-Error "Aucun fichier .exe n'a ete trouve dans $nsisDir"
    Exit 1
}

# Get the most recent exe or the first one
$exeFile = $exeFiles | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Write-Host "Trouve : $($exeFile.FullName) (Modifie le : $($exeFile.LastWriteTime))" -ForegroundColor Green

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

Write-Host "=== Succes ! ===" -ForegroundColor Green
Write-Host "L'installateur a ete copie avec succes sur le bureau :" -ForegroundColor Green
Write-Host $destFile -ForegroundColor Yellow
