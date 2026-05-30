const fs = require('fs');
let code = fs.readFileSync('src/components/SettingsPane.tsx', 'utf8');

code = code.replace(/ï¿½Ü¬ï¸ /g, '☀️');
code = code.replace(/ï¿½x   /g, '⚙️ ');
code = code.replace(/ï¿½x ⏳/g, '💻 ');
code = code.replace(/ï¿½S  /g, '✅ ');
code = code.replace(/ï¿½ /g, '— ');
code = code.replace(/ï¿½/g, '—');

fs.writeFileSync('src/components/SettingsPane.tsx', code, 'utf8');
console.log('Fixed 3');