param(
  [string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
  [string]$Branch = "main",
  [string]$Message = "Update Julien custom Sinew options"
)

$ErrorActionPreference = "Stop"

function Run([string[]]$GitArgs) {
  Write-Host "> git $($GitArgs -join ' ')" -ForegroundColor DarkGray
  & git @GitArgs
  if ($LASTEXITCODE -ne 0) { throw "git $($GitArgs -join ' ') failed" }
}

Set-Location $Repo

if (!(Test-Path ".git")) { throw "Pas un dépôt Git: $Repo" }

Write-Host "Sauvegarde des changements..." -ForegroundColor Cyan

Run @("checkout", $Branch)

# Évite de pousser les artefacts locaux inutiles.
if (Test-Path "dist") { Remove-Item -Recurse -Force "dist" }

$status = git status --porcelain
if ([string]::IsNullOrWhiteSpace($status)) {
  Write-Host "Rien à sauvegarder." -ForegroundColor Yellow
} else {
  Run @("add", "-A")
  Run @("commit", "-m", $Message)
}

# Push vers ton fork.
Run @("push", "-u", "origin", $Branch)

# Copie de la base de données vers OneDrive/Documents
$dbName = "desktop-state.sqlite3"
$localDbPath = "$env:USERPROFILE\AppData\Local\hyrak\sinew\data\$dbName"
$onedriveDbDir = Join-Path ([System.Environment]::GetFolderPath('MyDocuments')) "Sinew"
$onedriveDbPath = Join-Path $onedriveDbDir $dbName

if (Test-Path $localDbPath) {
  Write-Host "Sauvegarde de la base de données vers OneDrive/Documents/Sinew..." -ForegroundColor Cyan
  if (!(Test-Path $onedriveDbDir)) {
    New-Item -ItemType Directory -Path $onedriveDbDir -Force | Out-Null
  }
  Copy-Item -Path $localDbPath -Destination $onedriveDbPath -Force
  Write-Host "Base de données sauvegardée dans OneDrive." -ForegroundColor Green
  
  # Presse-papiers Partagé (Universal Clipboard)
  try {
    $clip = Get-Clipboard -ErrorAction SilentlyContinue
    if ($clip) {
      $onedriveClipPath = Join-Path $onedriveDbDir "clipboard.txt"
      [System.IO.File]::WriteAllText($onedriveClipPath, $clip, [System.Text.Encoding]::UTF8)
      Write-Host "Presse-papiers sauvegardé sur OneDrive." -ForegroundColor Green
    }
  } catch {}
}

Write-Host "OK: changements et base de données poussés." -ForegroundColor Green
