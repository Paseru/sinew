const fs = require('fs');
let chatPane = fs.readFileSync('src/components/chat/ChatPane.tsx', 'utf8');
chatPane = chatPane.replace('safeTokenCount, ', '');
fs.writeFileSync('src/components/chat/ChatPane.tsx', chatPane);
