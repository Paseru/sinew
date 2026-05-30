#!/usr/bin/env python3
"""E2E checks mirroring Sinew Composer OAuth + API health (no UI)."""
from __future__ import annotations

import json
import os
import sys
import uuid
from pathlib import Path

try:
    import httpx
except ImportError:
    print("pip install httpx")
    sys.exit(1)

API2 = "https://api2.cursor.sh"
AUTH_PATH = Path(os.environ.get("LOCALAPPDATA", "")) / "Hyrak" / "sinew" / "data" / "cursor-composer-auth.json"
RUN_URL = f"{API2}/agent.v1.AgentService/Run"


def load_token() -> str:
    if not AUTH_PATH.is_file():
        raise SystemExit(f"auth manquant: {AUTH_PATH}")
    data = json.loads(AUTH_PATH.read_text(encoding="utf-8"))
    token = (data.get("tokens") or {}).get("accessToken") or ""
    if not token.strip():
        raise SystemExit("accessToken vide")
    return token.strip()


def token_machine_id(token: str) -> str:
    import hashlib

    return hashlib.sha256(f"{token}machineId".encode()).hexdigest()


def token_client_key(token: str) -> str:
    import hashlib

    return hashlib.sha256(token.encode()).hexdigest()


def checksum(machine_id: str) -> str:
    import hashlib
    import time

    millis = int(time.time() * 1000)
    bucket = millis // 1_000_000
    seed = bytearray(
        [
            (bucket >> 40) & 0xFF,
            (bucket >> 32) & 0xFF,
            (bucket >> 24) & 0xFF,
            (bucket >> 16) & 0xFF,
            (bucket >> 8) & 0xFF,
            bucket & 0xFF,
        ]
    )
    seed.extend(machine_id.encode())
    digest = hashlib.sha256(bytes(seed)).hexdigest()
    return f"{digest[:32]}/{digest[32:]}01"


def cli_headers(token: str) -> dict[str, str]:
    mid = token_machine_id(token)
    return {
        "authorization": f"Bearer {token}",
        "x-client-key": token_client_key(token),
        "x-cursor-checksum": checksum(mid),
        "x-cursor-client-type": "cli",
        "x-ghost-mode": "true",
        "x-cursor-client-version": "2025.5.0",
        "content-type": "application/connect+proto",
        "connect-protocol-version": "1",
        "te": "trailers",
    }


def get_email(token: str) -> tuple[int, str]:
    url = f"{API2}/aiserver.v1.AuthService/GetEmail"
    headers = cli_headers(token)
    headers["content-type"] = "application/json"
    with httpx.Client(http2=True, timeout=30.0) as client:
        r = client.post(url, headers=headers, content=b"{}")
        return r.status_code, r.text[:200]


def main() -> int:
    print("=== Sinew Composer E2E (Python) ===")
    print(f"auth: {AUTH_PATH}")
    token = load_token()
    print(f"token: {len(token)} chars")

    status, body = get_email(token)
    print(f"GetEmail: HTTP {status} {body!r}")
    if status != 200:
        print("FAIL: OAuth refusé — reconnectez dans Sinew Réglages → Fournisseurs")
        return 1

    print("OK: session OAuth valide")
    print()
    print("Pour le stream agent.v1 complet, lancez:")
    print("  cd C:\\Dev\\sinew")
    print("  $env:SINEW_CURSOR_LIVE_ASSERT='1'")
    print("  cargo test -p sinew-cursor test_live_sinew_composer -- --ignored --nocapture")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
