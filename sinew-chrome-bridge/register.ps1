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
    "server.js",
    "mcp_server.js",
    "native-host-wrapper.exe",
    "package.json",
    "package-lock.json",
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

$sourceWs = Join-Path $SourceDir "node_modules\ws"
$targetNodeModules = Join-Path $InstallDir "node_modules"
$targetWs = Join-Path $targetNodeModules "ws"
if (Test-Path $sourceWs) {
    if (!(Test-Path $targetNodeModules)) { New-Item -ItemType Directory -Path $targetNodeModules -Force | Out-Null }
    Copy-Item -Path $sourceWs -Destination $targetNodeModules -Recurse -Force
} elseif (!(Test-Path $targetWs)) {
    Write-Host "node_modules/ws introuvable dans la source; installation npm minimale..."
    if (Get-Command npm -ErrorAction SilentlyContinue) {
        npm install --omit=dev --prefix $InstallDir | Out-Null
    } else {
        Write-Error "npm introuvable et node_modules/ws absent; installation impossible."
        exit 1
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

Write-Host "4/5 Configuration des lanceurs locaux..."
$NodePath = (Get-Command node).Source

$BridgeBatPath = Join-Path $InstallDir "run_sinew_bridge.bat"
$BridgeBatContent = "@echo off`r`n`"$NodePath`" `"%~dp0mcp_server.js`""
[System.IO.File]::WriteAllText($BridgeBatPath, $BridgeBatContent, [System.Text.Encoding]::UTF8)

# Legacy convenience launcher only: Sinew should run mcp_server.js directly, Chrome should run native-host-wrapper.exe.
$BatPath = Join-Path $InstallDir "native_host.bat"
$BatContent = "@echo off`r`n`"$NodePath`" `"%~dp0server.js`" --native"
[System.IO.File]::WriteAllText($BatPath, $BatContent, [System.Text.Encoding]::UTF8)
Write-Host "Lanceurs configures dans $InstallDir."

Write-Host "5/5 Configuration du serveur MCP dans la base de donnees de Sinew..."
if (!$SkipSinewDb) {
    $PyScriptPath = Join-Path $SourceDir "add_to_sinew.py"
    $env:SINEW_CHROME_BRIDGE_DIR = $InstallDir
    if (Get-Command python -ErrorAction SilentlyContinue) {
        python $PyScriptPath
    } else {
        Write-Host "Python introuvable. Veuillez executer 'python add_to_sinew.py' manuellement avec SINEW_CHROME_BRIDGE_DIR=$InstallDir."
    }
}

Write-Host "SUCCESS: Sinew Chrome Bridge et son serveur MCP sont enregistres et automatises !"
Write-Host "Manifest Native Host actif :"
Write-Host $ManifestPath
Write-Host "Runtime Chrome Bridge installe :"
Write-Host $InstallDir
Write-Host "Dossier de l'extension a charger dans Chrome :"
Write-Host $SourceDir
