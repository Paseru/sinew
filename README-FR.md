# Traduction française de Sinew

Ce dossier contient une traduction française réapplicable de l'interface Sinew.

## Ce qui a été fait

- Le dépôt source amont a été cloné dans `source/` depuis `https://github.com/Paseru/sinew`.
- Une préférence locale de langue a été ajoutée dans `source/src/lib/locale.ts`.
- Une option `Language` / `Langue` a été ajoutée dans `Settings > About` pour basculer entre `English` et `Français`.
- Une couche de traduction française a été ajoutée dans `source/src/lib/frRuntime.ts`.
- `source/src/main.tsx` importe cette couche au démarrage.
- Le style du sélecteur est dans `source/src/styles.css`.
- Le patch réapplicable est sauvegardé dans `sinew-fr.patch`.

La traduction est volontairement prudente : elle vise les libellés d'interface, boutons, titres, placeholders et dialogues, mais évite le contenu des conversations, les sorties d'outils, le terminal, Monaco, le Markdown et les fichiers ouverts. Elle est désactivée par défaut tant que la langue `Français` n'est pas choisie dans Settings.

## Réappliquer après une mise à jour de Sinew

Depuis PowerShell, dans ce dossier :

```powershell
.\apply-sinew-fr.ps1 -UpdateSource
```

Options utiles :

```powershell
# Appliquer le patch puis vérifier le build frontend
.\apply-sinew-fr.ps1 -UpdateSource -BuildFrontend

# Appliquer le patch et construire l'application Tauri complète
# Nécessite Rust/Cargo + prérequis Tauri Windows.
.\apply-sinew-fr.ps1 -UpdateSource -BuildTauri
```

Le script :

1. met à jour ou clone le dépôt source ;
2. applique `sinew-fr.patch` ;
3. détecte si le patch est déjà appliqué ;
4. peut lancer le build frontend ou Tauri.

## Construire une version installable

Le build complet nécessite Rust/Cargo, non présent sur cette machine au moment de la préparation.

Quand Rust est installé :

```powershell
cd .\source
npm ci
npm run tauri build
```

Les installateurs Tauri sont généralement générés dans :

```text
source\src-tauri\target\release\bundle\
```

## Changer la langue

Dans Sinew :

```text
Settings > About > Language
```

Choisir :

- `English` pour garder l'interface originale ;
- `Français` pour activer la traduction française.

Le choix est stocké localement dans `localStorage` sous la clé :

```text
sinew.locale
```

Sinew recharge l'interface après le changement pour appliquer proprement la langue à tous les panneaux.

## Contrôle manuel depuis la console dev

```js
localStorage.setItem('sinew.locale', 'fr') // activer Français
localStorage.setItem('sinew.locale', 'en') // revenir à English
```

puis redémarrer/recharger Sinew.

## Notes sur les mises à jour

Sinew possède un auto-updater Tauri pointant vers les releases GitHub. Une version officielle installée peut donc écraser une build personnalisée. C'est pour cela que la traduction est conservée sous forme de patch réapplicable plutôt qu'en modification directe de `Sinew.exe`.

Si une future version modifie fortement `src/main.tsx`, `src/components/SettingsPane.tsx`, `src/styles.css` ou l'interface, `git apply --3way` tentera une fusion. En cas de conflit, les fichiers centraux à maintenir sont principalement :

```text
source/src/lib/locale.ts
source/src/lib/frRuntime.ts
source/src/components/SettingsPane.tsx
```
