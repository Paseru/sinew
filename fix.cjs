const fs = require('fs');

let chatUtils = fs.readFileSync('src/components/chat/chatUtils.ts', 'utf8');
chatUtils = chatUtils.replace('import { ChatBlock, ChatMessage, Part } from "../../types";\n\n', '');
fs.writeFileSync('src/components/chat/chatUtils.ts', chatUtils);

let chatPane = fs.readFileSync('src/components/chat/ChatPane.tsx', 'utf8');
if (!chatPane.includes('formatFullTokenCount')) {
    console.log("Adding imports to ChatPane");
}
chatPane = 'import { formatTurnDuration, formatFullTokenCount, formatCompactTokenCount, safeTokenCount, hashString } from "./chatUtils";\n' + chatPane;
fs.writeFileSync('src/components/chat/ChatPane.tsx', chatPane);
