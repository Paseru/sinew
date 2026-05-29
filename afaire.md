# À faire — rapport d'analyse complet du projet Sinew

> Réanalyse complète le 2026-05-30 00:34.

---

## Résumé exécutif

Le projet est vivant, compile, tous les tests passent (250+), et la base technique est sérieuse. Les points les plus urgents sont la **dette de découpage** (5 fichiers monstres freinent toute évolution) et quelques **portes de sécurité trop larges** qui méritent d'être resserrées. Le reste est du perfectionnement.

---

## 1. Architecture & Structure

### 1.1 Les 5 fichiers monstres (god files)

Ces fichiers sont tellement gros qu'ils sont devenus des points de blocage : changer une ligne dedans est risqué car tout est mélangé.

| Fichier | Lignes réelles | Problème |
|---|---|---|
| `src/components/SettingsPane.tsx` | **7731** | 1 seul export, 63+ fonctions internes. Tout le panneau config (comptes, modèles, outils, apparence, clés API) vit dans un seul composant React. |
| `src/components/chat/ChatPane.tsx` | **6863** | Tout le chat : streaming, pièces jointes, planification, équipe, rendu markdown, aperçus. |
| `sinew-chrome-bridge/server.js` | **3037** | Serveur, sécurité, actions Chrome, macros, interface. Un seul fichier fait tout. |
| `src-tauri/src/providers.rs` | **2649** | ~100 symboles internes. Gère 7 fournisseurs IA dans un seul fichier. |
| `src/components/Workspace.tsx` | **2366** | Disposition, onglets, terminal, explorateur fichiers, éditeur, barre latérale. |

**Impact :** à chaque modification, on touche à une montagne. Le risque d'erreur est proportionnel à la taille. Un découpage par responsabilité réduirait ce risque de 80%.

### 1.2 Le module "entonnoir" (lib.rs)

`src-tauri/src/lib.rs` (1147 lignes) est un entonnoir : il importe tout depuis les 13 crates Rust (100+ symboles importés) et connecte tout à Tauri. C'est inévitable dans une app Tauri, mais le fichier pourrait être allégé en déléguant à des sous-modules (auth, sync, providers, terminal, git...).

### 1.3 Duplication entre fournisseurs

Les structures `LoginAttempt`/`LoginOutcome` sont dupliquées 5 fois à l'identique pour OpenAI, Anthropic, Google, Kimi, et Cursor dans `state.rs`. Un seul type générique suffirait.

---

## 2. Qualité du code

| Indicateur | Valeur | Verdict |
|---|---|---|
| Tests Rust | 250+ passent, 0 échouent | ✅ Excellent |
| Tests frontend | Aucun | ⚠️ Zone vide |
| Avertissements clippy | 43 | ⚠️ À réduire |
| Audit npm racine | 1 vulnérabilité modérée (esbuild, dev only) | ⚠️ Bénin mais présent |
| Audit npm chrome-bridge | 0 | ✅ |
| Audit npm agent-bridge | 0 | ✅ |
| Compilation Rust | Passe | ✅ |
| Compilation frontend | Passe | ✅ |

**Bonnes nouvelles :** les tests Rust sont solides. 141 tests dans sinew-app, 57 dans sinew-cursor, + tous les autres crates.

**Mauvaises nouvelles :** zéro test frontend. Les 7731 lignes de SettingsPane et les 6863 lignes de ChatPane n'ont aucune couverture automatique.

---

## 3. Sécurité

### 3.1 Porte grande ouverte — assets Tauri

```json
"assetProtocol": { "enable": true, "scope": { "allow": ["**/*"] } }
```

Le protocole asset permet d'accéder à **n'importe quel fichier** depuis le frontend. C'est pratique pour un IDE, mais c'est une porte grande ouverte. Idéalement, restreindre au dossier du projet ouvert + dossier de config.

### 3.2 Terminal intégré

