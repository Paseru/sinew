#!/usr/bin/env python3
"""Exhaustive Composer 2.5 config matrix — classifies OK / REJECT / STREAM / UNAUTH."""
from __future__ import annotations

import base64
import hashlib
import json
import pathlib
import re
import sys
import time
import uuid
from dataclasses import dataclass
from typing import Any

import httpx

AUTH_PATH = pathlib.Path.home() / "AppData/Local/Hyrak/sinew/data/cursor-composer-auth.json"
DEVICE_PATH = pathlib.Path.home() / "AppData/Local/Hyrak/sinew/data/cursor-composer-device.json"
DUMP_PATH = pathlib.Path.home() / "AppData/Local/Temp/sinew-composer-request-dump.json"
SSE_URL = "https://api2.cursor.sh/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE"
READ_BUDGET_S = 8.0
FIRST_BYTE_BUDGET_S = 6.0


@dataclass
class Result:
    label: str
    verdict: str  # OK | REJECT | STREAM | UNAUTH | ERROR | TIMEOUT
    status: int | str
    dt: float
    http: str
    detail: str
    bytes_read: int = 0


def checksum_xor(machine_id: str) -> str:
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
        b[idx] = (b[idx] ^ state) & 0xFF
        state = b[idx]
    enc = base64.urlsafe_b64encode(bytes(b)).decode().rstrip("=")
    return enc + machine_id


def checksum_rust_node(machine_id: str) -> str:
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
    enc = base64.urlsafe_b64encode(bytes(b)).decode().rstrip("=")
    return enc + machine_id


def connect_frame(body: dict, *, with_end: bool = True) -> bytes:
    payload = json.dumps(body).encode()
    frame = bytes([0]) + len(payload).to_bytes(4, "big") + payload
    if with_end:
        frame += bytes([0x02, 0, 0, 0, 0])
    return frame


def classify(content: bytes, status: int, dt: float, bytes_read: int) -> tuple[str, str]:
    text = content.decode("utf-8", "replace").lower()
    if "unauthenticated" in text or "unauthorized" in text:
        return "UNAUTH", _extract_detail(content)
    if "invalid" in text or "required" in text or '"error"' in text:
        return "REJECT", _extract_detail(content)
    if bytes_read > 80 and dt >= FIRST_BYTE_BUDGET_S * 0.9:
        return "STREAM", f"streaming ({bytes_read} bytes in {dt:.1f}s)"
    if status == 200 and bytes_read > 40:
        return "OK", f"{bytes_read} bytes"
    if status == 200 and bytes_read == 0 and dt < 1.0:
        return "REJECT", "(empty 200)"
    if status != 200:
        return "REJECT", _extract_detail(content) or f"HTTP {status}"
    return "OK", _extract_detail(content) or f"{bytes_read} bytes"


def _extract_detail(content: bytes) -> str:
    text = content.decode("utf-8", "replace")
    for pat in [
        r'"detail":"([^"]{0,120})"',
        r'"message":"([^"]{0,120})"',
        r'unauthenticated[^"]*',
    ]:
        m = re.search(pat, text, re.I)
        if m:
            return m.group(1) if m.lastindex else m.group(0)
    return text[:120].replace("\n", " ") if text.strip() else ""


