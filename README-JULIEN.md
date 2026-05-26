# Sinew — fork de Julien

Fork public : https://github.com/pironjulien/sinew

Ce fork garde Sinew proche de l’original, avec quelques ajouts pratiques pour mon usage.

## Ajouts

- Option `English / Français` dans `Settings > About`.
- Règle globale : réponses courtes, simples, adaptées à un power user.
- Multi-comptes OpenAI automatisé & raccourcis dynamiques 5.5 XHigh Fast.
- Extension Sinew Chrome Bridge & installeur dynamique.
- Enregistrement automatique des serveurs MCP dans la base de données.
- Sauvegarde permanente des logs dans `desktop-app.log`.
- Scripts locaux pour sync/build/push sans gérer Git à la main.

## Installation

1. Construire l’app :

```powershell
cd sinew-fr-work\source
npm run tauri build
```

2. Installer le fichier généré dans :

```text
src-tauri\target\release\bundle\
```

Sur Windows, choisir de préférence le `.exe` NSIS ou le `.msi`.

## Mises à jour

L’auto-update officiel de Sinew pointe vers les releases du dépôt original.

Pour éviter qu’une mise à jour officielle écrase mes ajouts, ce fork désactive l’auto-update officiel.

Méthode recommandée :

```powershell
.\scripts\sinew-build-save.ps1 -FullApp -Message "Description courte"
```

Ça synchronise, build, commit et push.