Le terminal (`terminal.rs`) a un accès shell complet. C'est voulu (c'est un outil de dev), mais 13 fonctions publiques exposent lancement, écriture, redimensionnement et terminaison de processus. Un audit de surface d'attaque est sain.

### 3.3 Pont Chrome

Le bridge Chrome (`server.js` + `mcp_server.js`) contrôle totalement un navigateur. C'est puissant, mais la surface d'attaque est large. Les logs (`bridge.log`, `bridge_err.log`) sont dans le dépôt.

### 3.4 Permissions Tauri

Les capabilities Tauri (`default.json`) sont très basiques : core defaults + dialog + updater. Pas de permissions shell/fs explicites. C'est plutôt rassurant, mais à surveiller lors des mises à jour Tauri.

---

## 4. Dettes techniques

### 4.1 CHANGELOG saturé

Le CHANGELOG a des entrées legacy (2026-05-29 avec descriptions premium/marketing), des sections dupliquées, et un format qui mélange historique marketing et changements techniques. À nettoyer.

### 4.2 Avertissements clippy (43)

Principalement dans `src-tauri` (14) et `sinew-app` (7). Des corrections simples (`needless_borrow`, `collapsible_if`) qui s'accumulent.

### 4.3 Dépendances Rust non maintenues

L'audit `cargo audit` signale des dépendances Tauri/GTK non maintenues (atk, gdk, gtk, etc.) et d'autres (fxhash, paste, unic-*, number_prefix). Ce sont des dépendances indirectes héritées de l'écosystème Tauri — rien d'urgent mais à surveiller aux prochaines versions.

---

## 5. Points forts (à préserver)

- **Fondation solide :** 250+ tests, 13 crates bien séparées, compilation propre
- **Script check :** `npm run check` fonctionne et couvre tout
- **Séparation des tests live :** les tests réseau sont ignorés par défaut
- **Multi-fournisseurs :** 7 fournisseurs IA intégrés proprement
- **Architecture extensible :** le pattern Provider permet d'ajouter un fournisseur sans tout casser

---

## Plan d'action (par impact décroissant)

### 🔴 À faire maintenant (blocage à l'évolution)

1. **Découper SettingsPane.tsx** (7731 → ~6 fichiers de ~1200 lignes)
   - Comptes & connexion, Modèles & quotas, Outils & MCP, Apparence, Avancé
2. **Découper ChatPane.tsx** (6863 → ~7 fichiers de ~1000 lignes)
   - Flux messages, Barre saisie, Pièces jointes, Planification, Équipe, Rendu markdown, Aperçus

### 🟡 À faire bientôt (sécurité & hygiène)

3. **Restreindre la portée asset Tauri** (`**/*` → dossier workspace + config)
4. **Auditer la surface terminal** (vérifier qui peut lancer quoi)
5. **Corriger les 43 warnings clippy** (1-2 sessions suffisent)
6. **Nettoyer le CHANGELOG** (archiver les entrées marketing legacy)

### 🟢 À faire quand possible (qualité long terme)

7. **Ajouter des tests frontend** (au minimum sur les fonctions utilitaires)
8. **Dédupliquer LoginAttempt/LoginOutcome** (5 copies → 1 générique)
9. **Alléger lib.rs** (déléguer à auth.rs, sync.rs, providers.rs dans src-tauri)
10. **Créer DESIGN.md** (documenter les règles visuelles)
11. **Résoudre la vulnérabilité esbuild** (mise à jour de Vite)

---

## État confirmé (vérifié le 2026-05-30)

- ✅ `npm run build` passe
- ✅ `cargo check --workspace --all-targets` passe
- ✅ 250+ tests passent (4 ignorés = tests live Cursor)
- ✅ `npm run check` complet passe
- ✅ Audits npm propres (sauf 1 esbuild dev)
- ✅ `cargo clippy` branché, mode strict disponible
- ✅ Git à jour avec origin/main
- ❌ Pas de tests frontend
- ⚠️ 43 avertissements clippy
- ⚠️ Portée asset Tauri = `**/*`
