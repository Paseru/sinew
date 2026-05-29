#!/usr/bin/env python3
"""Dump the exact Composer request Sinew would send (headers + Connect frames)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST = r"""
use sinew_core::{ChatMessage, ModelRef, ProviderRequest};
use sinew_cursor::{conversation::build_stream_request, identity::CursorIdeIdentity};

fn main() {
    let request = ProviderRequest::new(
        ModelRef::new("cursor", "composer-2.5"),
        vec![ChatMessage::user_text("Say OK")],
    )
    .with_workspace_root(r"C:\Dev\sinew")
    .with_cache_key("dump-test-conv");

    let identity = CursorIdeIdentity::load();
    let idempotency_key = "00000000-0000-4000-8000-000000000001";
    let encryption_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"; // fixed 32-byte b64url
    let (payload, next_seqno) = build_stream_request(
        &request,
        "dump-test-conv",
        idempotency_key,
        0,
        &identity,
        encryption_key,
    );

    let mut frames = Vec::new();
    let mut buf = payload.clone();
    while buf.len() >= 5 {
        let flags = buf[0];
        let length = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
        if buf.len() < 5 + length {
            break;
        }
        let body = &buf[5..5 + length];
        frames.push(serde_json::json!({
            "flags": format!("0x{flags:02x}"),
            "length": length,
            "json": serde_json::from_slice::<serde_json::Value>(body).ok(),
            "raw_preview": String::from_utf8_lossy(body).chars().take(200).collect::<String>(),
        }));
        buf = buf[5 + length..].to_vec();
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let token = std::env::var("SINEW_CURSOR_TOKEN").unwrap_or_else(|_| "dummy-token-for-dump".into());
    let mut headers = reqwest::header::HeaderMap::new();
    identity.apply_authenticated(&mut headers, &session_id, &request_id, &token);
    headers.insert(
        reqwest::header::HeaderName::from_static("x-idempotency-key"),
        reqwest::header::HeaderValue::from_str(idempotency_key).unwrap(),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("x-idempotent-encryption-key"),
        reqwest::header::HeaderValue::from_str(encryption_key).unwrap(),
    );
    headers.insert(reqwest::header::AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        "application/connect+json".parse().unwrap(),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("connect-protocol-version"),
        "1".parse().unwrap(),
    );
    headers.insert(reqwest::header::ACCEPT, "application/connect+json".parse().unwrap());

    let header_map: serde_json::Map<String, serde_json::Value> = headers
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), serde_json::Value::String(v.to_str().unwrap_or("").to_string())))
        .collect();

    let out = serde_json::json!({
        "url": "https://api2.cursor.sh/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE",
        "next_seqno": next_seqno,
        "payload_bytes": payload.len(),
        "payload_hex_prefix": payload.iter().take(64).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(""),
        "headers": header_map,
        "connect_frames": frames,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
"""


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        src = tmp_path / "dump_sinew_request.rs"
        src.write_text(RUST, encoding="utf-8")
        manifest = tmp_path / "Cargo.toml"
        core_path = (ROOT / "crates" / "sinew-core").as_posix()
        cursor_path = (ROOT / "crates" / "sinew-cursor").as_posix()
        manifest.write_text(
            f"""[package]
name = "dump_sinew_request"
version = "0.1.0"
edition = "2021"

[dependencies]
sinew-core = {{ path = "{core_path}" }}
sinew-cursor = {{ path = "{cursor_path}" }}
serde_json = "1"
uuid = {{ version = "1", features = ["v4"] }}
reqwest = {{ version = "0.12", default-features = false, features = ["rustls-tls"] }}
""",
            encoding="utf-8",
        )
        (tmp_path / "src").mkdir()
        (tmp_path / "src/main.rs").write_text(
            'include!("../dump_sinew_request.rs");\n', encoding="utf-8"
        )
        result = subprocess.run(
            ["cargo", "run", "--quiet", "--manifest-path", str(manifest)],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(result.stderr, file=sys.stderr)
            return result.returncode
        print(result.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
