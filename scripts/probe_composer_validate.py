#!/usr/bin/env python3
"""Validate winning vs failing Composer configs with longer read + decode."""
from __future__ import annotations

import base64
import hashlib
import json
import pathlib
import re
import sys
import time
import uuid

import httpx

AUTH = pathlib.Path.home() / "AppData/Local/Hyrak/sinew/data/cursor-composer-auth.json"
URL = "https://api2.cursor.sh/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE"


def checksum_rust(machine_id: str) -> str:
    millis = int(time.time() * 1000)
    bucket = millis // 1_000_000
    b = [
        (bucket >> 40) & 0xFF,
        (bucket >> 32) & 0xFF,
        (bucket >> 24) & 0xFF,
        (bucket >> 16) & 0xFF,
        (bucket >> 8) & 0xFF,
        bucket & 0xFF,
    ]
    state = 165
    for idx in range(len(b)):
        b[idx] = ((b[idx] ^ state) + (idx % 256)) & 0xFF
        state = b[idx]
    return base64.urlsafe_b64encode(bytes(b)).decode().rstrip("=") + machine_id


def frame(body: dict) -> bytes:
    p = json.dumps(body).encode()
    return bytes([0]) + len(p).to_bytes(4, "big") + p + bytes([0x02, 0, 0, 0, 0])


def decode_frames(data: bytes) -> list[str]:
    out = []
    i = 0
    while i + 5 <= len(data):
        length = int.from_bytes(data[i + 1 : i + 5], "big")
        if i + 5 + length > len(data):
            break
        payload = data[i + 5 : i + 5 + length]
        if payload:
            try:
                j = json.loads(payload)
                if "error" in j:
                    out.append(f"ERROR: {json.dumps(j['error'])[:200]}")
                elif "serverChunk" in j:
                    out.append("serverChunk")
                else:
                    out.append(str(list(j.keys())[:4]))
            except json.JSONDecodeError:
                out.append(f"raw:{payload[:60]!r}")
        i += 5 + length
    return out


def minimal(model: str, body_key: str, idem: str) -> dict:
    return {
        "clientChunk": {
            "streamUnifiedChatRequest": {
                "conversation": [
                    {
                        "type": "MESSAGE_TYPE_HUMAN",
                        "text": "Reply with exactly: OK",
                        "bubbleId": str(uuid.uuid4()),
                        "requestId": str(uuid.uuid4()),
                    }
                ],
                "conversationId": "validate",
                "modelDetails": {"modelName": model, "enableSlowPool": False, "maxMode": True},
                "isAgentic": True,
                "isChat": False,
                "unifiedMode": "UNIFIED_MODE_AGENT",
                "useUnifiedChatPrompt": True,
                "supportedTools": ["CLIENT_SIDE_TOOL_V2_READ_FILE_V2"],
                "blobEncryptionKey": body_key,
                "speculativeSummarizationEncryptionKey": body_key,
            }
        },
        "idempotencyKey": idem,
        "seqno": 0,
    }


