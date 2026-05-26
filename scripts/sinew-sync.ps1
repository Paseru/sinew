param(
  [string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
  [string]$Branch = "main"
)

$ErrorActionPreference = "Stop"

function Run([string[]]$GitArgs) {
  Write-Host "> git $($GitArgs -join ' ')" -ForegroundColor DarkGray
  & git @GitArgs
  if ($LASTEXITCODE -ne 0) { throw "git $($GitArgs -join ' ') failed" }
}

Set-Location $Repo

if (!(Test-Path ".git")) { throw "Pas un dépôt Git: $Repo" }

Write-Host "Synchronisation du dépôt..." -ForegroundColor Cyan

# Si la branche n'existe pas encore, la créer.
$branches = git branch --list $Branch
if (!$branches) {
  Run @("checkout", "-b", $Branch)
} else {
  Run @("checkout", $Branch)
}

# Sauver temporairement les changements locaux avant pull/rebase.
$hasChanges = -not [string]::IsNullOrWhiteSpace((git status --porcelain))
if ($hasChanges) {
  Run @("stash", "push", "-u", "-m", "auto-sync-before-pull")
}

Run @("fetch", "--all", "--prune")

# 1. Pull depuis ton fork perso (origin) pour synchroniser tes deux PCs
Write-Host "Mise à jour depuis votre fork GitHub (origin)..." -ForegroundColor Cyan
Run @("pull", "--rebase", "origin", $Branch)

# 2. Mettre main/upstream à jour si possible.
$hasUpstream = -not [string]::IsNullOrWhiteSpace((git remote | Select-String "^upstream$"))
if ($hasUpstream) {
  Write-Host "Vérification des mises à jour officielles (upstream)..." -ForegroundColor Cyan
  if ($Branch -eq "main") {
    Run @("pull", "--rebase", "upstream", "main")
  } else {
    # Si on travaille sur une branche secondaire, on met d'abord à jour main via rebase
    Run @("checkout", "main")
    Run @("pull", "--rebase", "upstream", "main")
    # Puis on retourne sur la branche et on la rebase sur main
    Run @("checkout", $Branch)
    Run @("rebase", "main")
  }
}

if ($hasChanges) {
  $stashList = git stash list
  if ($stashList -match "auto-sync-before-pull") {
    Run @("stash", "pop")
  }
}

# 3. Synchroniser la base de données (conversations) depuis OneDrive/Documents si disponible
$dbName = "desktop-state.sqlite3"
$localDbDir = "$env:USERPROFILE\AppData\Local\hyrak\sinew\data"
$localDbPath = Join-Path $localDbDir $dbName
$onedriveDbDir = Join-Path ([System.Environment]::GetFolderPath('MyDocuments')) "Sinew"
$onedriveDbPath = Join-Path $onedriveDbDir $dbName

if (Test-Path $onedriveDbPath) {
  Write-Host "Base de données OneDrive détectée. Synchronisation locale..." -ForegroundColor Cyan
  if (!(Test-Path $localDbDir)) {
    New-Item -ItemType Directory -Path $localDbDir -Force | Out-Null
  }
  # Faire un backup de sécurité de la base locale si elle existe
  if (Test-Path $localDbPath) {
    $localTime = (Get-Item $localDbPath).LastWriteTime
    $onedriveTime = (Get-Item $onedriveDbPath).LastWriteTime
    if ($onedriveTime -gt $localTime) {
      Write-Host "Le fichier sur OneDrive est plus récent ($onedriveTime) que le fichier local ($localTime). Copie..." -ForegroundColor Green
      Copy-Item -Path $localDbPath -Destination "$localDbPath.bak" -Force
      Copy-Item -Path $onedriveDbPath -Destination $localDbPath -Force
    } else {
      Write-Host "La base de données locale est déjà à jour ou plus récente." -ForegroundColor Yellow
    }
  } else {
    Write-Host "Aucune base locale. Installation de la base depuis OneDrive..." -ForegroundColor Green
    Copy-Item -Path $onedriveDbPath -Destination $localDbPath -Force
  }
} else {
  Write-Host "Aucune base de données sur OneDrive pour le moment (elle sera créée automatiquement lors de la première sauvegarde)." -ForegroundColor Yellow
}

Write-Host "OK: dépôt et base de données à jour." -ForegroundColor Green
