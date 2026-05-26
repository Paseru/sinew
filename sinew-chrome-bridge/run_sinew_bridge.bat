@echo off
netstat -ano | findstr LISTENING | findstr :29002 >nul
if %errorlevel% neq 0 (
    start /B "" "C:\Program Files\nodejs\node.exe" "%~dp0server.js" >"%~dp0bridge.log" 2>&1
)
"C:\Program Files\nodejs\node.exe" "%~dp0mcp_server.js"
