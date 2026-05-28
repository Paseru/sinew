#!/usr/bin/env python3
"""Revérification Composer 2.5 standalone — auth, IdempotentSSE, alternatives."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import secrets
import sys
import time
import uuid
from pathlib import Path

import httpx

API2 = "https://api2.cursor.sh"
IDEMPOTENT_PATH = (
    "/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE"
)
BIDI_PATH = "/aiserver.v1.ChatService/StreamUnifiedChatWithTools"
AGENT5 = "https://agent.api5.cursor.sh"
CLIENT_VERSION = os.environ.get("SINEW_CURSOR_CLIENT_VERSION", "3.5.38")
STREAM_TIMEOUT = 12.0


def data_dir() -> Path:
    local = os.environ.get("LOCALAPPDATA")
    if local:
        return Path(local) / "Hyrak" / "sinew" / "data"
    return Path.home() / ".local" / "share" / "hyrak" / "sinew" / "data"


def load_token() -> str:
    auth_path = data_dir() / "cursor-composer-auth.json"
    if not auth_path.exists():
        print(f"Missing auth: {auth_path}")
        sys.exit(1)
    data = json.loads(auth_path.read_text(encoding="utf-8"))
    token = data.get("tokens", {}).get("accessToken") or data.get("accessToken")
    if not token:
        print("No accessToken in auth file")
        sys.exit(1)
    return token


def sha256_hex(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def token_machine_id(token: str) -> str:
    return sha256_hex(f"{token}machineId")


def checksum(token: str) -> str:
    machine_id = token_machine_id(token)
    millis = int(time.time() * 1000)
    bucket = millis // 1_000_000
    bytes_ = [
        (bucket >> 40) & 0xFF,
        (bucket >> 32) & 0xFF,
        (bucket >> 24) & 0xFF,
        (bucket >> 16) & 0xFF,
        (bucket >> 8) & 0xFF,
        bucket & 0xFF,
    ]
    state = 165
    out = []
    for index, byte in enumerate(bytes_):
        x = (byte ^ state) + (index % 256)
        x &= 0xFF
        out.append(x)
        state = x
    encoded = base64.urlsafe_b64encode(bytes(out)).decode().rstrip("=")
    return f"{encoded}{machine_id}"


def base_headers(token: str, session_id: str, request_id: str) -> dict[str, str]:
    return {
        "authorization": f"Bearer {token}",
        "user-agent": f"Cursor/{CLIENT_VERSION}",
        "x-cursor-client-version": CLIENT_VERSION,
        "x-cursor-client-type": "ide",
        "x-cursor-client-device-type": "desktop",
        "x-cursor-client-os": "windows",
        "x-cursor-client-arch": "x64",
        "x-ghost-mode": "false",
        "x-new-onboarding-completed": "true",
        "x-cursor-timezone": "Europe/Paris",
        "x-cursor-client-shell": "powershell",
        "x-session-id": session_id,
        "x-request-id": request_id,
        "x-client-key": sha256_hex(token),
        "x-cursor-checksum": checksum(token),
        "connect-protocol-version": "1",
        "accept": "application/connect+json",
    }


def random_key() -> bytes:
    return secrets.token_bytes(32)


def key_formats(raw: bytes) -> dict[str, str]:
    return {
        "hex": raw.hex(),
        "url": base64.urlsafe_b64encode(raw).decode().rstrip("="),
        "std": base64.standard_b64encode(raw).decode(),
    }


def connect_frame(payload: bytes, flags: int = 0) -> bytes:
    return bytes([flags]) + len(payload).to_bytes(4, "big") + payload


def minimal_payload() -> bytes:
    chunk = {
        "clientChunk": {
            "streamUnifiedChatRequest": {
                "conversation": [
                    {
                        "type": "MESSAGE_TYPE_HUMAN",
                        "text": "ping",
                        "bubbleId": str(uuid.uuid4()),
                    }
                ],
                "conversationId": str(uuid.uuid4()),
                "modelDetails": {"modelName": "composer-2.5"},
                "isAgentic": True,
                "unifiedMode": "UNIFIED_MODE_AGENT",
            }
        },
        "idempotencyKey": str(uuid.uuid4()),
        "seqno": 0,
    }
    body = connect_frame(json.dumps(chunk, separators=(",", ":")).encode())
    body += connect_frame(b"", 0x02)
    return body


def probe_stream(
    label: str,
    token: str,
    extra_headers: dict[str, str] | None = None,
    body: bytes | None = None,
    url: str = API2 + IDEMPOTENT_PATH,
) -> None:
    session_id = str(uuid.uuid4())
    request_id = str(uuid.uuid4())
    headers = base_headers(token, session_id, request_id)
    headers["content-type"] = "application/connect+json"
    if extra_headers:
        headers.update(extra_headers)
    payload = body if body is not None else minimal_payload()
    idem = headers.get("x-idempotency-key", str(uuid.uuid4()))
    headers.setdefault("x-idempotency-key", idem)
    t0 = time.monotonic()
    try:
        with httpx.Client(http2=True, timeout=STREAM_TIMEOUT) as client:
            with client.stream("POST", url, headers=headers, content=payload) as resp:
                status = resp.status_code
                detail = ""
                for line in resp.iter_lines():
                    if line:
                        detail = line[:200]
                        break
                elapsed = time.monotonic() - t0
                print(
                    f"{label:22} status={status} elapsed={elapsed:.1f}s "
                    f"detail={detail[:120]!r}"
                )
    except httpx.ReadTimeout:
        elapsed = time.monotonic() - t0
        print(f"{label:22} ERROR ReadTimeout elapsed={elapsed:.1f}s")
    except Exception as exc:
        elapsed = time.monotonic() - t0
        print(f"{label:22} ERROR {type(exc).__name__}: {exc} elapsed={elapsed:.1f}s")


def auth_checks(token: str) -> None:
    print("=== AUTH ===")
    with httpx.Client(http2=True, timeout=30.0) as client:
        for name, path in [
            ("GetEmail", "/aiserver.v1.AuthService/GetEmail"),
            ("Usage", "/aiserver.v1.UsageService/GetCurrentPeriodUsage"),
        ]:
            headers = base_headers(token, str(uuid.uuid4()), str(uuid.uuid4()))
            headers["content-type"] = "application/connect+json"
            body = connect_frame(b"{}")
            r = client.post(API2 + path, headers=headers, content=body)
            print(f"{name}: {r.status_code} bytes={len(r.content)}")


def idempotent_matrix(token: str) -> None:
    print("\n=== IDEMPOTENT SSE (api2) ===")
    raw = random_key()
    fmt = key_formats(raw)
    idem_key = str(uuid.uuid4())

    probe_stream(
        "blob only",
        token,
        {
            "x-blob-encryption-key": fmt["hex"],
            "x-idempotency-key": idem_key,
        },
    )
    probe_stream(
        "idem url only",
        token,
        {
            "x-idempotent-encryption-key": fmt["url"],
            "x-idempotency-key": idem_key,
        },
    )
    probe_stream(
        "idem hex only",
        token,
        {
            "x-idempotent-encryption-key": fmt["hex"],
            "x-idempotency-key": idem_key,
        },
    )
    probe_stream(
        "sinew both url+hex",
        token,
        {
            "x-idempotent-encryption-key": fmt["url"],
            "x-blob-encryption-key": fmt["hex"],
            "x-idempotency-key": idem_key,
        },
    )


def alternatives(token: str) -> None:
    print("\n=== ALTERNATIVES ===")
    raw = random_key()
    fmt = key_formats(raw)
    extra = {
        "x-idempotent-encryption-key": fmt["url"],
        "x-blob-encryption-key": fmt["hex"],
        "x-idempotency-key": str(uuid.uuid4()),
    }
    body = minimal_payload()

    with httpx.Client(http2=False, timeout=STREAM_TIMEOUT) as client:
        try:
            r = client.post(API2 + BIDI_PATH, headers={**base_headers(token, str(uuid.uuid4()), str(uuid.uuid4())), "content-type": "application/connect+json", **extra}, content=body)
            print(f"bidi HTTP/1.1          status={r.status_code} bytes={len(r.content)}")
        except Exception as exc:
            print(f"bidi HTTP/1.1          ERROR {exc}")

    probe_stream("bidi HTTP/2", token, extra, body, url=API2 + BIDI_PATH)
    probe_stream(
        "agent.api5 idempotent",
        token,
        extra,
        body,
        url=AGENT5 + IDEMPOTENT_PATH,
    )


def dump_replay(token: str) -> None:
    dump_path = Path(os.environ.get("TEMP", "/tmp")) / "sinew-composer-request-dump.json"
    print("\n=== DUMP REPLAY ===")
    if not dump_path.exists():
        print(f"full dump replay      SKIP (no {dump_path})")
        return
    data = json.loads(dump_path.read_text(encoding="utf-8"))
    headers = data.get("headers", {})
    # Replace dummy token from dump
    session_id = str(uuid.uuid4())
    request_id = str(uuid.uuid4())
    headers = {k: v for k, v in headers.items() if k.lower() != "authorization"}
    headers.update(base_headers(token, session_id, request_id))
    headers["content-type"] = "application/connect+json"
    url = data.get("url", API2 + IDEMPOTENT_PATH)
    hex_prefix = data.get("payload_hex_prefix", "")
    if hex_prefix:
        payload = bytes.fromhex(hex_prefix)
        # If only prefix stored, skip
        if len(payload) < 100:
            print("full dump replay      SKIP (payload_hex_prefix too short)")
            return
    else:
        print("full dump replay      SKIP (no payload hex)")
        return
    probe_stream("full dump replay", token, headers, payload, url=url)


def main() -> None:
    token = load_token()
    auth_checks(token)
    idempotent_matrix(token)
    dump_replay(token)
    alternatives(token)
    print("\nDone. sinew both url+hex doit streamer du texte quand c'est réglé.")


if __name__ == "__main__":
    main()
