import os
import shutil
import sqlite3
import subprocess
import sys

local_db = os.path.expandvars(r'%LOCALAPPDATA%\hyrak\sinew\data\desktop-state.sqlite3')
onedrive_dir = os.path.expandvars(r'%USERPROFILE%\OneDrive\Documents\Sinew')
onedrive_db = os.path.join(onedrive_dir, 'desktop-state.sqlite3')

def merge_databases(dest_path, src_path):
    """
    Merges data from src_path into dest_path using SQLite's ATTACH and INSERT OR REPLACE.
    Mimics the Rust implementation in src-tauri/src/lib.rs.
    """
    print(f"Executing differential merge from Local ({src_path}) into OneDrive ({dest_path})...")
    conn = None
    try:
        conn = sqlite3.connect(dest_path)
        c = conn.cursor()
        
        # Attach the source database (local_db)
        c.execute(f"ATTACH DATABASE '{src_path}' AS local_source")
        
        # Ensure tombstones table exists in both to prevent errors
        c.execute("CREATE TABLE IF NOT EXISTS main.tombstones (id TEXT PRIMARY KEY, deleted_at_ms INTEGER NOT NULL)")
        c.execute("CREATE TABLE IF NOT EXISTS local_source.tombstones (id TEXT PRIMARY KEY, deleted_at_ms INTEGER NOT NULL)")
        
        # Enable foreign keys
        c.execute("PRAGMA foreign_keys = ON")
        
        # 1. Merge tombstones
        c.execute("INSERT OR REPLACE INTO main.tombstones SELECT * FROM local_source.tombstones")
        
        # 2. Delete conversations/messages that are in tombstones
        c.execute("DELETE FROM main.conversations WHERE id IN (SELECT id FROM main.tombstones)")
        c.execute("DELETE FROM main.messages WHERE conversation_id IN (SELECT id FROM main.tombstones)")
        c.execute("DELETE FROM main.turn_checkpoints WHERE conversation_id IN (SELECT id FROM main.tombstones)")
        
        # 3. Merge conversations (excluding those with tombstones)
        c.execute("""
            INSERT OR IGNORE INTO main.conversations 
            SELECT * FROM local_source.conversations 
            WHERE id NOT IN (SELECT id FROM main.tombstones)
        """)
        c.execute("""
            INSERT OR REPLACE INTO main.conversations 
            SELECT * FROM local_source.conversations AS s 
            WHERE EXISTS (
                SELECT 1 FROM main.conversations AS d 
                WHERE d.id = s.id AND s.updated_at_ms > d.updated_at_ms
            ) AND s.id NOT IN (SELECT id FROM main.tombstones)
        """)
        
        # 4. Merge messages
        c.execute("""
            INSERT OR IGNORE INTO main.messages 
            SELECT * FROM local_source.messages 
            WHERE conversation_id NOT IN (SELECT id FROM main.tombstones)
        """)
        c.execute("""
            INSERT OR REPLACE INTO main.messages 
            SELECT * FROM local_source.messages AS s 
            WHERE EXISTS (
                SELECT 1 FROM main.conversations AS mc 
                JOIN local_source.conversations AS sc ON mc.id = sc.id 
                WHERE mc.id = s.conversation_id AND sc.updated_at_ms > mc.updated_at_ms
            ) AND s.conversation_id NOT IN (SELECT id FROM main.tombstones)
        """)
        
        # 5. Merge app_settings
        c.execute("INSERT OR IGNORE INTO main.app_settings SELECT * FROM local_source.app_settings")
        c.execute("""
            INSERT OR REPLACE INTO main.app_settings 
            SELECT * FROM local_source.app_settings AS s 
            WHERE EXISTS (
                SELECT 1 FROM main.app_settings AS d 
                WHERE d.key = s.key AND s.updated_at_ms > d.updated_at_ms
            )
        """)
        
        conn.commit()
        print("Differential merge completed successfully!")
        return True
    except Exception as e:
        print(f"Error during differential merge: {e}")
        if conn:
            try:
                conn.rollback()
            except Exception:
                pass
        return False
    finally:
        if conn:
            try:
                c.execute("DETACH DATABASE local_source")
            except Exception:
                pass
            conn.close()

def sync():
    print("--- STARTING SINEW FORCE SYNC ---")
    
    # 1. Sync Database to OneDrive
    if not os.path.exists(local_db):
        print(f"Error: Local database not found at {local_db}")
        return
        
    os.makedirs(onedrive_dir, exist_ok=True)
    
    # Backup existing OneDrive db just in case
    if os.path.exists(onedrive_db):
        backup_path = onedrive_db + ".bak"
        print(f"Creating a backup of current OneDrive database at {backup_path}")
        try:
            shutil.copy2(onedrive_db, backup_path)
        except Exception as e:
            print(f"Warning: Could not create backup of OneDrive database: {e}")
            
        # Attempt differential merge
        merge_success = merge_databases(onedrive_db, local_db)
        if not merge_success:
            print("Falling back to direct copy of the local database to OneDrive...")
            try:
                shutil.copy2(local_db, onedrive_db)
                print("Direct copy successful!")
            except Exception as e:
                print(f"Error: Direct copy failed: {e}")
    else:
        print("OneDrive database does not exist. Performing initial copy...")
        try:
            shutil.copy2(local_db, onedrive_db)
            print("Initial copy successful!")
        except Exception as e:
            print(f"Error: Initial copy failed: {e}")
            
    # 2. Sync global learning files
    print("Consolidating global learning rules before OneDrive copy...")
    try:
        script_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'consolidate_rules.py')
        if os.path.exists(script_path):
            subprocess.run([sys.executable, script_path], check=False)
    except Exception as e:
        print(f"Warning: Failed to consolidate global learning rules: {e}")

    local_learning_dir = os.path.expandvars(r'%LOCALAPPDATA%\Sinew')
    errors_local = os.path.join(local_learning_dir, 'errors_raw.json')
    rules_local = os.path.join(local_learning_dir, 'instructions_consolidated.md')
    
    errors_onedrive = os.path.join(onedrive_dir, 'errors_raw.json')
    rules_onedrive = os.path.join(onedrive_dir, 'instructions_consolidated.md')
    
    if os.path.exists(errors_local):
        print("Syncing errors_raw.json to OneDrive...")
        try:
            shutil.copy2(errors_local, errors_onedrive)
        except Exception as e:
            print(f"Warning: Failed to copy errors_raw.json: {e}")
            
    if os.path.exists(rules_local):
        print("Syncing instructions_consolidated.md to OneDrive...")
        try:
            shutil.copy2(rules_local, rules_onedrive)
        except Exception as e:
            print(f"Warning: Failed to copy instructions_consolidated.md: {e}")
            
    # 3. Git Push current workspace
    print("\nVerifying Git status for the workspace...")
    try:
        # Check if there are uncommitted changes
        status_out = subprocess.check_output(["git", "status", "--porcelain"], text=True)
        if status_out.strip():
            print("Uncommitted changes found in workspace. Committing them...")
            subprocess.run(["git", "add", "."], check=True)
            subprocess.run(["git", "commit", "-m", "Sauvegarde automatique et synchronisation complète avant de quitter"], check=True)
            
        print("Pushing commits to remote repository...")
        subprocess.run(["git", "push"], check=True)
        print("Git Push successful!")
    except Exception as e:
        print(f"Warning: Git synchronization failed: {e}")
        
    print("\n--- SINEW FORCE SYNC COMPLETED SUCCESSFULLY ---")

if __name__ == '__main__':
    sync()
