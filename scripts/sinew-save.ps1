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

Write-Host "OK: changements poussés sur GitHub." -ForegroundColor Green
