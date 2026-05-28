#!/usr/bin/env python3
"""Probe agent.v1 AgentService with Sinew OAuth token (no IdempotentSSE)."""

from __future__ import annotations

import json
import sys
import uuid
from pathlib import Path

import httpx

sys.path.insert(0, str(Path(__file__).parent))
from verify_all import API2, base_headers, connect_frame, load_token

GET_MODELS = "/agent.v1.AgentService/GetUsableModels"
RUN_SSE = "/agent.v1.AgentService/RunSSE"
TIMEOUT = 15.0


def post(path: str, token: str, body: bytes, content_type: str, framed: bool) -> tuple[int, bytes, str]:
    session_id = str(uuid.uuid4())
    request_id = str(uuid.uuid4())
    headers = base_headers(token, session_id, request_id)
    headers["content-type"] = content_type
    headers["te"] = "trailers"
    headers["x-cursor-client-type"] = "cli"
    headers["x-ghost-mode"] = "true"
    if content_type.startswith("application/connect"):
        headers["connect-protocol-version"] = "1"
    payload = connect_frame(body) if framed else body
    with httpx.Client(http2=True, timeout=TIMEOUT) as client:
        r = client.post(API2 + path, headers=headers, content=payload)
        return r.status_code, r.content, r.headers.get("content-type", "")


def main() -> None:
    token = load_token()
    print("=== agent.v1 PROBE (Sinew OAuth) ===\n")

    variants = [
        ("GetUsableModels connect+proto framed {}", "application/connect+proto", b"{}", True),
        ("GetUsableModels connect+proto empty", "application/connect+proto", b"", True),
        ("GetUsableModels proto empty", "application/proto", b"", False),
    ]
    for label, ctype, body, framed in variants:
        status, resp, rctype = post(GET_MODELS, token, body, ctype, framed)
        print(f"{label:40} status={status} bytes={len(resp)} ctype={rctype!r}")
        if resp:
            print(f"  prefix={resp[:64]!r}")

    print("\nIf all 415: ajuster framing (voir cursor-oauth-opencode h2-bridge unary mode).")
    print("If 200 on any variant: OAuth agent.v1 OK -> spike Rust.")


if __name__ == "__main__":
    main()
