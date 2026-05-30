import re

with open('src/components/SettingsPane.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Find the blocks
sync_re = re.compile(r'(\s*\{\/\*\s*Synchronisation Multi-PC.*?)(?=\{\/\*\s*Recherche Sémantique)', re.DOTALL)
semantic_re = re.compile(r'(\s*\{\/\*\s*Recherche Sémantique.*?)(?=\{\/\*\s*Grille des 8 sous-cartes)', re.DOTALL)
ai_learning_re = re.compile(r'(\s*\{\/\*\s*Apprentissage Automatique IA.*?)(?=\{\/\*\s*Pacte de Libération)', re.DOTALL)

sync_match = sync_re.search(content)
semantic_match = semantic_re.search(content)
ai_match = ai_learning_re.search(content)

if not sync_match or not semantic_match or not ai_match:
    print("Could not find one of the source blocks!")
    exit(1)

sync_block = sync_match.group(1)
semantic_block = semantic_match.group(1)
ai_block = ai_match.group(1)

# Remove them from current position
content = content.replace(sync_block, '')
content = content.replace(semantic_block, '')
content = content.replace(ai_block, '')

# Prepare the new unified diagnostic layout
# We will inject them right before "Diagnostic Système SOTA" inside the diagnostic tab.
sota_marker = r"{/* Diagnostic Système SOTA */}"
if sota_marker not in content:
    print("Could not find SOTA marker!")
    exit(1)

# To harmonize appearance, they are all .settings-pane__about-card.
# The diagnostic tab has <div className="options-category-grid"> which applies CSS grid (usually 2 columns or auto-fit).
# Sync, Semantic, and AI Learning are full-width cards. If we put them in the grid, they might be squished or they might span full width if we set gridColumn: "1 / -1".
# But SOTA and Pacte are currently inside the grid? No, wait!
# Let's see if SOTA is inside the grid.
