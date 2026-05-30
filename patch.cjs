const fs = require('fs');
let code = fs.readFileSync('src/components/SettingsPane.tsx', 'utf8');

// 1. Extract blocks
const syncRe = /(\s*\{\/\*\s*Synchronisation Multi-PC.*?)(?=\{\/\*\s*Recherche Sémantique)/s;
const semanticRe = /(\s*\{\/\*\s*Recherche Sémantique.*?)(?=\{\/\*\s*Grille des 8 sous-cartes)/s;
const aiLearningRe = /(\s*\{\/\*\s*Apprentissage Automatique IA.*?)(?=\{\/\*\s*Pacte de Libération)/s;

const syncMatch = code.match(syncRe);
const semanticMatch = code.match(semanticRe);
const aiMatch = code.match(aiLearningRe);

if (!syncMatch || !semanticMatch || !aiMatch) {
    console.log("Could not find all source blocks!");
    process.exit(1);
}

const syncBlock = syncMatch[1];
const semanticBlock = semanticMatch[1];
const aiBlock = aiMatch[1];

// 2. Remove them from their original locations
code = code.replace(syncBlock, '');
code = code.replace(semanticBlock, '');
code = code.replace(aiBlock, '');

// 3. Inject them into the diagnostic tab's grid
// The diagnostic tab starts with:
// <div className="options-category-grid">
//           
//   {/* Diagnostic Système SOTA */}
const insertionPoint = /\s*\{\/\*\s*Diagnostic Système SOTA\s*\*\/\}/;

if (!insertionPoint.test(code)) {
    console.log("Could not find SOTA marker!");
    process.exit(1);
}

const injectedContent = syncBlock + semanticBlock + aiBlock + '\n          {/* Diagnostic Système SOTA */}';

code = code.replace(insertionPoint, injectedContent);

fs.writeFileSync('src/components/SettingsPane.tsx', code);
console.log("Done!");
