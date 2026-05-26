$ErrorActionPreference = "Stop"
$ScriptDir = $PSScriptRoot
if (!$ScriptDir) { $ScriptDir = $pwd.Path }

$ManifestPath = Join-Path $ScriptDir "com.sinew.chrome_bridge.json"
$HostScriptPath = Join-Path $ScriptDir "native_host.bat"

Write-Host "1/3 Mise a jour du manifest JSON local..."
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

Write-Host "2/3 Configuration de la base de registre Windows..."
$RegPath = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.sinew.chrome_bridge"
if (!(Test-Path $RegPath)) {
    New-Item -Path $RegPath -Force | Out-Null
}
Set-ItemProperty -Path $RegPath -Name "(default)" -Value $ManifestPath
Write-Host "Cle de registre configuree."

Write-Host "3/3 Configuration de native_host.bat..."
$BatPath = Join-Path $ScriptDir "native_host.bat"
$BatContent = "@echo off`r`nnode `"%~dp0server.js`" --native"
[System.IO.File]::WriteAllText($BatPath, $BatContent, [System.Text.Encoding]::UTF8)
Write-Host "Script d'appel configure."

Write-Host "SUCCESS: Sinew Chrome Bridge est enregistre !"
Write-Host "Dossier de l'extension a charger dans Chrome :"
Write-Host $ScriptDir
