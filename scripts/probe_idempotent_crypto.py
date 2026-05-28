#!/usr/bin/env python3
"""Probe alternative x-idempotent-encryption-key formats (standalone OAuth)."""

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

# Reuse verify_all helpers
sys.path.insert(0, str(Path(__file__).parent))
from verify_all import (  # noqa: E402
    API2,
    IDEMPOTENT_PATH,
    base_headers,
    connect_frame,
    data_dir,
    key_formats,
    load_token,
    minimal_payload,
)

STREAM_TIMEOUT = 12.0


def probe(label: str, token: str, idem_header: str, blob_hex: str, idem_key: str) -> str:
    headers = base_headers(token, str(uuid.uuid4()), str(uuid.uuid4()))
    headers["content-type"] = "application/connect+json"
    headers["x-idempotency-key"] = idem_key
    headers["x-idempotent-encryption-key"] = idem_header
    headers["x-blob-encryption-key"] = blob_hex
    t0 = time.monotonic()
    try:
        with httpx.Client(http2=True, timeout=STREAM_TIMEOUT) as client:
            with client.stream(
                "POST",
                API2 + IDEMPOTENT_PATH,
                headers=headers,
                content=minimal_payload(),
            ) as resp:
                detail = ""
                for line in resp.iter_lines():
                    if line:
                        detail = (
                            line.decode("utf-8", errors="replace")
                            if isinstance(line, bytes)
                            else str(line)
                        )[:200]
                        break
                elapsed = time.monotonic() - t0
                if "invalid" in detail.lower():
                    return f"{label:40} INVALID {elapsed:.1f}s {detail[:80]!r}"
                if "required" in detail.lower():
                    return f"{label:40} REQUIRED {elapsed:.1f}s"
                if detail and "serverChunk" in detail:
                    return f"{label:40} *** STREAM OK *** {elapsed:.1f}s"
                return f"{label:40} status={resp.status_code} {elapsed:.1f}s {detail[:80]!r}"
    except httpx.ReadTimeout:
        return f"{label:40} TIMEOUT {time.monotonic() - t0:.1f}s"
    except Exception as exc:
        return f"{label:40} ERROR {type(exc).__name__}: {exc}"


def aes_gcm_encrypt(key: bytes, plaintext: bytes, aad: bytes = b"") -> bytes:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    nonce = secrets.token_bytes(12)
    ct = AESGCM(key).encrypt(nonce, plaintext, aad)
    return nonce + ct


def main() -> None:
    try:
        from cryptography.hazmat.primitives.ciphers.aead import AESGCM  # noqa: F401
    except ImportError:
        print("pip install cryptography")
        sys.exit(1)

    token = load_token()
    raw = secrets.token_bytes(32)
    fmt = key_formats(raw)
    idem_key = str(uuid.uuid4())

    print("=== IDEMPOTENT KEY FORMAT PROBES ===\n")
    probes: list[tuple[str, str]] = [
        ("url raw 32B (baseline)", fmt["url"]),
        ("std b64 32B", fmt["std"].rstrip("=")),
        ("idem uuid plain", idem_key),
        ("idem uuid b64url", base64.urlsafe_b64encode(idem_key.encode()).decode().rstrip("=")),
        ("sha256(blob) hex", hashlib.sha256(raw).hexdigest()),
        ("sha256(blob) url", base64.urlsafe_b64encode(hashlib.sha256(raw).digest()).decode().rstrip("=")),
        ("sha256(idem+blob) url", base64.urlsafe_b64encode(hashlib.sha256((idem_key + fmt["hex"]).encode()).digest()).decode().rstrip("=")),
        ("hex+blob concat url", base64.urlsafe_b64encode((idem_key + fmt["hex"]).encode()).decode().rstrip("=")[:86]),
        ("64B random url", base64.urlsafe_b64encode(secrets.token_bytes(64)).decode().rstrip("=")),
        ("128B random url", base64.urlsafe_b64encode(secrets.token_bytes(128)).decode().rstrip("=")[:172]),
    ]

    # AES-GCM envelopes
    for name, pt, key, aad in [
        ("aesgcm(blob) key=raw", raw, raw, b""),
        ("aesgcm(idem) key=blob", idem_key.encode(), raw, b""),
        ("aesgcm(blob) key=idem", raw, idem_key.encode()[:32].ljust(32, b"\0"), b""),
        ("aesgcm(blob) aad=idem", raw, raw, idem_key.encode()),
    ]:
        try:
            env = aes_gcm_encrypt(key[:32] if len(key) >= 32 else key.ljust(32, b"\0")[:32], pt, aad)
            probes.append((name, base64.urlsafe_b64encode(env).decode().rstrip("=")))
        except Exception as exc:
            probes.append((name + " (fail)", fmt["url"]))
            print(f"skip {name}: {exc}")

    for name, key32 in {
        "hmac_idem_raw": __import__("hmac").new(idem_key.encode(), raw, hashlib.sha256).digest(),
        "sha256_std_body": hashlib.sha256(fmt["std"].encode()).digest(),
        "sha256_url_body": hashlib.sha256(fmt["url"].encode()).digest(),
    }.items():
        probes.append((name, base64.urlsafe_b64encode(key32).decode().rstrip("=")))

    for label, header_val in probes:
        print(probe(label, token, header_val, fmt["hex"], idem_key))

    print("\nDone. Chercher une ligne sans TIMEOUT et sans INVALID.")


if __name__ == "__main__":
    main()
