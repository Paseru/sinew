use serde_json::Value;

const BRAND_REPLACEMENTS: &[(&str, &str)] = &[
    ("Sinew", "Cursor"),
    ("sinew", "cursor"),
    ("SINEW", "CURSOR"),
    ("Hyrak", "Cursor"),
    ("hyrak", "cursor"),
    ("HYRAK", "CURSOR"),
    ("Hyrrak", "Cursor"),
    ("hyrrak", "cursor"),
    ("HYRRAK", "CURSOR"),
];

pub fn sanitize_outbound_text(text: &str) -> String {
    let mut out = text.to_string();
    for (from, to) in BRAND_REPLACEMENTS {
        if out.contains(from) {
            out = out.replace(from, to);
        }
    }
    out.trim().to_string()
}

pub fn sanitize_outbound_json(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(sanitize_outbound_text(&text)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(sanitize_outbound_json)
                .collect::<Vec<_>>(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (sanitize_outbound_text(&key), sanitize_outbound_json(value)))
                .collect(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_brand_variants() {
        let text = sanitize_outbound_text("Sinew/Hyrak/hyrrak in SINEW path");
        assert!(!text.to_ascii_lowercase().contains("sinew"));
        assert!(!text.to_ascii_lowercase().contains("hyrak"));
        assert!(text.contains("Cursor"));
    }

    #[test]
    fn sanitizes_json_recursively() {
        let value = sanitize_outbound_json(serde_json::json!({
            "message": "Sinew MCP error",
            "path": "C:\\Dev\\sinew\\src\\main.rs",
            "nested": [{ "note": "Hyrak team" }]
        }));
        assert_eq!(value["message"], "Cursor MCP error");
        assert_eq!(value["nested"][0]["note"], "Cursor team");
    }
}
