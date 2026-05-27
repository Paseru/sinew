---
name: browser
description: "SOTA Chrome automation. Use for opening pages, inspecting DOM, clicking, typing, waiting, screenshots, or browser workflows. Prefer Sinew Chrome MCP selector tools before heuristic browser-agent actions."
---

# Browser Control — Sinew Chrome MCP

Use this skill whenever controlling Google Chrome, opening pages, clicking elements, typing into forms, inspecting DOM, waiting for UI, or running browser workflows.

## Prime Directive

Be faster and more reliable than generic browser automation:

1. Prefer **structured MCP tools** over natural-language browser-agent actions.
2. Prefer **CSS selector tools** over heuristic text targeting.
3. Prefer **turbo/direct actions** for normal UI automation.
4. Use human cursor/heurstic actions only when the user explicitly wants visible human-like movement or when selectors are unknown.

## Required Tool Order

For Chrome automation, use this order:

1. `open_browser` / `navigate` — open the target page.
2. `page_snapshot` or `query_selector` — inspect available elements.
3. `wait_for_selector` — wait for stable UI before acting.
4. `click_selector` / `type_selector` — direct, fast, reliable action.
5. `evaluate` — read page state or perform small DOM checks.
6. `screenshot` only when visual confirmation is needed.
7. `click` / `run_browser_agent` only as fallback when no selector is available.

## Tool Usage Rules

### Opening / Navigation

- If the task is only “open X” or “go to X”, use `open_browser` or `navigate` and stop.
- Do not click inside the page after a pure navigation request.

### Inspecting

Use `page_snapshot` first when you need discoverable selectors. It returns visible elements with selectors and bounding boxes.

Use `query_selector` when you already have a selector and need text, visibility, href, or bbox.

### Waiting

Use `wait_for_selector` before clicking/typing dynamic pages:

- `wait_for_selector("button[type='submit']")`
- `wait_for_selector("input[name='q']")`
- `wait_for_selector("#menu-button")`

### Clicking

Prefer:

- `click_selector("#menu-button")`
- `click_selector("a[href='/pricing']")`
- `click_selector("[data-testid='submit']")`

Avoid heuristic `click("menu button")` unless selectors are unavailable.

### Typing

Prefer:

- `type_selector("input[name='q']", "search query", submit=true)`
- `type_selector("textarea", "message")`

### Evaluating

Use `evaluate` for simple reads:

- `document.title`
- `location.href`
- `Array.from(document.querySelectorAll('a')).map(a => a.href).slice(0, 10)`

Keep evaluation small and return JSON-serializable values.

## Recovery

If tools fail:

1. Call `get_page_state` or `/api/status` style diagnostics through the MCP/bridge if available.
2. If bridge disconnected: ask user to reload the Sinew Chrome Bridge extension or click Reconnect in the popup.
3. If native host missing: run `sinew-chrome-bridge/register.ps1`.
4. If page is restricted (`chrome://`, extension pages), explain Chrome blocks content script automation.

## Human Cursor Mode

Use visible cursor only when the user asks to “show”, “demonstrate”, “human”, “record”, or when anti-bot behavior is suspected.

For speed, prefer hidden/direct selector actions.
