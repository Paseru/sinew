# Sinew custom - mode simple

Ton fork public :

```text
https://github.com/pironjulien/sinew
```

Branche avec tes options :

```text
main
```

## Ce qui est en place

- Option `English / Français` dans `Settings > About`.
- Règle globale intégrée : te répondre simple, concis, power-user, options SOTA sans jargon.
- Dernière question posée fixée (sticky) en haut du chat pendant le défilement (défilement fluide vers la question au clic).
- Correction Windows pour mieux trouver `git.exe`.
- Correction Windows pour désactiver les warnings de compilation liés aux menus macOS/Linux (imports inutilisés).
- Scripts pour que tu gères le moins possible Git.

## Scripts utiles

Depuis ce dossier :

```powershell
.\scripts\sinew-sync.ps1
```

Met à jour depuis Sinew officiel.

```powershell
.\scripts\sinew-save.ps1
```

Commit + push tes changements sur GitHub.

```powershell
.\scripts\sinew-build-save.ps1 -Message "Description courte"
```

Build frontend + commit + push.

```powershell
.\scripts\sinew-build-save.ps1 -FullApp -Message "Description courte"
```

Build complet installable + commit + push. Nécessite Rust/Cargo.

## Règle simple

Quand tu ouvres ce repo pour travailler dessus :

```powershell
.\scripts\sinew-sync.ps1
```

Après une modification validée :

```powershell
.\scripts\sinew-build-save.ps1 -Message "Description courte"
```

Si tu veux juste sauvegarder sans build :

```powershell
.\scripts\sinew-save.ps1 -Message "Description courte"
```
