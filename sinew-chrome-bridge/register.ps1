$ErrorActionPreference = "Stop"
$ScriptDir = $PSScriptRoot
if (!$ScriptDir) { $ScriptDir = $pwd.Path }

$SourceManifestPath = Join-Path $ScriptDir "com.sinew.chrome_bridge.json"
$InstallDir = Join-Path $env:LOCALAPPDATA "Sinew\ChromeBridge"
$ManifestPath = Join-Path $InstallDir "com.sinew.chrome_bridge.json"
$HostScriptPath = Join-Path $ScriptDir "native-host-wrapper.exe"

if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Write-Host "1/4 Creation du manifest Native Host installe localement..."
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

Write-Host "2/4 Configuration de la base de registre Windows..."
$RegPath = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.sinew.chrome_bridge"
if (!(Test-Path $RegPath)) {
    New-Item -Path $RegPath -Force | Out-Null
}
Set-ItemProperty -Path $RegPath -Name "(default)" -Value $ManifestPath
Write-Host "Cle de registre configuree vers le manifest installe."

Write-Host "3/4 Configuration du lanceur MCP Node local run_sinew_bridge.bat..."
$NodePath = (Get-Command node).Source

$BridgeBatPath = Join-Path $ScriptDir "run_sinew_bridge.bat"
$BridgeBatContent = "@echo off`r`n`"$NodePath`" `"%~dp0mcp_server.js`""
[System.IO.File]::WriteAllText($BridgeBatPath, $BridgeBatContent, [System.Text.Encoding]::UTF8)

# Legacy convenience launcher only: Sinew should run mcp_server.js directly, Chrome should run native-host-wrapper.exe.
$BatPath = Join-Path $ScriptDir "native_host.bat"
$BatContent = "@echo off`r`n`"$NodePath`" `"%~dp0server.js`" --native"
[System.IO.File]::WriteAllText($BatPath, $BatContent, [System.Text.Encoding]::UTF8)
Write-Host "Lanceur configure."

Write-Host "4/4 Configuration du serveur MCP dans la base de donnees de Sinew..."
$PyScriptPath = Join-Path $ScriptDir "add_to_sinew.py"
if (Get-Command python -ErrorAction SilentlyContinue) {
    python $PyScriptPath
} else {
    Write-Host "Python introuvable. Veuillez executer 'python add_to_sinew.py' manuellement."
}

Write-Host "SUCCESS: Sinew Chrome Bridge et son serveur MCP sont enregistres et automatises !"
Write-Host "Manifest Native Host actif :"
Write-Host $ManifestPath
Write-Host "Dossier de l'extension a charger dans Chrome :"
Write-Host $ScriptDir
