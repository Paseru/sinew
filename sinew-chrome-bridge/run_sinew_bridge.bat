@echo off
start /B "" "C:\Program Files\nodejs\node.exe" "%~dp0server.js"
"C:\Users\julie\.gemini\antigravity\scratch\browser-use-env\Scripts\python.exe" -m mcp_server_browser_use.cli server --foreground