def minimal_body(model: str, body_key: str, idem: str) -> dict:
    return {
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
                "conversationId": f"matrix-{idem[:8]}",
                "modelDetails": {
                    "modelName": model,
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


def run_case(
    client: httpx.Client,
    label: str,
    headers: dict[str, str],
    body: dict,
    *,
    http2: bool = True,
) -> Result:
    started = time.time()
    bytes_read = 0
    status: int | str = "?"
    http_ver = "?"
    content = b""
    try:
        if http2:
            with client.stream(
                "POST",
                SSE_URL,
                headers=headers,
                content=connect_frame(body),
            ) as response:
                status = response.status_code
                http_ver = str(response.http_version)
                for chunk in response.iter_bytes(4096):
                    bytes_read += len(chunk)
                    content += chunk
                    if bytes_read > 400 or time.time() - started > READ_BUDGET_S:
                        break
        else:
            response = httpx.post(
                SSE_URL,
                headers=headers,
                content=connect_frame(body),
                timeout=READ_BUDGET_S,
            )
            status = response.status_code
            http_ver = str(response.http_version)
            content = response.content[:800]
            bytes_read = len(content)
    except httpx.ReadTimeout:
        dt = time.time() - started
        if bytes_read > 0:
            verdict, detail = "STREAM", f"read timeout after {bytes_read} bytes"
        else:
            verdict, detail = "TIMEOUT", "no bytes before timeout (may still stream)"
        return Result(label, verdict, status, dt, http_ver, detail, bytes_read)
    except Exception as err:
        dt = time.time() - started
        return Result(label, "ERROR", type(err).__name__, dt, http_ver, str(err)[:120], bytes_read)

    dt = time.time() - started
    verdict, detail = classify(content, int(status) if isinstance(status, int) else 0, dt, bytes_read)
    return Result(label, verdict, status, dt, http_ver, detail, bytes_read)


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8", errors="backslashreplace")
    if not AUTH_PATH.exists():
        print(f"FAIL: missing auth at {AUTH_PATH}")
        return 1

    auth = json.loads(AUTH_PATH.read_text())
    token = auth["tokens"]["accessToken"]
    token_machine = hashlib.sha256(f"{token}machineId".encode()).hexdigest()
    sinew_machine = ""
    if DEVICE_PATH.exists():
        sinew_machine = json.loads(DEVICE_PATH.read_text()).get("machineId", "")

    client_key = hashlib.sha256(token.encode()).hexdigest()
    raw = bytes([0xAB] * 32)
    enc_std = base64.b64encode(raw).decode()
    enc_url = base64.urlsafe_b64encode(raw).decode().rstrip("=")
    enc_hex = raw.hex()

    print("=== PREREQ ===")
    email = httpx.post(
        "https://api2.cursor.sh/aiserver.v1.AuthService/GetEmail",
        headers={"authorization": f"Bearer {token}", "connect-protocol-version": "1"},
        json={},
        timeout=12,
    )
    print(f"GetEmail: {email.status_code} {email.text[:80]}")

    cases: list[tuple[str, dict[str, str], dict, bool]] = []

    def base_h(checksum: str, extra: dict[str, str], idem: str) -> dict[str, str]:
        return {
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
            "x-cursor-timezone": "Romance Standard Time",
            "x-client-key": client_key,
            "x-cursor-checksum": checksum,
            "x-ghost-mode": "false",
            "x-new-onboarding-completed": "true",
            "connect-accept-encoding": "gzip",
            "x-session-id": str(uuid.uuid4()),
            "x-request-id": str(uuid.uuid4()),
            "x-idempotency-key": idem,
            **extra,
        }

    # --- Header matrix (Sinew production: url idem + hex blob, body std b64) ---
    header_matrix = [
        ("none", {}),
        ("blob-hex only", {"x-blob-encryption-key": enc_hex}),
        ("idem-url only", {"x-idempotent-encryption-key": enc_url}),
        ("idem-std only", {"x-idempotent-encryption-key": enc_std}),
        ("idem-hex only", {"x-idempotent-encryption-key": enc_hex}),
        ("SINEW url+hex", {"x-idempotent-encryption-key": enc_url, "x-blob-encryption-key": enc_hex}),
        ("both std+hex", {"x-idempotent-encryption-key": enc_std, "x-blob-encryption-key": enc_hex}),
        ("both url+std", {"x-idempotent-encryption-key": enc_url, "x-blob-encryption-key": enc_std}),
    ]

    for hdr_label, extra in header_matrix:
        idem = str(uuid.uuid4())
        cs = checksum_rust_node(token_machine)
        body = minimal_body("composer-2.5", enc_std, idem)
        cases.append(
            (
                f"hdr[{hdr_label}] cs=rust",
                base_h(cs, extra, idem),
                body,
                True,
            )
        )

    # --- Checksum variants (with SINEW headers) ---
    sinew_extra = {"x-idempotent-encryption-key": enc_url, "x-blob-encryption-key": enc_hex}
    for cs_label, cs_fn, mid in [
        ("rust+tokenMachine", checksum_rust_node, token_machine),
        ("xor+tokenMachine", checksum_xor, token_machine),
        ("rust+sinewUuid", checksum_rust_node, sinew_machine or token_machine),
        ("xor+sinewUuid", checksum_xor, sinew_machine or token_machine),
        ("no-checksum", lambda _: "", token_machine),
    ]:
        idem = str(uuid.uuid4())
        h = base_h(cs_fn(mid), sinew_extra, idem)
        if cs_label == "no-checksum":
            h.pop("x-cursor-checksum", None)
        cases.append((f"cs[{cs_label}]", h, minimal_body("composer-2.5", enc_std, idem), True))

    # --- Body key format (header SINEW, idem key matches) ---
    body_matrix = [
        ("body-std idem-url", enc_std, enc_url),
        ("body-std idem-std", enc_std, enc_std),
        ("body-url idem-url", enc_url, enc_url),
        ("body-hex idem-hex", enc_hex, enc_hex),
    ]
    for blabel, body_key, idem_hdr in body_matrix:
        idem = str(uuid.uuid4())
        extra = {"x-idempotent-encryption-key": idem_hdr, "x-blob-encryption-key": enc_hex}
        cases.append(
            (
                f"body[{blabel}]",
                base_h(checksum_rust_node(token_machine), extra, idem),
                minimal_body("composer-2.5", body_key, idem),
                True,
            )
        )

    # --- Models ---
    for model in ["composer-2.5", "composer-2.5-fast"]:
        idem = str(uuid.uuid4())
        cases.append(
            (
                f"model[{model}]",
                base_h(checksum_rust_node(token_machine), sinew_extra, idem),
                minimal_body(model, enc_std, idem),
                True,
            )
        )

    # --- HTTP/1.1 ---
    idem = str(uuid.uuid4())
    cases.append(
        (
            "http1.1 SINEW",
            base_h(checksum_rust_node(token_machine), sinew_extra, idem),
            minimal_body("composer-2.5", enc_std, idem),
            False,
        )
    )

    # --- Full dump replay ---
    if DUMP_PATH.exists():
        dump = json.loads(DUMP_PATH.read_text())
        frame_json = dump["connect_frames"][0]["json"]
        idem = str(uuid.uuid4())
        body = json.loads(json.dumps(frame_json))
        req = body["clientChunk"]["streamUnifiedChatRequest"]
        req["blobEncryptionKey"] = enc_std
        req["speculativeSummarizationEncryptionKey"] = enc_std
        body["idempotencyKey"] = idem
        body["seqno"] = 0
        cases.append(
            (
                "full-dump-replay",
                base_h(checksum_rust_node(token_machine), sinew_extra, idem),
                body,
                True,
            )
        )

    print(f"\n=== MATRIX ({len(cases)} cases) ===\n")
    results: list[Result] = []
    with httpx.Client(
        http2=True,
        timeout=httpx.Timeout(READ_BUDGET_S + 2, connect=10.0),
    ) as client:
        for label, headers, body, use_h2 in cases:
            r = run_case(client if use_h2 else httpx.Client(http2=False), label, headers, body, http2=use_h2)
            results.append(r)
            mark = {"OK": "+", "STREAM": "~", "TIMEOUT": "~", "UNAUTH": "X", "REJECT": "-", "ERROR": "!"}.get(
                r.verdict, "?"
            )
            print(
                f"[{mark}] {r.label:28} {r.verdict:8} status={r.status} "
                f"dt={r.dt:4.1f}s http={r.http} bytes={r.bytes_read:4} | {r.detail[:90]}"
            )
            time.sleep(0.15)

    print("\n=== SUMMARY ===")
    by_verdict: dict[str, list[str]] = {}
    for r in results:
        by_verdict.setdefault(r.verdict, []).append(r.label)
    for verdict in ["OK", "STREAM", "TIMEOUT", "UNAUTH", "REJECT", "ERROR"]:
        items = by_verdict.get(verdict, [])
        if items:
            print(f"{verdict} ({len(items)}): {', '.join(items[:8])}{'...' if len(items)>8 else ''}")

    working = [r.label for r in results if r.verdict in ("OK", "STREAM", "TIMEOUT")]
    broken = [r.label for r in results if r.verdict in ("UNAUTH", "REJECT", "ERROR")]
    print(f"\nWORKING (stream likely): {len(working)}")
    print(f"BROKEN (immediate fail): {len(broken)}")
    if working:
        print("\nRecommended Sinew config:")
        print("  URL:", SSE_URL)
        print("  Headers: x-idempotent-encryption-key=URL_SAFE_B64(32B), x-blob-encryption-key=HEX(32B)")
        print("  Body keys: blobEncryptionKey=STANDARD_B64(32B)")
        print("  Checksum: rust/node algorithm + sha256(token+'machineId')")
        print("  HTTP: HTTP/2 required")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
