$ErrorActionPreference = "Stop"
$authPath = Join-Path $env:LOCALAPPDATA "Hyrak\sinew\data\cursor-composer-auth.json"
if (-not (Test-Path $authPath)) { throw "Missing auth: $authPath" }
$auth = Get-Content $authPath -Raw | ConvertFrom-Json
$token = $auth.tokens.accessToken
if (-not $token) { $token = $auth.accessToken }
if (-not $token) { throw "No accessToken" }

$payload = @{
  accessToken = $token
  modelId = "composer-2-fast"
  systemPrompt = "You are Composer. Reply in one short sentence."
  userText = "Dis bonjour en francais, une phrase."
  workspaceRoot = "C:\Dev\Sinew"
} | ConvertTo-Json -Compress

Set-Location $PSScriptRoot
$job = Start-Job -ScriptBlock {
  param($p, $dir)
  Set-Location $dir
  $p | npx --yes tsx run-stream.mjs 2>&1
} -ArgumentList $payload, $PSScriptRoot
if (-not (Wait-Job $job -Timeout 90)) {
  Stop-Job $job -Force
  Remove-Job $job -Force
  throw "agent-bridge live test timed out after 90s"
}
Receive-Job $job
Remove-Job $job
