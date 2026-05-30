param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Sinew\ChromeBridge"),
    [switch]$SkipSinewDb
)

$ErrorActionPreference = "Stop"
$SourceDir = $PSScriptRoot
if (!$SourceDir) { $SourceDir = $pwd.Path }

$SourceManifestPath = Join-Path $SourceDir "com.sinew.chrome_bridge.json"
$ManifestPath = Join-Path $InstallDir "com.sinew.chrome_bridge.json"
$HostScriptPath = Join-Path $InstallDir "native-host-wrapper.exe"

if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Write-Host "1/5 Installation du runtime Chrome Bridge dans LOCALAPPDATA..."
$runtimeFiles = @(
    "native-host-wrapper.exe",
    "manifest.json",
    "background.js",
    "sinew_cursor.js",
    "popup.html",
    "popup.js",
    "icon-32.png",
    "icon-64.png",
    "icon-128.png"
)
foreach ($file in $runtimeFiles) {
    $source = Join-Path $SourceDir $file
    if (Test-Path $source) {
        Copy-Item -Path $source -Destination (Join-Path $InstallDir $file) -Force
    }
}

if (!(Test-Path $HostScriptPath)) {
    Write-Error "native-host-wrapper.exe introuvable dans $InstallDir"
    exit 1
}

Write-Host "2/5 Creation du manifest Native Host installe localement..."
if (Test-Path $SourceManifestPath) {
    $manifestContent = Get-Content -Raw -Path $SourceManifestPath
    $json = ConvertFrom-Json $manifestContent
    $json.path = $HostScriptPath
    $updatedJson = ConvertTo-Json $json -Depth 10
    [System.IO.File]::WriteAllText($ManifestPath, $updatedJson, [System.Text.Encoding]::UTF8)
    Write-Host "Manifest installe: $ManifestPath"
} else {
    Write-Error "Fichier manifest source introuvable."
    exit 1
}

Write-Host "3/5 Configuration de la base de registre Windows..."
$RegPath = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.sinew.chrome_bridge"
if (!(Test-Path $RegPath)) {
    New-Item -Path $RegPath -Force | Out-Null
}
Set-ItemProperty -Path $RegPath -Name "(default)" -Value $ManifestPath
Write-Host "Cle de registre configuree vers le manifest installe."

Write-Host "4/5 Configuration du serveur MCP dans la base de donnees de Sinew..."
if (!$SkipSinewDb) {
    $env:SINEW_CHROME_BRIDGE_DIR = $InstallDir
    $SinewExe = ""
    $PathsToCheck = @(
        (Join-Path $SourceDir "..\src-tauri\target\release\Sinew.exe"),
        (Join-Path $SourceDir "..\src-tauri\target\debug\Sinew.exe"),
        (Join-Path $env:LOCALAPPDATA "Programs\Sinew\Sinew.exe")
    )
    foreach ($p in $PathsToCheck) {
        if (Test-Path $p) {
            $SinewExe = $p
            break
        }
    }
    if (!$SinewExe) {
        $SinewExe = (Get-Command Sinew -ErrorAction SilentlyContinue).Source
    }
    
    if ($SinewExe) {
        Write-Host "Execution de l'enregistrement via $SinewExe..."
        & $SinewExe --register-chrome
    } else {
        Write-Warning "Sinew.exe introuvable : enregistrement du serveur MCP dans la base SQLite impossible."
    }
}
}

Write-Host "SUCCESS: Sinew Chrome Bridge et son serveur MCP sont enregistres et automatises !"
Write-Host "Manifest Native Host actif :"
Write-Host $ManifestPath
Write-Host "Runtime Chrome Bridge installe :"
Write-Host $InstallDir
Write-Host "Dossier de l'extension a charger dans Chrome :"
Write-Host $SourceDir
