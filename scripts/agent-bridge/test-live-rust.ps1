# Live test: Rust agent bridge via sinew-cursor (requires OAuth token).
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent | Join-Path -ChildPath "..")
$env:SINEW_CURSOR_TRANSPORT = "agent"
$env:SINEW_CURSOR_BRIDGE = "rust"
$env:SINEW_CURSOR_LIVE_ASSERT = "1"
cargo test -p sinew-cursor test_live_rust_agent_bridge -- --ignored --nocapture
