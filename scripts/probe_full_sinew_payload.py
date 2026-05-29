#!/usr/bin/env python3
"""Send the exact Sinew dump payload with real auth and header variants."""
import base64
import hashlib
import json
import pathlib
import time
import uuid

import httpx

DUMP = pathlib.Path.home() / "AppData/Local/Temp/sinew-composer-request-dump.json"
auth = json.loads(
    (pathlib.Path.home() / "AppData/Local/Hyrak/sinew/data/cursor-composer-auth.json").read_text()
)
token = auth["tokens"]["accessToken"]
client_key = hashlib.sha256(token.encode()).hexdigest()

dump = json.loads(DUMP.read_text())
frame_json = dump["connect_frames"][0]["json"]
idem = str(uuid.uuid4())
raw = bytes([7] * 32)
enc_std = base64.b64encode(raw).decode()
enc_url = base64.urlsafe_b64encode(raw).decode().rstrip("=")
enc_hex = raw.hex()

# Patch keys in dumped body
body = json.loads(json.dumps(frame_json))
req = body["clientChunk"]["streamUnifiedChatRequest"]
req["blobEncryptionKey"] = enc_std
req["speculativeSummarizationEncryptionKey"] = enc_std
body["idempotencyKey"] = idem
body["seqno"] = 0
payload = json.dumps(body).encode()
frame = bytes([0]) + len(payload).to_bytes(4, "big") + payload + bytes([0x02, 0, 0, 0, 0])

url = dump["url"]
base = {
    "authorization": f"Bearer {token}",
    "content-type": "application/connect+json",
    "connect-protocol-version": "1",
    "accept": "application/connect+json",
    "user-agent": "Cursor/3.5.38",
    "x-cursor-client-version": "3.5.38",
    "x-cursor-client-type": "ide",
    "x-cursor-client-device-type": "desktop",
    "x-cursor-client-os": "windows",
    "x-cursor-client-arch": "x64",
    "x-client-key": client_key,
    "x-idempotency-key": idem,
    "x-ghost-mode": "false",
    "x-new-onboarding-completed": "true",
}

variants = [
    ("sinew-current url+no-blob", {**base, "x-idempotent-encryption-key": enc_url}),
    ("fixed std+blob-hex", {**base, "x-idempotent-encryption-key": enc_std, "x-blob-encryption-key": enc_hex}),
    ("fixed url+blob-hex", {**base, "x-idempotent-encryption-key": enc_url, "x-blob-encryption-key": enc_hex}),
]

for label, headers in variants:
    started = time.time()
    try:
        with httpx.Client(http2=True, timeout=35.0) as client:
            with client.stream("POST", url, headers=headers, content=frame) as response:
                chunks = []
                for chunk in response.iter_bytes():
                    chunks.append(chunk)
                    if sum(len(c) for c in chunks) > 400 or time.time() - started > 32:
                        break
                data = b"".join(chunks)
                print(
                    f"{label}: status={response.status_code} dt={time.time()-started:.1f}s "
                    f"bytes={len(data)} preview={data[:160]!r}"
                )
    except Exception as err:
        print(f"{label}: ERROR dt={time.time()-started:.1f}s {type(err).__name__}: {err}")
