import sqlite3
import json
import os
import sys
import time
import shutil

# Paths
home_dir = os.path.expanduser("~")
db_path = os.path.join(home_dir, "AppData", "Local", "hyrak", "sinew", "data", "desktop-state.sqlite3")

if not os.path.exists(db_path):
    print(f"ERROR: Database not found at {db_path}")
    sys.exit(1)

# Configure the Sinew Chrome MCP server dynamically
script_dir = os.path.dirname(os.path.abspath(__file__))
node_path = os.environ.get("SINEW_NODE_PATH") or shutil.which("node") or r"C:\Program Files\nodejs\node.exe"
if not (os.path.isabs(node_path) and os.path.exists(node_path)) and not shutil.which(node_path):
    node_path = "node"

new_server = {
    "id": "sinew-chrome",
    "name": "Sinew Chrome",
    "command": node_path,
    "args": [os.path.join(script_dir, "mcp_server.js")],
    "env": [
        {"key": "MCP_BROWSER_CDP_URL", "value": "http://localhost:29002"}
    ],
    "cwd": script_dir,
    "enabled": True
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
            if s.get("id") in ("sinew-chrome", "browser-use"):
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
    print("SUCCESS: MCP server 'Sinew Chrome' registered successfully in Sinew's database!")
except Exception as e:
    print("ERROR: Error updating database:", e)
    sys.exit(1)
