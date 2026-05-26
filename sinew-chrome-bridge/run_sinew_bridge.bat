@echo off
netstat -ano | findstr LISTENING | findstr :29002 >nul
if %errorlevel% neq 0 (
    start /B "" "C:\Program Files\nodejs\node.exe" "%~dp0server.js" >"%~dp0bridge.log" 2>&1
)
"C:\Users\julie\AppData\Local\Programs\Python\Python314\python.exe" -m mcp_server_browser_use.server