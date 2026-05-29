# À faire — rapport d'analyse complet du projet Sinew

> Réanalyse le 2026-05-30 00:39. Réorienté pour éviter les conflits avec l'upstream actif.

---

## Résumé exécutif

**Règle n°1 : on ne refactorise pas le code hérité de l'upstream.** Paseru/sinew publie une release tous les 2-3 jours. On a déjà +8750 lignes de divergence sur les 5 plus gros fichiers. Si on les découpe, chaque merge upstream devient un cauchemar.

**Stratégie :** on améliore ce qui nous appartient (nos modules, nos correctifs, nos automatisations) et on garde le code upstream tel quel en le mettant à jour régulièrement.

---

## Différence upstream/nous sur les fichiers critiques

| Fichier | Nos ajouts | Risque si on découpe |
|---|---|---|
| `src/components/SettingsPane.tsx` | +3791 lignes | Très élevé |
| `src-tauri/src/providers.rs` | +1158 lignes | Très élevé |
| `sinew-chrome-bridge/server.js` | +3037 lignes (tout le fichier) | Aucun (rien chez upstream) |
| `src/components/chat/ChatPane.tsx` | +883 lignes | Élevé |
| `src/components/Workspace.tsx` | +147 lignes | Modéré |

---

## Plan d'action (nouvel ordre)

### 🔴 Priorité 1 — Nos modules à nous (zéro risque de conflit)

Ces modules n'existent pas chez l'upstream ou sont des ajouts purs. On peut les améliorer librement.

1. **Nettoyer `sinew-chrome-bridge/server.js`** (3037 lignes, 100% nous)
   - Séparer : serveur WebSocket, sécurité/authentification, actions Chrome, macros, interface
   - Gains : code plus sûr, plus facile à déboguer
   
2. **Ajouter des tests au bridge Chrome**
   - Tests end-to-end (`e2e-local.mjs`, `e2e-structured.mjs`) déjà présents
   - Ajouter des tests unitaires pour les fonctions internes

3. **Nettoyer les logs du bridge**
   - `bridge.log` et `bridge_err.log` traînent dans le dépôt → les gitignore ou les nettoyer

4. **Documenter le bridge Chrome**
   - Son fonctionnement, ses endpoints, ses dépendances

---

### 🟡 Priorité 2 — Nos correctifs propres (zéro risque de conflit)

Ces fichiers sont nos créations. L'upstream ne les a pas.

5. **Nettoyer le CHANGELOG**
   - Archiver les entrées marketing legacy (section "Présentation des Fonctionnalités Majeures")
   - Réorganiser en sections claires : correctifs, fonctionnalités, maintenance

6. **Corriger les 43 warnings clippy** (1-2 heures)
   - Commencer par nos propres crates (`sinew-cursor` = 8 warnings, `sinew-app` = 7)
   - Laisser les warnings upstream (`src-tauri` = 14) pour éviter les conflits

7. **Créer `DESIGN.md`**
   - Documenter les couleurs, tailles, boutons, règles visuelles
   - Le README l'annonce, il n'existe pas

8. **Résoudre la vulnérabilité esbuild** (mise à jour Vite)

---

### 🟢 Priorité 3 — Améliorations ciblées sur le code upstream (risque modéré)

Ces modifications touchent le code upstream mais sont assez isolées pour ne pas créer de conflits majeurs.

9. **Restreindre la portée asset Tauri** (`**/*` → dossier workspace + config)
   - Modification ciblée dans `tauri.conf.json`
   - Faible risque de conflit

10. **Dédupliquer LoginAttempt/LoginOutcome** (5 copies identiques → 1 générique)
    - Dans `state.rs`, 5 structures copiées-collées
    - Modification isolée, facile à rejouer après merge

---

### ⚪ Pas maintenant — Ce qu'on ne touche pas

- ❌ **Découper SettingsPane/ChatPane/Workspace/Providers** — trop de conflits à chaque merge upstream
- ❌ **Ajouter des tests frontend** — nécessiterait de toucher à l'architecture des composants upstream
- ❌ **Alléger lib.rs** — c'est le point de connexion Tauri, chaque refacto ici casse tout

---

## Routine de maintenance

Toutes les 1-2 semaines :
```bash
git fetch upstream
git merge upstream/main
npm run check    # valider que rien n'a cassé
```

Si conflit : on priorise nos ajouts premium mais on garde la structure upstream intacte.

---

## État confirmé (vérifié le 2026-05-30)

- ✅ 250+ tests passent, 0 échouent
- ✅ `npm run build` et `cargo check` passent
- ✅ `npm run check` complet passe
- ✅ Audits npm propres (sauf 1 esbuild dev)
- ✅ `cargo clippy` branché en mode rapport, strict dispo
- ✅ Git à jour avec origin/main
- ⚠️ 43 warnings clippy (dont 14 dans src-tauri = upstream)
- ⚠️ Portée asset Tauri = `**/*`
- ⚠️ CHANGELOG contient du bloat marketing legacy
- ⚠️ Pas de DESIGN.md
- ⚠️ bridge.log et bridge_err.log dans le dépôt
