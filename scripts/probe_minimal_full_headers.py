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
machine_id = hashlib.sha256(f"{token}machineId".encode()).hexdigest()


def cursor_checksum(machine_id: str) -> str:
    millis = int(time.time() * 1000)
    bucket = millis // 1_000_000
    bytes_arr = [
        (bucket >> 40) & 0xFF,
        (bucket >> 32) & 0xFF,
        (bucket >> 24) & 0xFF,
        (bucket >> 16) & 0xFF,
        (bucket >> 8) & 0xFF,
        bucket & 0xFF,
    ]
    state = 165
    out = []
    for index, byte in enumerate(bytes_arr):
        x = (byte ^ state) & 0xFF
        x = (x + (index % 256)) & 0xFF
        state = x
        out.append(x)
    encoded = base64.urlsafe_b64encode(bytes(out)).decode().rstrip("=")
    return f"{encoded}{machine_id}"


raw = bytes([9] * 32)
enc_std = base64.b64encode(raw).decode()
enc_hex = raw.hex()
idem = str(uuid.uuid4())
request_id = str(uuid.uuid4())
session_id = str(uuid.uuid4())

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
            "conversationId": "probe",
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
            "blobEncryptionKey": enc_std,
            "speculativeSummarizationEncryptionKey": enc_std,
        }
    },
    "idempotencyKey": idem,
    "seqno": 0,
}
payload = json.dumps(body_obj).encode()
frame = bytes([0]) + len(payload).to_bytes(4, "big") + payload + bytes([0x02, 0, 0, 0, 0])
url = "https://api2.cursor.sh/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE"

headers = {
    "authorization": f"Bearer {token}",
    "content-type": "application/connect+json",
    "connect-protocol-version": "1",
    "accept": "application/connect+json",
    "connect-accept-encoding": "gzip",
    "user-agent": "Cursor/3.5.38",
    "x-cursor-client-version": "3.5.38",
    "x-cursor-client-type": "ide",
    "x-cursor-client-device-type": "desktop",
    "x-cursor-client-os": "windows",
    "x-cursor-client-arch": "x64",
    "x-cursor-timezone": "Romance Standard Time",
    "x-ghost-mode": "false",
    "x-new-onboarding-completed": "true",
    "x-client-key": client_key,
    "x-cursor-checksum": cursor_checksum(machine_id),
    "x-session-id": session_id,
    "x-request-id": request_id,
    "x-amzn-trace-id": f"Root={request_id}",
    "x-idempotency-key": idem,
    "x-idempotent-encryption-key": enc_std,
    "x-blob-encryption-key": enc_hex,
}

started = time.time()
with httpx.Client(http2=True, timeout=40.0) as client:
    with client.stream("POST", url, headers=headers, content=frame) as response:
        chunks = []
        for chunk in response.iter_bytes():
            chunks.append(chunk)
            if sum(len(c) for c in chunks) > 500 or time.time() - started > 35:
                break
        data = b"".join(chunks)
        print("status", response.status_code, "dt", round(time.time() - started, 2))
        print("bytes", len(data), "preview", data[:300])
