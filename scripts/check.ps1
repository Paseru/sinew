$ErrorActionPreference = "Stop"

function Invoke-CheckStep {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )

    Write-Host ""
    Write-Host "=== $Name ===" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name a échoué avec le code $LASTEXITCODE"
    }
}

function Test-CommandAvailable {
    param([Parameter(Mandatory = $true)][string]$Command)
    $null -ne (Get-Command $Command -ErrorAction SilentlyContinue)
}

if (-not (Test-CommandAvailable "npm")) { throw "npm est introuvable." }
if (-not (Test-CommandAvailable "cargo")) { throw "cargo est introuvable." }

Invoke-CheckStep "Build frontend" { npm run build }
Invoke-CheckStep "Rust check" { cargo check --workspace --all-targets }

Write-Host ""
Write-Host "=== Rust clippy ===" -ForegroundColor Cyan
cargo clippy --version | Out-Host
if ($LASTEXITCODE -ne 0) {
    throw "cargo clippy est introuvable. Lancez : rustup component add clippy"
}

if ($env:SINEW_STRICT_CLIPPY -eq "1") {
    cargo clippy --workspace --all-targets -- -D warnings
} else {
    cargo clippy --workspace --all-targets
}
if ($LASTEXITCODE -ne 0) {
    throw "Rust clippy a échoué avec le code $LASTEXITCODE"
}

Write-Host ""
Write-Host "Les tests live externes marqués ignorés restent lancés par leurs scripts dédiés." -ForegroundColor DarkGray
Invoke-CheckStep "Tests Rust locaux" { cargo test --workspace --no-fail-fast }
Invoke-CheckStep "Audit npm racine" { npm audit --omit=dev }
Invoke-CheckStep "Audit npm Chrome bridge" { npm --prefix sinew-chrome-bridge audit --omit=dev }
Invoke-CheckStep "Audit npm Agent bridge" { npm --prefix scripts/agent-bridge audit --omit=dev }

if (Test-CommandAvailable "cargo-audit") {
    Invoke-CheckStep "Audit Rust" { cargo audit }
} else {
    Write-Host ""
    Write-Host "=== Audit Rust ===" -ForegroundColor Yellow
    Write-Host "cargo-audit est absent : étape ignorée. Pour l'activer : cargo install cargo-audit --locked"
}

Write-Host ""
Write-Host "Tous les contrôles disponibles sont passés." -ForegroundColor Green
