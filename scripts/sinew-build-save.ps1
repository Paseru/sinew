param(
  [string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
  [string]$Message = "Update Julien custom Sinew options",
  [switch]$FullApp
)

$ErrorActionPreference = "Stop"

function Run($Command, [string[]]$Arguments) {
  Write-Host "> $Command $($Arguments -join ' ')" -ForegroundColor DarkGray
  & $Command @Arguments
  if ($LASTEXITCODE -ne 0) { throw "Échec: $Command $($Arguments -join ' ')" }
}

Set-Location $Repo

Write-Host "1/4 Sync depuis GitHub..." -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "sinew-sync.ps1") -Repo $Repo -Branch main

Write-Host "2/4 Installation dépendances si besoin..." -ForegroundColor Cyan
if (!(Test-Path "node_modules")) {
  Run "npm" @("install")
}

Write-Host "3/4 Build..." -ForegroundColor Cyan
if ($FullApp) {
  if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Rust/Cargo manque. Installe Rust avant le build complet."
  }
  Run "npm" @("run", "tauri", "build")
} else {
  Run "npm" @("run", "build")
}

Write-Host "4/4 Commit + push..." -ForegroundColor Cyan
if (Test-Path "dist") { Remove-Item -Recurse -Force "dist" }
& (Join-Path $PSScriptRoot "sinew-save.ps1") -Repo $Repo -Branch main -Message $Message

Write-Host "OK terminé." -ForegroundColor Green
