const fs = require('fs');
let code = fs.readFileSync('src/components/SettingsPane.tsx', 'utf8');

// The file has some corrupted sequences, let's fix them with regex or simple replacements

// "Cursor connectÃ© ï¿½  vous pouvez..." => "Cursor connecté — vous pouvez..."
code = code.replace(/Cursor connect\u00C3\u00A9 \u00EF\u00BF\u00BD  vous/g, 'Cursor connecté — vous');
code = code.replace(/Cursor connectÃ© ï¿½  vous/g, 'Cursor connecté — vous');

// "return to Cursor Â» ï¿½  c'est normal" => "return to Cursor » — c'est normal"
code = code.replace(/Â» ï¿½  c'est normal/g, "» — c'est normal");

// "ï¿½Ü¬ï¸  Jour" => "☀️ Jour"
code = code.replace(/ï¿½Ü¬ï¸  Jour/g, '☀️ Jour');
code = code.replace(/ï¿½Ü¬ï¸  Day/g, '☀️ Day');

// "ï¿½x ⏳Système" => "💻 Système"
// wait, we replaced part of it already?
code = code.replace(/ï¿½x ⏳Système/g, '💻 Système');
code = code.replace(/ï¿½x ⏳System/g, '💻 System');

// "ï¿½x   CARTE GENERALE 3" => "⚙️ CARTE GENERALE 3"
code = code.replace(/ï¿½x   CARTE GENERALE 3/g, '⚙️ CARTE GENERALE 3');

// "JulienPiron.fr ï¿½  AmÃ©liorations ClÃ©s" => "JulienPiron.fr — Améliorations Clés"
code = code.replace(/JulienPiron\.fr ï¿½  AmÃ©liorations ClÃ©s/g, 'JulienPiron.fr — Améliorations Clés');
code = code.replace(/JulienPiron\.fr Fork ï¿½  Key Enhancements/g, 'JulienPiron.fr Fork — Key Enhancements');

// percent == null ? "ï¿½ " : 
code = code.replace(/"ï¿½ "/g, '"⏳ "');

// No servers yet ï¿½  add
code = code.replace(/No servers yet ï¿½  add/g, 'No servers yet — add');

// No sub-agents yet ï¿½  click
code = code.replace(/No sub-agents yet ï¿½  click/g, 'No sub-agents yet — click');

// "ï¿½S  ModÃ¨les vÃ©rifiÃ©s"
code = code.replace(/ï¿½S  ModÃ¨les vÃ©rifiÃ©s/g, '✅ Modèles vérifiés');
code = code.replace(/ï¿½S  ModÃ¨les/g, '✅ Modèles');

// Replace general broken A tilde sequences
code = code.replace(/AmÃ©liorations/g, 'Améliorations');
code = code.replace(/ClÃ©s/g, 'Clés');
code = code.replace(/ParamÃ¨tres/g, 'Paramètres');
code = code.replace(/vÃ©rifiÃ©s/g, 'vérifiés');
code = code.replace(/ModÃ¨les/g, 'Modèles');
code = code.replace(/systÃ¨me/g, 'système');
code = code.replace(/Ã /g, 'à');
code = code.replace(/aprÃ¨s/g, 'après');
code = code.replace(/sÃ©curisÃ©/g, 'sécurisé');
code = code.replace(/Ã©galement/g, 'également');
code = code.replace(/premiÃ¨re/g, 'première');
code = code.replace(/LibÃ©ration/g, 'Libération');
code = code.replace(/CÃ´te/g, 'Côte');
code = code.replace(/Â«/g, '«');
code = code.replace(/Â»/g, '»');

fs.writeFileSync('src/components/SettingsPane.tsx', code, 'utf8');
console.log('Fixed');