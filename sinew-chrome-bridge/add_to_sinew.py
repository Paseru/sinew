import sqlite3
import json
import os
import sys
import time
import shutil
from pathlib import Path

# Paths
home_dir = os.path.expanduser("~")
db_path = os.environ.get(
    "SINEW_DESKTOP_DB",
    os.path.join(home_dir, "AppData", "Local", "hyrak", "sinew", "data", "desktop-state.sqlite3"),
)

if not os.path.exists(db_path):
    print(f"ERROR: Database not found at {db_path}")
    sys.exit(1)

# Configure the Sinew Chrome MCP server dynamically.
# Prefer the installed runtime copied by register.ps1 so Sinew never depends on a dev path.
source_dir = Path(__file__).resolve().parent
installed_dir = Path(
    os.environ.get(
        "SINEW_CHROME_BRIDGE_DIR",
        os.path.join(os.environ.get("LOCALAPPDATA", os.path.join(home_dir, "AppData", "Local")), "Sinew", "ChromeBridge"),
    )
)
script_dir = installed_dir if (installed_dir / "mcp_server.js").exists() else source_dir

node_path = os.environ.get("SINEW_NODE_PATH") or shutil.which("node") or r"C:\Program Files\nodejs\node.exe"
if not (os.path.isabs(node_path) and os.path.exists(node_path)) and not shutil.which(node_path):
    node_path = "node"

new_server = {
    "id": "sinew-chrome",
    "name": "Sinew Chrome",
    "command": node_path,
    "args": [str(script_dir / "mcp_server.js")],
    "env": [
        {"key": "MCP_BROWSER_CDP_URL", "value": "http://127.0.0.1:29002"},
        {"key": "SINEW_CHROME_BRIDGE_DIR", "value": str(script_dir)},
    ],
    "cwd": str(script_dir),
    "enabled": True,
    "autoLoad": True,
}

try:
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Read existing settings
    cursor.execute("SELECT value_json FROM app_settings WHERE key = 'mcp_settings';")
    row = cursor.fetchone()
    
    if row:
        settings = json.loads(row[0])
        # Find if Sinew Chrome is already registered, including the previous legacy id.
        servers = settings.get("servers", [])
        updated = False
        for i, s in enumerate(servers):
            if s.get("id") in ("sinew-chrome", "browser-use") or s.get("name") == "Sinew Chrome":
                servers[i] = new_server
                updated = True
                break
        if not updated:
            servers.append(new_server)
        settings["servers"] = servers
    else:
        settings = {
            "servers": [new_server]
        }
        
    value_json = json.dumps(settings)
    updated_at_ms = int(time.time() * 1000)
    
    cursor.execute(
        """
        INSERT INTO app_settings (key, value_json, updated_at_ms)
        VALUES ('mcp_settings', ?, ?)
        ON CONFLICT(key) DO UPDATE SET
            value_json = excluded.value_json,
            updated_at_ms = excluded.updated_at_ms;
        """,
        (value_json, updated_at_ms)
    )
    
    conn.commit()
    conn.close()
    print(f"SUCCESS: MCP server 'Sinew Chrome' registered at {script_dir}!")
except Exception as e:
    print("ERROR: Error updating database:", e)
    sys.exit(1)
