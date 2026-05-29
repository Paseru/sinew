import json
import os
import re
import sys
from datetime import datetime
from pathlib import Path

if sys.platform.startswith("win"):
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except AttributeError:
        pass

ALIASES = {
    "git_exclusions_build_node_modules": ["node_modules", "build/", ".exe", ".msi"],
    "spawn_einval_windows": ["spawn einval", "shell: true"],
    "recursive_postinstall_npm": ["postinstall", "npm install"],
    "mcp_autoload_serialization": ["autoload", "settingstojson"],
    "absolute_paths_windows": ["chemins de fichiers absolus", "chemins relatifs"],
}


def load_text(path: Path):
    for encoding in ("utf-8-sig", "utf-8", "cp1252", "latin-1"):
        try:
            return path.read_text(encoding=encoding), encoding
        except UnicodeDecodeError:
            continue
    return path.read_text(encoding="utf-8", errors="replace"), "utf-8"


def load_json_file(path: Path):
    text, encoding = load_text(path)
    return json.loads(text), encoding


def normalized(value: str) -> str:
    value = value.lower().replace("_", " ").replace("-", " ")
    return re.sub(r"\s+", " ", value).strip()


def rule_covers_error(rules_text: str, error_id: str) -> bool:
    rules = normalized(rules_text)
    error_key = normalized(error_id)
    if error_key and error_key in rules:
        return True
    aliases = ALIASES.get(error_id, [])
    if aliases and all(normalized(alias) in rules for alias in aliases[:2]):
        return True
    return False


def next_rule_number(rules_text: str) -> int:
    numbers = []
    for line in rules_text.splitlines():
        match = re.match(r"^###\s+(\d+)\.", line.strip())
        if match:
            numbers.append(int(match.group(1)))
    return max(numbers, default=0) + 1


def title_from_error_id(error_id: str) -> str:
    return " ".join(part.capitalize() for part in error_id.split("_") if part)


def build_rule(number: int, error: dict) -> str:
    error_id = str(error.get("id", "erreur_repetee"))
    description = str(error.get("description", "Erreur répétitive sans description.")).strip()
    description = re.sub(r"\s+", " ", description)
    return (
        f"\n\n### {number}. 🧠 Règle auto-consolidée — {title_from_error_id(error_id)}\n"
        f"* **Règle** : Cette erreur répétée a été détectée automatiquement : {description}. "
        "À chaque occurrence similaire, l'agent doit s'arrêter, identifier la cause générale, "
        "appliquer ou créer une règle globale adaptée, puis éviter de répéter la même tentative ciblée."
    )


def main():
    local_app_data = os.getenv("LOCALAPPDATA")
    if not local_app_data:
        print("LOCALAPPDATA introuvable : consolidation ignorée.")
        return

    sinew_dir = Path(local_app_data) / "Sinew"
    errors_path = sinew_dir / "errors_raw.json"
    rules_path = sinew_dir / "instructions_consolidated.md"

    if not errors_path.exists():
        print("Aucun fichier errors_raw.json trouvé : rien à consolider.")
        return

    try:
        errors, errors_encoding = load_json_file(errors_path)
    except Exception as exc:
        print(f"Lecture impossible de errors_raw.json : {exc}")
        return

    if not isinstance(errors, list):
        print("Format errors_raw.json inattendu : la consolidation attend une liste.")
        return

    if rules_path.exists():
        rules_text, _ = load_text(rules_path)
    else:
        rules_text = (
            "# 🛡️ Instructions Globales Consolidées (Règles anti-erreurs répétitives)\n\n"
            "Ces instructions ont été validées et consolidées après avoir été rencontrées au moins 3 fois. "
            "Tout agent intervenant sur ce projet doit les respecter à la lettre."
        )

    changed_errors = False
    changed_rules = False
    created_rules = 0
    cleaned_errors = 0
    number = next_rule_number(rules_text)
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    for error in errors:
        if not isinstance(error, dict):
            continue
        error_id = str(error.get("id", "")).strip()
        count = int(error.get("count") or 0)
        if count < 3 or not error_id:
            continue

        if rule_covers_error(rules_text, error_id):
            cleaned_errors += 1
        else:
            rules_text += build_rule(number, error)
            number += 1
            created_rules += 1
            changed_rules = True

        error["count"] = 0
        error["consolidated_at"] = now
        changed_errors = True

    if changed_rules:
        rules_path.parent.mkdir(parents=True, exist_ok=True)
        rules_path.write_text(rules_text.rstrip() + "\n", encoding="utf-8")

    if changed_errors:
        errors_path.write_text(json.dumps(errors, ensure_ascii=False, indent=4) + "\n", encoding=errors_encoding)

    print(
        f"Consolidation terminée : {created_rules} règle(s) créée(s), "
        f"{cleaned_errors} erreur(s) déjà couvertes nettoyée(s)."
    )


if __name__ == "__main__":
    main()
