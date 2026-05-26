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

Write-Host "OK: dépôt à jour." -ForegroundColor Green
