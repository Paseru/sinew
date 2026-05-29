# 🛡️ Instructions Globales Consolidées (Règles anti-erreurs répétitives)

Ces instructions ont été validées et consolidées après avoir été rencontrées au moins 3 fois. Tout agent intervenant sur ce projet doit les respecter à la lettre.

### 1. ⚡ Windows child_process (spawn EINVAL)
* **Règle** : Lors du lancement de commandes système (`npm`, `node`, etc.) sous Windows via `spawn` ou `execFile`, **toujours** spécifier `{ shell: true }` dans les options pour éviter l'erreur bloquante `spawn EINVAL`.

### 2. 🔄 Scripts npm récursifs (postinstall)
* **Règle** : Ne **jamais** appeler `npm install` (ou `npm --prefix ...`) directement dans le bloc `"postinstall"` du fichier `package.json` principal sous peine de déclencher une boucle infinie de téléchargement. Passer par les scripts de cycle de vie de compilation dédiés (comme `prepare-agent-bridge`).

### 3. 🎛️ Sérialisation MCP (autoLoad)
* **Règle** : Dans `src/components/SettingsPane.tsx`, lors de l'écriture ou de l'édition de la fonction `settingsToJson`, **toujours** inclure la ligne `if (server.autoLoad) entry.autoLoad = true;` sous peine de réinitialiser le choix de l'utilisateur à chaque rechargement.
