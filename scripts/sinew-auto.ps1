param(
  [string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
  [string]$Branch = "main",
  [string]$Message = "Update Julien custom Sinew options"
)

$ErrorActionPreference = "Stop"

& (Join-Path $PSScriptRoot "sinew-sync.ps1") -Repo $Repo -Branch $Branch
& (Join-Path $PSScriptRoot "sinew-save.ps1") -Repo $Repo -Branch $Branch -Message $Message
