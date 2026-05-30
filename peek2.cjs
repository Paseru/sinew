const fs = require('fs');
const code = fs.readFileSync('src/components/chat/TodoStrip.tsx', 'utf8');
console.log(code.substring(24500, 25500));
