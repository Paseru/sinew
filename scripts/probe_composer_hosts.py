#!/usr/bin/env python3
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
idem = str(uuid.uuid4())
raw = bytes([1] * 32)
enc_b64url = base64.urlsafe_b64encode(raw).decode().rstrip("=")
enc_hex = raw.hex()
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
            "blobEncryptionKey": enc_b64url,
            "speculativeSummarizationEncryptionKey": enc_b64url,
        }
    },
    "idempotencyKey": idem,
    "seqno": 0,
}
payload = json.dumps(body_obj).encode()
frame = bytes([0]) + len(payload).to_bytes(4, "big") + payload + bytes([0x02, 0, 0, 0, 0])
path = "/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE"
headers_base = {
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
}
variants = [
    ("api2 idempotent-b64", "https://api2.cursor.sh", {**headers_base, "x-idempotent-encryption-key": enc_b64url}),
    ("api2 blob-hex", "https://api2.cursor.sh", {**headers_base, "x-blob-encryption-key": enc_hex}),
    ("agent.api5 idempotent-b64", "https://agent.api5.cursor.sh", {**headers_base, "x-idempotent-encryption-key": enc_b64url}),
    ("agent.api5 blob-hex", "https://agent.api5.cursor.sh", {**headers_base, "x-blob-encryption-key": enc_hex}),
]

for name, host, headers in variants:
    url = host + path
    started = time.time()
    try:
        with httpx.Client(http2=True, timeout=25.0) as client:
            with client.stream("POST", url, headers=headers, content=frame) as response:
                first_byte_at = None
                chunks = []
                for chunk in response.iter_bytes():
                    if first_byte_at is None:
                        first_byte_at = time.time() - started
                    chunks.append(chunk)
                    if sum(len(part) for part in chunks) > 200 or time.time() - started > 24:
                        break
                data = b"".join(chunks)
                content_type = response.headers.get("content-type", "")
                print(
                    f"{name}: status={response.status_code} "
                    f"first_byte_dt={first_byte_at} total_bytes={len(data)} "
                    f"ct={content_type} preview={data[:120]!r}"
                )
    except Exception as err:
        print(f"{name}: ERROR after {time.time() - started:.1f}s -> {type(err).__name__}: {err}")
