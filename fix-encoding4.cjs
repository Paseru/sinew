const fs = require('fs');
let code = fs.readFileSync('src/components/SettingsPane.tsx', 'utf8');

code = code.replace(/—Ü¬ï¸ /g, '☀️');
code = code.replace(/—x /g, '💻 ');
code = code.replace(/⚡ï¸ /g, '⚠️');

fs.writeFileSync('src/components/SettingsPane.tsx', code, 'utf8');
console.log('Fixed 4');