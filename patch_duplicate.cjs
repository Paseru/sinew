const fs = require('fs');
let code = fs.readFileSync('src/components/SettingsPane.tsx', 'utf8');

const regex = /\{\/\*\s*Recherche Sémantique Vectorielle.*?<\/div>\s*<\/div>\n\n/s;
code = code.replace(regex, '');

fs.writeFileSync('src/components/SettingsPane.tsx', code);
