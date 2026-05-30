const fs = require('fs');
let code = fs.readFileSync('src/components/SettingsPane.tsx', 'utf8');

const replacements = {
  'ï¿½  vous': '— vous',
  'Â« return to Cursor Â» ï¿½  c\'est normal': '« return to Cursor » — c\'est normal',
  'ï¿½x}ï¿½': '🛠️',
  'ï¿½Ü¬ï¸ ': '☀️',
  'ï¿½x ï¿½': '💻',
  'SystÃ¨me': 'Système',
  'ï¿½Sï¿½': '✨',
  'ï¿½0diteur': 'Éditeur',
  'ï¿½aï¿½': '⚡',
  'ï¿½xï¿½ï¿½': '🧠',
  'ï¿½x   ': '⚙️',
  'LibÃ©ration': 'Libération',
  'ï¿½  AmÃ©liorations ClÃ©s': '— Améliorations Clés',
  'aprÃ¨s la frappe. ï¿½0cran': 'après la frappe. Écran',
  'sÃ©curisÃ©': 'sécurisé',
  'ï¿½ ': '⏳',
  'ParamÃ¨tres ï¿½   Providers': 'Paramètres > Providers',
  'No servers yet ï¿½  add': 'No servers yet — add',
  'ï¿½aï¿½ï¸ ': '⚠️',
  'Ã©galement': 'également',
  'premiÃ¨re': 'première',
  'No sub-agents yet ï¿½  click': 'No sub-agents yet — click',
  'ï¿½S  ModÃ¨les vÃ©rifiÃ©s': '✅ Modèles vérifiés',
};

for (const [bad, good] of Object.entries(replacements)) {
  code = code.split(bad).join(good);
}

// Ensure remaining ï¿½ are cleaned up or checked
fs.writeFileSync('src/components/SettingsPane.tsx', code, 'utf8');
console.log('Done');