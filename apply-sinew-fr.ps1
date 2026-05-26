param(
  [string]$SourceDir = $PSScriptRoot,
  [string]$PatchPath = (Join-Path $PSScriptRoot "sinew-fr.patch"),
  [switch]$UpdateSource,
  [switch]$BuildFrontend,
  [switch]$BuildTauri
)

$ErrorActionPreference = "Stop"

function Run($Command, $Arguments, $WorkingDirectory = $PWD.Path) {
  Write-Host "> $Command $Arguments" -ForegroundColor DarkGray
  $process = Start-Process -FilePath $Command -ArgumentList $Arguments -WorkingDirectory $WorkingDirectory -NoNewWindow -Wait -PassThru
  if ($process.ExitCode -ne 0) {
    throw "Command failed with exit code $($process.ExitCode): $Command $Arguments"
  }
}

function Test-GitApply($Arguments, $WorkingDirectory) {
  $process = Start-Process -FilePath "git" -ArgumentList $Arguments -WorkingDirectory $WorkingDirectory -NoNewWindow -Wait -PassThru -RedirectStandardError ([System.IO.Path]::GetTempFileName()) -RedirectStandardOutput ([System.IO.Path]::GetTempFileName())
  return $process.ExitCode -eq 0
}

if (!(Test-Path $PatchPath)) {
  throw "Patch introuvable: $PatchPath"
}

if (!(Test-Path $SourceDir)) {
  Write-Host "Clonage de Sinew dans $SourceDir" -ForegroundColor Cyan
  Run "git" @("clone", "https://github.com/Paseru/sinew", $SourceDir)
}

if (!(Test-Path (Join-Path $SourceDir ".git"))) {
  throw "SourceDir n'est pas un dépôt Git: $SourceDir"
}

if ($UpdateSource) {
  Write-Host "Mise à jour du dépôt source Sinew" -ForegroundColor Cyan
  Run "git" @("fetch", "origin") $SourceDir
  Run "git" @("checkout", "main") $SourceDir
  Run "git" @("pull", "--ff-only", "origin", "main") $SourceDir
}

Write-Host "Application du patch français" -ForegroundColor Cyan
$mainPath = Join-Path $SourceDir "src\main.tsx"
$runtimePath = Join-Path $SourceDir "src\lib\frRuntime.ts"
$localePath = Join-Path $SourceDir "src\lib\locale.ts"
$settingsPath = Join-Path $SourceDir "src\components\SettingsPane.tsx"
$alreadyApplied =
  (Test-Path $runtimePath) -and
  (Test-Path $localePath) -and
  ((Get-Content -Raw $runtimePath) -match "sinew-fr-runtime") -and
  ((Get-Content -Raw $mainPath) -match './lib/frRuntime') -and
  ((Get-Content -Raw $settingsPath) -match "settings-pane__locale-switch")

if ($alreadyApplied) {
  Write-Host "Patch déjà appliqué." -ForegroundColor Yellow
} elseif (Test-GitApply @("apply", "--check", $PatchPath) $SourceDir) {
  Run "git" @("apply", "--whitespace=nowarn", $PatchPath) $SourceDir
  Write-Host "Patch appliqué." -ForegroundColor Green
} else {
  Write-Host "Application directe impossible, tentative en mode --3way..." -ForegroundColor Yellow
  Run "git" @("apply", "--whitespace=nowarn", "--3way", $PatchPath) $SourceDir
  Write-Host "Patch appliqué en mode 3-way. Vérifiez 'git status'." -ForegroundColor Green
}

if ($BuildFrontend -or $BuildTauri) {
  Write-Host "Installation des dépendances Node" -ForegroundColor Cyan
  if (Test-Path (Join-Path $SourceDir "package-lock.json")) {
    Run "npm" @("ci") $SourceDir
  } else {
    Run "npm" @("install") $SourceDir
  }

  Write-Host "Build frontend" -ForegroundColor Cyan
  Run "npm" @("run", "build") $SourceDir
}

if ($BuildTauri) {
  if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Rust/Cargo est introuvable. Installez Rust puis relancez avec -BuildTauri."
  }
  Write-Host "Build Tauri" -ForegroundColor Cyan
  Run "npm" @("run", "tauri", "build") $SourceDir
  Write-Host "Build terminé. Cherchez l'installateur dans src-tauri\target\release\bundle." -ForegroundColor Green
}

Write-Host "Terminé." -ForegroundColor Green
Write-Host "Langue dans l'app: Settings > About > Language (English / Français)" -ForegroundColor DarkGray
Write-Host "Contrôle manuel: localStorage.setItem('sinew.locale','fr') ou localStorage.setItem('sinew.locale','en')" -ForegroundColor DarkGray
