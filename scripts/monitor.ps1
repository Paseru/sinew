# monitor.ps1 - Monitors current rustc/cargo compile and finishes copy to Desktop.
$startTime = Get-Date
Write-Host "En attente de la fin de la compilation en arriere-plan (cargo/rustc)..." -ForegroundColor Cyan
while ($true) {
    $procs = Get-Process cargo, rustc -ErrorAction SilentlyContinue
    if (-not $procs) {
        Write-Host "Les processus cargo/rustc se sont arretes." -ForegroundColor Green
        break
    }
    $elapsed = New-TimeSpan -Start $startTime -End (Get-Date)
    $roundedSecs = [Math]::Round($elapsed.TotalSeconds)
    Write-Host "Toujours en cours... (${roundedSecs}s ecoulees)" -ForegroundColor Yellow
    Start-Sleep -Seconds 10
}

Write-Host "=== Recherche de l'installateur compile ===" -ForegroundColor Cyan
$searchPaths = @(
    "C:\Users\julie\AppData\Local\Temp\sinew-cargo-target\release\bundle\nsis",
    "target\release\bundle\nsis",
    "src-tauri\target\release\bundle\nsis",
    "scripts\..\target\release\bundle\nsis",
    "scripts\..\src-tauri\target\release\bundle\nsis"
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
    Write-Host "Dossier de bundle NSIS introuvable. On relance une compilation propre..." -ForegroundColor Magenta
    npx tauri build -b nsis
    if ($LASTEXITCODE -ne 0) {
        Write-Error "La compilation Tauri a echoue."
        Exit $LASTEXITCODE
    }
    foreach ($path in $searchPaths) {
        if ($path -and (Test-Path $path)) {
            $nsisDir = $path
            break
        }
    }
}

if ($nsisDir) {
    $exeFiles = Get-ChildItem -Path $nsisDir -Filter "*.exe"
    if ($exeFiles.Count -eq 0) {
        Write-Host "Aucun fichier .exe trouve. On tente de recompiler propre..." -ForegroundColor Magenta
        npx tauri build -b nsis
        $exeFiles = Get-ChildItem -Path $nsisDir -Filter "*.exe"
    }

    if ($exeFiles.Count -gt 0) {
        $exeFile = $exeFiles | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        Write-Host "Trouve : $($exeFile.FullName) (Modifie le : $($exeFile.LastWriteTime))" -ForegroundColor Green

        $desktopPath = [Environment]::GetFolderPath([Environment+SpecialFolder]::Desktop)
        if (-not $desktopPath) { $desktopPath = Join-Path $env:USERPROFILE "OneDrive\Bureau" }
        if (-not (Test-Path $desktopPath)) { $desktopPath = Join-Path $env:USERPROFILE "Desktop" }

        Write-Host "Destination : $desktopPath" -ForegroundColor Green
        $destFile = Join-Path $desktopPath $exeFile.Name
        Copy-Item -Path $exeFile.FullName -Destination $destFile -Force

        Write-Host "=== Succes ! ===" -ForegroundColor Green
        Write-Host "L'installateur a ete copie avec succes sur le bureau :" -ForegroundColor Green
        Write-Host $destFile -ForegroundColor Yellow
    } else {
        Write-Error "Impossible de trouver ou compiler le fichier .exe"
        Exit 1
    }
} else {
    Write-Error "Impossible de localiser le repertoire de compilation."
    Exit 1
}
