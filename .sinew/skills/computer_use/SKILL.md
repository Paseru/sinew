---
name: computer_use
description: "Pilotage complet du bureau Windows (Souris, Clavier, Captures d'écran)."
---

# Pilotage Windows (Computer Use)

Utilisez cette compétence pour contrôler directement le système d'exploitation Windows lorsque l'utilisateur vous demande d'interagir avec des applications installées sur sa machine (Notepad, Visual Studio, etc.).

## Directives principales

1. **Prendre un screenshot en premier :** Avant d'interagir, effectuez une capture d'écran (`screenshot`) pour connaître la disposition des fenêtres et la résolution de l'écran.
2. **Utiliser des coordonnées réelles :** La souris se déplace à l'aide de coordonnées en pixels. La résolution totale est retournée lors des captures d'écran et des demandes de position du curseur.
3. **Simuler des actions de manière réaliste :**
   - Utilisez `mouse_move` suivi de `left_click` pour cliquer sur un bouton.
   - Utilisez `type` pour taper des textes.
   - Utilisez `key` pour presser des raccourcis clavier comme `ctrl+c`, `ctrl+v` ou des touches spéciales comme `enter` ou `escape`.
4. **Vérifier après chaque action :** Reprenez une capture d'écran après avoir cliqué ou écrit pour vous assurer que l'état de l'écran correspond bien à vos attentes.
