$ErrorActionPreference = "Stop"
$ScriptDir = $PSScriptRoot
if (!$ScriptDir) { $ScriptDir = $pwd.Path }

$ManifestPath = Join-Path $ScriptDir "com.sinew.chrome_bridge.json"
$HostScriptPath = Join-Path $ScriptDir "native_host.bat"

Write-Host "1/4 Mise a jour du manifest JSON local..."
if (Test-Path $ManifestPath) {
    $manifestContent = Get-Content -Raw -Path $ManifestPath
    $json = ConvertFrom-Json $manifestContent
    $json.path = $HostScriptPath
    $updatedJson = ConvertTo-Json $json -Depth 10
    [System.IO.File]::WriteAllText($ManifestPath, $updatedJson, [System.Text.Encoding]::UTF8)
    Write-Host "Manifest configure."
} else {
    Write-Error "Fichier manifest introuvable."
    exit 1
}

Write-Host "2/4 Configuration de la base de registre Windows..."
$RegPath = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.sinew.chrome_bridge"
if (!(Test-Path $RegPath)) {
    New-Item -Path $RegPath -Force | Out-Null
}
Set-ItemProperty -Path $RegPath -Name "(default)" -Value $ManifestPath
Write-Host "Cle de registre configuree."

Write-Host "3/4 Configuration du lanceur unifie run_sinew_bridge.bat..."
$NodePath = (Get-Command node).Source
$homeDir = [System.Environment]::GetFolderPath('UserProfile')
$PythonPath = "$homeDir\.gemini\antigravity\scratch\browser-use-env\Scripts\python.exe"
if (!(Test-Path $PythonPath)) {
    $PythonPath = (Get-Command python -ErrorAction SilentlyContinue).Source
}

$BridgeBatPath = Join-Path $ScriptDir "run_sinew_bridge.bat"
$BridgeBatContent = "@echo off`r`nstart /B `"`" `"$NodePath`" `"%~dp0server.js`"`r`n`"$PythonPath`" -m mcp_server_browser_use"
[System.IO.File]::WriteAllText($BridgeBatPath, $BridgeBatContent, [System.Text.Encoding]::UTF8)

# Configure also the standard native_host.bat
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
Write-Host "Dossier de l'extension a charger dans Chrome :"
Write-Host $ScriptDir
