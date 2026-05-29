#!/usr/bin/env python3
"""Probe idempotent encryption key formats with x-blob-encryption-key present."""
import base64
import hashlib
import json
import pathlib
import time
import uuid

import httpx

auth = json.loads(
    (pathlib.Path.home() / "AppData/Local/Hyrak/sinew/data/cursor-composer-auth.json").read_text()
)
token = auth["tokens"]["accessToken"]
client_key = hashlib.sha256(token.encode()).hexdigest()
url = "https://api2.cursor.sh/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE"


def frame(body_key: str, idem: str) -> bytes:
    body_obj = {
        "clientChunk": {
            "streamUnifiedChatRequest": {
                "conversation": [
                    {
                        "type": "MESSAGE_TYPE_HUMAN",
                        "text": "Say OK",
                        "bubbleId": str(uuid.uuid4()),
                        "requestId": str(uuid.uuid4()),
                    }
                ],
                "conversationId": "probe-conv",
                "modelDetails": {
                    "modelName": "composer-2.5",
                    "enableSlowPool": False,
                    "maxMode": True,
                },
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
    payload = json.dumps(body_obj).encode()
    return bytes([0]) + len(payload).to_bytes(4, "big") + payload + bytes([0x02, 0, 0, 0, 0])


raw = bytes([4] * 32)
enc_std = base64.b64encode(raw).decode()
enc_url = base64.urlsafe_b64encode(raw).decode().rstrip("=")
enc_hex = raw.hex()
idem = str(uuid.uuid4())

variants = [
    ("body=std, idem=std", enc_std, enc_std),
    ("body=std, idem=url", enc_std, enc_url),
    ("body=url, idem=std", enc_url, enc_std),
    ("body=hex, idem=std", enc_hex, enc_std),
    ("body=std, idem=hex", enc_std, enc_hex),
    ("body=std, idem=std+pad copy", enc_std, enc_std),
]

base_headers = {
    "authorization": f"Bearer {token}",
    "content-type": "application/connect+json",
    "connect-protocol-version": "1",
    "accept": "application/connect+json",
    "user-agent": "Cursor/3.5.38",
    "x-cursor-client-version": "3.5.38",
    "x-cursor-client-type": "ide",
    "x-cursor-client-device-type": "desktop",
    "x-client-key": client_key,
    "x-idempotency-key": idem,
    "x-blob-encryption-key": enc_hex,
}

for label, body_key, idem_key in variants:
    headers = {**base_headers, "x-idempotent-encryption-key": idem_key}
    started = time.time()
    try:
        response = httpx.post(url, headers=headers, content=frame(body_key, idem), timeout=18)
        text = response.content.decode("utf-8", "replace")
        detail = ""
        if "detail" in text:
            start = text.find('"detail"')
            detail = text[start : start + 120]
        print(
            f"{label}: status={response.status_code} dt={time.time() - started:.1f}s "
            f"bytes={len(response.content)} {detail}"
        )
    except Exception as err:
        print(f"{label}: ERROR dt={time.time() - started:.1f}s {type(err).__name__}: {err}")
