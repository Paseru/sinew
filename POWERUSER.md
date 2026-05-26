# Guide de Julien (Power User) — Sinew Custom

Ce fichier regroupe toutes les fonctionnalités, configurations et scripts de ton fork personnalisé de **Sinew**.

---

## 🚀 Le mode "Power User" : C'est quoi ?

Le bouton **Power User Mode** (situé dans `Settings > About`) est la clé. Lorsqu'il est activé, il configure l'environnement pour toi :
1. **Zéro Jargon** : L'assistant IA te répond de manière simple, concise et orientée vers l'action, sans jargon technique ou Git inutile.
2. **Automatisation Git** : L'assistant gère les commits, les pushs et les synchronisations en arrière-plan pour que tu n'aies pas à taper de commandes Git compliquées.

Si tu désactives ce bouton, l'assistant se comportera de manière classique (avec jargon technique normal et sans automatisation Git forcée).

---

## ✨ Fonctionnalités et optimisations de ton Fork

Ton fork contient plusieurs améliorations majeures par rapport à la version officielle de Sinew :

- **Traduction Française Dynamique** : Option de langue intégrée dans `Settings > About` pour basculer instantanément toute l'interface en français.
- **Sticky Question** : La dernière question posée reste fixée (sticky) en haut du chat pendant le défilement. Un simple clic dessus te ramène en haut de manière fluide.
- **Diagnostic SOTA en temps réel** : Un bouton et un panneau visuel dans `Settings > About` (ainsi que l'outil IA `check_sota`) pour tester instantanément si tes outils système de pointe (Git, Node, Cargo, Python, Ripgrep) sont opérationnels.
- **Corrections Windows** :
  - Meilleure détection automatique de `git.exe` sur ton système.
  - Suppression des avertissements (warnings) inutiles lors de la compilation sur Windows.
- **Auto-enregistrement MCP & Logs** : Sauvegarde propre des logs de l'application dans `desktop-app.log` et détection automatique des serveurs MCP.

---

## 🛠️ Scripts utiles pour automatiser ton quotidien

Pour te simplifier la vie, utilise ces scripts depuis la console (PowerShell) :

### 1. Démarrer ta session de travail (Synchroniser)
Avant de commencer à travailler sur ton projet, mets à jour ton code avec le dépôt officiel :
```powershell
.\scripts\sinew-sync.ps1
```

### 2. Sauvegarder tes modifications (sans compiler l'application)
Si tu veux juste enregistrer ton travail actuel sur GitHub :
```powershell
.\scripts\sinew-save.ps1 -Message "Description de tes changements"
```

### 3. Compiler et sauvegarder (Recommandé après modification)
Pour compiler l'interface et envoyer tes modifications sur GitHub d'un coup :
```powershell
.\scripts\sinew-build-save.ps1 -Message "Description de tes changements"
```

### 4. Build complet de l'application installable (.exe)
Si tu as installé Rust/Cargo et que tu veux générer l'installateur Windows final tout en le sauvegardant :
```powershell
.\scripts\sinew-build-save.ps1 -FullApp -Message "Description"
```
*Note : L'installateur généré se trouvera dans `src-tauri\target\release\bundle\`.*
