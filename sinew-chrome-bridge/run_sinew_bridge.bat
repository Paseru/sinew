@echo off
setlocal
set "BRIDGE_DIR=%LOCALAPPDATA%\Sinew\ChromeBridge"
if exist "%BRIDGE_DIR%\mcp_server.js" (
  node "%BRIDGE_DIR%\mcp_server.js"
) else (
  node "%~dp0mcp_server.js"
)