def probe(label: str, token: str, headers: dict, body: dict, *, read_s: float = 25.0) -> None:
    started = time.time()
    data = b""
    status = "?"
    try:
        with httpx.Client(http2=True, timeout=httpx.Timeout(read_s + 5, connect=12.0)) as client:
            with client.stream("POST", URL, headers=headers, content=frame(body)) as resp:
                status = resp.status_code
                http = resp.http_version
                for chunk in resp.iter_bytes():
                    data += chunk
                    if len(data) > 1200 or time.time() - started > read_s:
                        break
        dt = time.time() - started
        text = data.decode("utf-8", "replace")
        has_unauth = "unauth" in text.lower()
        has_text = "OK" in text or "text" in text.lower() or "serverChunk" in text
        frames = decode_frames(data)
        print(f"\n--- {label} ---")
        print(f"status={status} http={http} dt={dt:.1f}s bytes={len(data)} unauth={has_unauth} has_content={has_text}")
        if frames:
            print("frames:", "; ".join(frames[:6]))
        if has_unauth:
            m = re.search(r'"detail":"([^"]+)"', text)
            print("detail:", m.group(1) if m else text[:200])
        elif len(data) < 200:
            print("preview:", repr(data[:200]))
    except httpx.ReadTimeout:
        dt = time.time() - started
        frames = decode_frames(data)
        print(f"\n--- {label} ---")
        print(f"READ_TIMEOUT dt={dt:.1f}s bytes={len(data)} (stream actif si bytes>0)")
        if frames:
            print("frames:", "; ".join(frames[:6]))
        elif data:
            print("preview:", repr(data[:200]))
    except Exception as err:
        print(f"\n--- {label} --- ERROR {type(err).__name__}: {err}")


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8", errors="backslashreplace")
    auth = json.loads(AUTH.read_text())
    token = auth["tokens"]["accessToken"]
    bad_token = token[:-4] + "XXXX"
    machine = hashlib.sha256(f"{token}machineId".encode()).hexdigest()
    client_key = hashlib.sha256(token.encode()).hexdigest()
    raw = bytes([0xCD] * 32)
    enc_std = base64.b64encode(raw).decode()
    enc_url = base64.urlsafe_b64encode(raw).decode().rstrip("=")
    enc_hex = raw.hex()

    def headers_for(tok: str, extra: dict, idem: str) -> dict:
        mid = hashlib.sha256(f"{tok}machineId".encode()).hexdigest()
        return {
            "authorization": f"Bearer {tok}",
            "content-type": "application/connect+json",
            "connect-protocol-version": "1",
            "accept": "application/connect+json",
            "user-agent": "Cursor/3.5.38",
            "x-cursor-client-version": "3.5.38",
            "x-cursor-client-type": "ide",
            "x-cursor-client-device-type": "desktop",
            "x-cursor-client-os": "windows",
            "x-cursor-client-arch": "x64",
            "x-client-key": hashlib.sha256(tok.encode()).hexdigest(),
            "x-cursor-checksum": checksum_rust(mid),
            "x-ghost-mode": "false",
            "x-new-onboarding-completed": "true",
            "x-idempotency-key": idem,
            **extra,
        }

    print("=== VALIDATION CIBLÉE ===")

    # WIN: Sinew production
    idem = str(uuid.uuid4())
    probe(
        "WIN sinew url+hex / body std / 2.5",
        token,
        headers_for(token, {"x-idempotent-encryption-key": enc_url, "x-blob-encryption-key": enc_hex}, idem),
        minimal("composer-2.5", enc_std, idem),
        read_s=30,
    )

    idem = str(uuid.uuid4())
    probe(
        "WIN fast model",
        token,
        headers_for(token, {"x-idempotent-encryption-key": enc_url, "x-blob-encryption-key": enc_hex}, idem),
        minimal("composer-2.5-fast", enc_std, idem),
        read_s=30,
    )

    # FAIL: missing idem header
    idem = str(uuid.uuid4())
    probe(
        "FAIL blob only",
        token,
        headers_for(token, {"x-blob-encryption-key": enc_hex}, idem),
        minimal("composer-2.5", enc_std, idem),
        read_s=5,
    )

    # FAIL: hex idem key
    idem = str(uuid.uuid4())
    probe(
        "FAIL idem hex header",
        token,
        headers_for(token, {"x-idempotent-encryption-key": enc_hex, "x-blob-encryption-key": enc_hex}, idem),
        minimal("composer-2.5", enc_std, idem),
        read_s=5,
    )

    # FAIL: bad token
    idem = str(uuid.uuid4())
    probe(
        "FAIL bad token",
        bad_token,
        headers_for(
            bad_token,
            {"x-idempotent-encryption-key": enc_url, "x-blob-encryption-key": enc_hex},
            idem,
        ),
        minimal("composer-2.5", enc_std, idem),
        read_s=8,
    )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
