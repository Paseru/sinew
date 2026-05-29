import os
import json
import sys

# Forcer la sortie standard en UTF-8 si nécessaire
if sys.platform.startswith('win'):
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except AttributeError:
        pass

def load_json_file(path):
    encodings = ["utf-8", "cp1252", "latin-1"]
    for encoding in encodings:
        try:
            with open(path, "r", encoding=encoding) as f:
                return json.load(f), encoding
        except (UnicodeDecodeError, json.JSONDecodeError):
            continue
    # Fallback
    with open(path, "r", encoding="utf-8", errors="ignore") as f:
        return json.load(f), "utf-8"

def main():
    local_app_data = os.getenv("LOCALAPPDATA")
    if not local_app_data:
        print("Erreur: LOCALAPPDATA non trouve.")
        return

    sinew_dir = os.path.join(local_app_data, "Sinew")
    errors_path = os.path.join(sinew_dir, "errors_raw.json")
    rules_path = os.path.join(sinew_dir, "instructions_consolidated.md")

    if not os.path.exists(errors_path):
        print("Aucun fichier d'erreurs brutes trouve.")
        return

    try:
        errors, encoding = load_json_file(errors_path)
    except Exception as e:
        print(f"Erreur lors de la lecture des erreurs : {e}")
        return

    # Charger les regles existantes
    rules_content = ""
    if os.path.exists(rules_path):
        try:
            for enc in ["utf-8", "cp1252", "latin-1"]:
                try:
                    with open(rules_path, "r", encoding=enc) as f:
                        rules_content = f.read()
                        break
                except UnicodeDecodeError:
                    continue
        except Exception as e:
            print(f"Erreur lors de la lecture des regles : {e}")

    cleaned_any = False

    for error in errors:
        error_id = error.get("id", "")
        count = error.get("count", 0)

        # Si l'erreur a ete integree dans le fichier MD de regles, on remet son compteur a 0
        keyword = error_id.replace("_", " ").lower()
        if keyword and keyword in rules_content.lower():
            if count > 0:
                print(f"Nettoyage : L'erreur '{error_id}' a ete identifiee comme resolue par une regle globale. Reinitialisation.")
                error["count"] = 0
                cleaned_any = True
        elif count >= 3:
            print(f"[ALERTE] L'erreur '{error_id}' est apparue {count} fois et necessite une regle globale.")

    if cleaned_any:
        try:
            with open(errors_path, "w", encoding=encoding) as f:
                json.dump(errors, f, ensure_ascii=False, indent=4)
            print("Fichier errors_raw.json mis a jour avec succes.")
        except Exception as e:
            print(f"Erreur lors de l'ecriture des erreurs : {e}")

if __name__ == "__main__":
    main()
