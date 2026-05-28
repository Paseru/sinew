#!/usr/bin/env python3
"""One-off: extract agent.v1 / agent.api5 hints from Cursor workbench bundle."""
import re
from pathlib import Path

WB = Path(r"C:\Users\julie\AppData\Local\Programs\cursor\resources\app\out\vs\workbench\workbench.desktop.main.js")
AL = Path(
    r"C:\Users\julie\AppData\Local\Programs\cursor\resources\app\extensions\cursor-always-local\dist\main.js"
)

def scan(path: Path, label: str) -> None:
    t = path.read_text(encoding="utf-8", errors="ignore")
    print(f"\n=== {label} ({path.name}, {len(t)//1_000_000}M chars) ===")
    for pat in [
        "agent.api5",
        "agentn.api5",
        "api2.cursor.sh",
        "AgentService",
        "agent.v1",
        "runAgentLoop",
        "runSSE",
        "RunSSE",
        "/Run",
        "GetUsableModels",
    ]:
        print(f"  {pat}: {len(re.findall(re.escape(pat), t))}")

    names = sorted(set(re.findall(r'typeName":"(agent\.v1\.[^"]+)"', t)))
    print(f"  agent.v1 types: {len(names)}")
    for n in names:
        if any(k in n for k in ("Service", "Run", "Agent", "Request", "Response")):
            print(f"    {n}")

    for m in re.finditer(r'agent\.api5[^"\']{0,80}', t):
        print(f"  ctx: {m.group(0)[:120]}")
        break

    for m in re.finditer(r'AgentService[^"\']{0,120}', t):
        s = t[max(0, m.start() - 80) : m.start() + 200]
        if "Run" in s or "run" in s:
            print(f"  AgentService ctx: ...{s.replace(chr(10),' ')[:260]}...")
            break


if WB.exists():
    scan(WB, "workbench")
if AL.exists():
    scan(AL, "always-local")
