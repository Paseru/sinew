const fs = require('fs');

let chatPane = fs.readFileSync('src/components/chat/ChatPane.tsx', 'utf8');

const fnsToExtract = [
    /function formatTurnDuration\(durationMs: number\): string \{[\s\S]*?\n\}/,
    /function formatFullTokenCount\(value: number\): string \{[\s\S]*?\n\}/,
    /function formatCompactTokenCount\(value: number\): string \{[\s\S]*?\n\}/,
    /function safeTokenCount\(value: number \| undefined\): number \{[\s\S]*?\n\}/,
    /function hashString\(value: string\): string \{[\s\S]*?\n\}/
];

let utilsCode = 'import { ChatBlock, ChatMessage, Part } from "../../types";\n\n';

for (const regex of fnsToExtract) {
    const match = chatPane.match(regex);
    if (match) {
        // Add export
        const code = match[0].replace(/^function/, 'export function');
        utilsCode += code + '\n\n';
        // Remove from ChatPane
        chatPane = chatPane.replace(match[0], '');
    }
}

// Add imports to ChatPane
const importStatement = 'import {\n  formatTurnDuration,\n  formatFullTokenCount,\n  formatCompactTokenCount,\n  safeTokenCount,\n  hashString\n} from "./chatUtils";\n';
chatPane = chatPane.replace(/(import .*?\n)+/, (match) => match + importStatement);

fs.writeFileSync('src/components/chat/chatUtils.ts', utilsCode);
fs.writeFileSync('src/components/chat/ChatPane.tsx', chatPane);
