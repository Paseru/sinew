import sqlite3
import os

local_db = os.path.expandvars(r'%LOCALAPPDATA%\hyrak\sinew\data\desktop-state.sqlite3')
onedrive_db = os.path.expandvars(r'%USERPROFILE%\OneDrive\Documents\Sinew\desktop-state.sqlite3')

def get_db_data(path):
    if not os.path.exists(path):
        return None
    try:
        conn = sqlite3.connect(path)
        c = conn.cursor()
        
        # Get conversations and their message count
        c.execute('''
            SELECT c.id, c.title, c.updated_at_ms, COUNT(m.conversation_id) as msg_count
            FROM conversations c
            LEFT JOIN messages m ON c.id = m.conversation_id
            GROUP BY c.id
        ''')
        convs = {r[0]: {"title": r[1], "updated_at_ms": r[2], "msg_count": r[3]} for r in c.fetchall()}
        
        c.execute('SELECT COUNT(*) FROM tombstones')
        tombstones = c.fetchone()[0]
        
        conn.close()
        return {"convs": convs, "tombstones": tombstones, "size": os.path.getsize(path)}
    except Exception as e:
        print(f"Error loading {path}: {e}")
        return None

local_data = get_db_data(local_db)
onedrive_data = get_db_data(onedrive_db)

if local_data and onedrive_data:
    print(f"Local DB size: {local_data['size']} bytes, Tombstones: {local_data['tombstones']}")
    print(f"OneDrive DB size: {onedrive_data['size']} bytes, Tombstones: {onedrive_data['tombstones']}")
    
    print("\n--- COMPARING CONVERSATIONS ---")
    all_keys = set(local_data['convs'].keys()) | set(onedrive_data['convs'].keys())
    
    mismatches = 0
    for k in sorted(all_keys):
        local_c = local_data['convs'].get(k)
        onedrive_c = onedrive_data['convs'].get(k)
        
        if not local_c:
            print(f"Only in OneDrive: {k} - '{onedrive_c['title']}' (msgs: {onedrive_c['msg_count']})")
            mismatches += 1
        elif not onedrive_c:
            print(f"Only in Local: {k} - '{local_c['title']}' (msgs: {local_c['msg_count']})")
            mismatches += 1
        else:
            # Both exist, compare
            msg_diff = local_c['msg_count'] - onedrive_c['msg_count']
            time_diff = local_c['updated_at_ms'] - onedrive_c['updated_at_ms']
            
            if msg_diff != 0 or time_diff != 0:
                print(f"Mismatch in {k} ('{local_c['title']}'):")
                print(f"  Local   : msgs={local_c['msg_count']}, updated={local_c['updated_at_ms']}")
                print(f"  OneDrive: msgs={onedrive_c['msg_count']}, updated={onedrive_c['updated_at_ms']}")
                print(f"  Diff    : msg_diff={msg_diff}, time_diff={time_diff}")
                mismatches += 1
                
    if mismatches == 0:
        print("All conversations are identical!")
    else:
        print(f"\nTotal mismatched conversations: {mismatches}")
else:
    print("Could not load both databases")



