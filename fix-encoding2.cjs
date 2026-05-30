const fs = require('fs');
let code = fs.readFileSync('src/components/SettingsPane.tsx', 'utf8');

const replacements = {
  'ï¿½ ': '⏳ ',
  'ï¿½S ': '✅ ',
  'Ã©': 'é',
  'Ã¨': 'è',
  'Ãª': 'ê',
  'Ã ': 'à',
  'Ã¢': 'â',
  'Ã®': 'î',
  'Ã§': 'ç',
  'Ã´': 'ô',
  'ï¿½x ': '💻 '
};

for (const [bad, good] of Object.entries(replacements)) {
  code = code.split(bad).join(good);
}

fs.writeFileSync('src/components/SettingsPane.tsx', code, 'utf8');
console.log('Fixed 2');