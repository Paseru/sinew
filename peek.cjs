const fs = require('fs');
const code = fs.readFileSync('crates/sinew-app/src/agent/turn.rs', 'utf8');

const startIndex = code.indexOf('let result = run_tool(');
console.log(code.substring(startIndex, startIndex + 2000));
