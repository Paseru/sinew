@echo off
start /B "" "C:\Program Files\nodejs\node.exe" "%~dp0server.js" 1>&2
"C:\Users\julie\.gemini\antigravity\scratch\browser-use-env\Scripts\python.exe" -W ignore -m mcp_server_browser_use.server