// ðŸ§¬ Sinew Chrome Bridge â€” Cursor & Overlay Script
// Injected into web pages to draw a SOTA biologically-realistic virtual cursor with spring physics.

(function () {
  const OVERLAY_ROOT_ID = "Sinew-agent-overlay-root";
  if (document.getElementById(OVERLAY_ROOT_ID)) {
    return; // Already injected
  }

  // Create isolated container
  const container = document.createElement("div");
  container.id = OVERLAY_ROOT_ID;
  container.style.position = "fixed";
  container.style.inset = "0";
  container.style.zIndex = "2147483647"; // Max index
  container.style.pointerEvents = "none";
  
  // Shadow Root for 100% styles isolation
  const shadow = container.attachShadow({ mode: "closed" });

  // Stylings for our cyber neon cursor, click shockwaves and target HUD
  const style = document.createElement("style");
  style.textContent = `
    .sinew-controlled-tab-indicator {
      position: fixed;
      left: 0;
      top: 0;
      width: 100vw;
      height: 4px;
      z-index: 2147483646;
      pointer-events: none;
      opacity: 0;
      transform: translateY(-4px);
      transition: opacity 180ms ease, transform 180ms ease;
      background: linear-gradient(90deg, #ff6b00, #ff0080, #66f7ff, #ff6b00);
      background-size: 260% 100%;
      box-shadow: 0 0 12px rgba(255, 0, 128, 0.55), 0 0 24px rgba(102, 247, 255, 0.35);
      animation: sinew-controlled-tab-flow 2.2s linear infinite;
    }

    .sinew-controlled-tab-indicator.active {
      opacity: 1;
      transform: translateY(0);
    }

    @keyframes sinew-controlled-tab-flow {
      0% { background-position: 0% 50%; }
      100% { background-position: 260% 50%; }
    }

    .cursor-overlay {
      position: absolute;
      top: 0;
      left: 0;
      width: 28px;
      height: 28px;
      will-change: transform, opacity, filter;
      opacity: 0;
      transition: opacity 0.3s ease;
    }
    
    .cursor-pointer {
      width: 28px;
      height: 28px;
      transform-origin: 4px 3px; /* Align with SVG tip */
      will-change: transform;
      filter: drop-shadow(0 0 5px #ff6b00) drop-shadow(0 0 12px #ff0080);
    }

    .cursor-label {
      position: absolute;
      left: 32px;
      top: 0px;
      background: rgba(10, 10, 15, 0.85);
      border: 1px solid rgba(255, 107, 0, 0.5);
      backdrop-filter: blur(8px);
      color: #ff6b00;
      font-family: 'Courier New', Courier, monospace;
      font-size: 10px;
      font-weight: bold;
      padding: 3px 8px;
      border-radius: 4px;
      white-space: nowrap;
      text-shadow: 0 0 5px rgba(255, 107, 0, 0.8);
      box-shadow: 0 4px 15px rgba(0, 0, 0, 0.5);
      transform: none !important; /* Keep it stable */
    }

    .click-shockwave {
      position: absolute;
      width: 12px;
      height: 12px;
      margin-left: -6px;
      margin-top: -6px;
      border: 2px solid #ff0080;
      border-radius: 50%;
      background: radial-gradient(circle, rgba(255, 107, 0, 0.2) 0%, rgba(255, 0, 128, 0) 70%);
      box-shadow: 0 0 10px #ff6b00, inset 0 0 10px #ff0080;
      opacity: 1;
      transform: scale(0.1);
      animation: shockwave-implode-explode 0.6s cubic-bezier(0.1, 0.8, 0.1, 1) forwards;
      pointer-events: none;
    }
    
    @keyframes shockwave-implode-explode {
      0% {
        transform: scale(0.1);
        opacity: 0.8;
        border-color: #ff6b00;
      }
      20% {
        transform: scale(0.8);
        opacity: 1;
        border-color: #ff0080;
        box-shadow: 0 0 15px #ff0080, inset 0 0 15px #ff6b00;
      }
      100% {
        transform: scale(5);
        opacity: 0;
        border-color: rgba(255, 0, 128, 0);
        box-shadow: 0 0 30px rgba(255, 0, 128, 0), inset 0 0 30px rgba(255, 107, 0, 0);
      }
    }

    /* Cyber targeting target HUD box SOTA */
    .cyber-hud-target {
      position: absolute;
      border: 1px dashed rgba(255, 107, 0, 0.3);
      border-radius: 4px;
      pointer-events: none;
      opacity: 0;
      transform: scale(1.08);
      transition: opacity 0.22s ease, transform 0.22s ease, left 0.18s cubic-bezier(0.25, 0.8, 0.25, 1), top 0.18s cubic-bezier(0.25, 0.8, 0.25, 1), width 0.18s cubic-bezier(0.25, 0.8, 0.25, 1), height 0.18s cubic-bezier(0.25, 0.8, 0.25, 1);
      box-shadow: 0 0 8px rgba(255, 0, 128, 0.25), inset 0 0 8px rgba(255, 107, 0, 0.15);
      z-index: 2147483645;
    }
    
    .cyber-hud-target::before, .cyber-hud-target::after {
      content: "";
      position: absolute;
      width: 8px;
      height: 8px;
      border-color: #ff0080;
      border-style: solid;
      will-change: transform;
    }
    
    .cyber-hud-target::before {
      top: -2px;
      left: -2px;
      border-width: 2px 0 0 2px;
    }
    
    .cyber-hud-target::after {
      bottom: -2px;
      right: -2px;
      border-width: 0 2px 2px 0;
    }
    
    .cyber-hud-target.active {
      opacity: 1;
      transform: scale(1);
      border-color: rgba(255, 107, 0, 0.85);
      box-shadow: 0 0 12px rgba(255, 0, 128, 0.5), inset 0 0 12px rgba(255, 107, 0, 0.3);
      animation: hud-pulsate 1.5s infinite alternate;
    }
    
    @keyframes hud-pulsate {
      0% {
        box-shadow: 0 0 8px rgba(255, 0, 128, 0.4), inset 0 0 8px rgba(255, 107, 0, 0.2);
      }
      100% {
        box-shadow: 0 0 16px rgba(255, 0, 128, 0.75), inset 0 0 16px rgba(255, 107, 0, 0.5);
      }
    }

    /* Cyber-Neon Macro Recorder Widget */
    .cyber-macro-widget {
      position: fixed;
      top: 80px;
      right: 20px;
      background: rgba(10, 10, 15, 0.95);
      border: 1px solid rgba(255, 107, 0, 0.4);
      border-radius: 12px;
      font-family: 'Share Tech Mono', monospace;
      padding: 12px;
      pointer-events: auto; /* MUST receive clicks */
      z-index: 2147483647;
      backdrop-filter: blur(12px);
      box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7), 0 0 15px rgba(255, 107, 0, 0.15);
      color: #f0f0f5;
      width: 280px;
      transition: width 0.3s cubic-bezier(0.25, 0.8, 0.25, 1), height 0.3s cubic-bezier(0.25, 0.8, 0.25, 1), border-radius 0.3s, background 0.3s;
      display: flex;
      flex-direction: column;
      gap: 10px;
    }
    
    .cyber-macro-widget.collapsed {
      width: 48px;
      height: 48px;
      border-radius: 50%;
      padding: 0;
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: move;
      border-color: rgba(255, 0, 128, 0.5);
      background: radial-gradient(circle, rgba(255, 0, 128, 0.15) 0%, rgba(10, 10, 15, 0.9) 70%);
      box-shadow: 0 0 12px rgba(255, 0, 128, 0.4);
    }
    
    .cyber-macro-widget.collapsed:hover {
      box-shadow: 0 0 20px #ff0080;
      border-color: #ff0080;
      transform: scale(1.05);
    }
    
    .widget-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      border-bottom: 1px solid rgba(255, 255, 255, 0.08);
      padding-bottom: 8px;
      cursor: move;
    }
    
    .widget-title {
      font-size: 11px;
      font-weight: bold;
      color: #ff6b00;
      letter-spacing: 1px;
      text-shadow: 0 0 4px rgba(255, 107, 0, 0.6);
      display: flex;
      align-items: center;
      gap: 6px;
    }
    
    .widget-close {
      cursor: pointer;
      color: #8fa0b0;
      font-size: 12px;
      transition: color 0.2s;
    }
    
    .widget-close:hover {
      color: #ff0080;
    }
    
    .widget-body {
      display: flex;
      flex-direction: column;
      gap: 10px;
    }
    
    .rec-status {
      display: flex;
      align-items: center;
      justify-content: space-between;
      font-size: 11px;
      color: #8fa0b0;
    }
    
    .rec-indicator {
      display: flex;
      align-items: center;
      gap: 6px;
    }
    
    .rec-dot {
      width: 8px;
      height: 8px;
      background: #8fa0b0;
      border-radius: 50%;
    }
    
    .rec-dot.active {
      background: #ff0080;
      box-shadow: 0 0 8px #ff0080;
      animation: rec-pulse 1s infinite alternate;
    }
    
    @keyframes rec-pulse {
      0% { opacity: 0.4; }
      100% { opacity: 1; box-shadow: 0 0 12px #ff0080; }
    }
    
    .widget-actions-list {
      background: rgba(0, 0, 0, 0.5);
      border: 1px solid rgba(255, 255, 255, 0.05);
      border-radius: 6px;
      font-size: 10px;
      padding: 8px;
      max-height: 120px;
      overflow-y: auto;
      color: #a0b5c5;
      display: flex;
      flex-direction: column;
      gap: 4px;
    }
    
    .action-entry {
      display: flex;
      justify-content: space-between;
      border-bottom: 1px solid rgba(255, 255, 255, 0.02);
      padding-bottom: 2px;
      gap: 10px;
    }
    
    .action-type {
      color: #ff6b00;
      font-weight: bold;
    }
    
    .action-details {
      color: #f0f0f5;
      text-overflow: ellipsis;
      overflow: hidden;
      white-space: nowrap;
      max-width: 170px;
      text-align: right;
    }
    
    .widget-buttons {
      display: flex;
      gap: 8px;
    }
    
    .widget-btn {
      flex: 1;
      background: rgba(255, 107, 0, 0.1);
      border: 1px solid rgba(255, 107, 0, 0.4);
      color: #ff6b00;
      padding: 8px 6px;
      border-radius: 6px;
      font-family: 'Share Tech Mono', monospace;
      font-size: 10px;
      cursor: pointer;
      text-align: center;
      transition: all 0.2s;
      outline: none;
    }
    
    .widget-btn:hover {
      background: #ff6b00;
      color: #000;
      box-shadow: 0 0 10px rgba(255, 107, 0, 0.5);
      border-color: transparent;
    }
    
    .widget-btn.btn-rec {
      border-color: rgba(255, 0, 128, 0.5);
      color: #ff0080;
      background: rgba(255, 0, 128, 0.1);
    }
    
    .widget-btn.btn-rec:hover {
      background: #ff0080;
      color: #000;
      box-shadow: 0 0 10px rgba(255, 0, 128, 0.5);
      border-color: transparent;
    }
    
    .widget-btn:disabled {
      opacity: 0.4;
      pointer-events: none;
    }
    
    .widget-input {
      background: rgba(0, 0, 0, 0.3);
      border: 1px solid rgba(255, 255, 255, 0.1);
      border-radius: 4px;
      color: #fff;
      font-family: 'Share Tech Mono', monospace;
      font-size: 11px;
      padding: 6px 8px;
      width: 100%;
      outline: none;
      box-sizing: border-box;
    }
    
    .widget-input:focus {
      border-color: #ff6b00;
    }
  `;
  shadow.appendChild(style);

  // Shockwave container
  const waveContainer = document.createElement("div");
  waveContainer.style.position = "absolute";
  waveContainer.style.inset = "0";
  waveContainer.style.pointerEvents = "none";
  shadow.appendChild(waveContainer);

  // HUD target highlight SOTA
  const targetHud = document.createElement("div");
  targetHud.className = "cyber-hud-target";
  shadow.appendChild(targetHud);

  const tabIndicator = document.createElement("div");
  tabIndicator.className = "sinew-controlled-tab-indicator";
  shadow.appendChild(tabIndicator);

  // Holographic Cyber cursor element
  const overlay = document.createElement("div");
  overlay.className = "cursor-overlay";
  
  const pointer = document.createElement("div");
  pointer.className = "cursor-pointer";
  pointer.innerHTML = `
    <svg width="28" height="28" viewBox="0 0 28 28" fill="none" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <linearGradient id="cyber-grad" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="#ff6b00" />
          <stop offset="100%" stop-color="#ff0080" />
        </linearGradient>
        <filter id="cyber-glow">
          <feGaussianBlur stdDeviation="1.5" result="coloredBlur"/>
          <feMerge>
            <feMergeNode in="coloredBlur"/>
            <feMergeNode in="SourceGraphic"/>
          </feMerge>
        </filter>
      </defs>
      <!-- Premium Cyber-Neon triangle cursor design pointing to (4,3) -->
      <path d="M4.5 3L23.5 11.5L14 14.5L11.5 24L4.5 3Z" fill="url(#cyber-grad)" filter="url(#cyber-glow)" stroke="white" stroke-width="1.2" stroke-linejoin="round" />
    </svg>
  `;
  overlay.appendChild(pointer);

  const label = document.createElement("div");
  label.className = "cursor-label";
  label.textContent = "Sinew ACTIVE";
  // overlay.appendChild(label); // Disabled to match clean Codex style (no flashing text next to the cursor)

  shadow.appendChild(overlay);
  document.documentElement.appendChild(container);

  // Masse-Ressort-Amortisseur Spring system
  class Spring {
    constructor(val = 0, damping = 0.85, response = 0.22) {
      this.value = val;
      this.target = val;
      this.velocity = 0;
      this.damping = damping;
      this.response = response;
    }

    update(dt) {
      const stiffness = Math.pow((2 * Math.PI) / this.response, 2);
      const dampingCoefficient = 2 * ((2 * Math.PI) / this.response) * this.damping;

      const force = (this.target - this.value) * stiffness - this.velocity * dampingCoefficient;
      this.velocity += force * dt;
      this.value += this.velocity * dt;
    }
  }

  // Springs for coordinates, stretching/scooting, and blur
  const xSpring = new Spring(window.innerWidth / 2);
  const ySpring = new Spring(window.innerHeight / 2);
  const stretchSpring = new Spring(1, 0.85, 0.15); // Velocity-based length stretch
  const blurSpring = new Spring(0, 0.9, 0.12);     // Motion blur

  let activeState = {
    x: window.innerWidth / 2,
    y: window.innerHeight / 2,
    visible: false,
    moveSequence: 0,
    sessionId: null,
    turnId: null
  };

  let animFrameId = null;
  let lastTime = performance.now();
  let hasArrived = false;
  let lastTargetEl = null;

  // DOM Attention Layer (HUD highlighted elements)
  function updateTargetHud(x, y, visible) {
    // Disabled to match clean Codex-like visual style (no flashing pink boxes)
    targetHud.classList.remove("active");
    lastTargetEl = null;
    return;
  }

  function loop(now) {
    const dt = Math.min((now - lastTime) / 1000, 0.1); // Limit dt to 100ms to prevent instability
    lastTime = now;

    // Update spring coordinates
    xSpring.target = activeState.x;
    ySpring.target = activeState.y;

    xSpring.update(dt);
    ySpring.update(dt);

    // Calculate current speed/velocity to apply organic stretch and motion blur
    const speed = Math.sqrt(xSpring.velocity * xSpring.velocity + ySpring.velocity * ySpring.velocity);
    
    stretchSpring.target = 1 + Math.min(speed / 1200, 0.45); // Limit maximum stretch
    blurSpring.target = Math.min(speed / 300, 4);            // Blur according to speed
    
    stretchSpring.update(dt);
    blurSpring.update(dt);

    // Dynamic rotation according to movement angle
    let angle = -45; // Idle angle (default cursor orientation)
    if (speed > 10) {
      // Dynamic alignment: SVG points to -135deg by default, add 135deg to align to motion angle
      angle = Math.atan2(ySpring.velocity, xSpring.velocity) * (180 / Math.PI) + 135;
    }

    // Apply calculated matrices
    const px = Math.round(xSpring.value);
    const py = Math.round(ySpring.value);
    const scale = stretchSpring.value;
    const blur = blurSpring.value;

    // Move only container (overlay) to position px, py - label stays 100% horizontal
    overlay.style.transform = `translate3d(${px}px, ${py}px, 0)`;
    overlay.style.filter = blur > 0.2 ? `blur(${blur}px)` : "none";

    // Apply rotation and stretch/scale ONLY to the pointer element
    pointer.style.transform = `rotate(${angle}deg) scale(${scale}, ${1 / scale})`;

    // Show or hide overlay
    overlay.style.opacity = activeState.visible ? "1" : "0";

    // Update targeting HUD live
    updateTargetHud(px, py, activeState.visible);

    // Check arrival tolerance (similar to Sinew's Pn tolerance check)
    const distance = Math.sqrt(Math.pow(px - activeState.x, 2) + Math.pow(py - activeState.y, 2));
    
    if (activeState.visible && distance < 1.5 && Math.abs(xSpring.velocity) < 8 && Math.abs(ySpring.velocity) < 8) {
      if (!hasArrived && activeState.moveSequence > 0 && activeState.sessionId) {
        hasArrived = true;
        
        // Notify background that the cursor has arrived!
        chrome.runtime.sendMessage({
          type: "AGENT_CURSOR_ARRIVED",
          moveSequence: activeState.moveSequence,
          sessionId: activeState.sessionId,
          turnId: activeState.turnId
        }).catch(() => {});
      }
    }

    animFrameId = requestAnimationFrame(loop);
  }

  // Set new target coordinates and animate
  function updateCursorState(state) {
    activeState = { ...activeState, ...state };
    hasArrived = false;
    
    // Resume animation loop if not running
    if (!animFrameId) {
      lastTime = performance.now();
      animFrameId = requestAnimationFrame(loop);
    }
  }

  // Click shockwave trigger
  function triggerClickShockwave(x, y) {
    const shockwave = document.createElement("div");
    shockwave.className = "click-shockwave";
    shockwave.style.left = `${x}px`;
    shockwave.style.top = `${y}px`;
    
    waveContainer.appendChild(shockwave);
    
    setTimeout(() => {
      shockwave.remove();
    }, 600);
  }

  // ==========================================================
  // Controlled tab visual indicator (Codex-style underline)
  // ==========================================================
  let controlledIndicatorTimer = null;

  function updateControlledTabIndicator(status) {
    if (status === "detached" || status === "idle" || status === false) {
      tabIndicator.classList.remove("active");
      return;
    }
    tabIndicator.classList.add("active");
  }

  function markControlledActivity() {
    updateControlledTabIndicator("active");
    if (controlledIndicatorTimer) clearTimeout(controlledIndicatorTimer);
    controlledIndicatorTimer = setTimeout(() => {
      updateControlledTabIndicator("idle");
    }, 3500);
  }

  function handleActivity() {
    markControlledActivity();
  }

  // Listening to messages from background worker
  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === "AGENT_CURSOR_STATE") {
      handleActivity();
      updateCursorState({
        x: message.state.x,
        y: message.state.y,
        visible: message.state.visible,
        moveSequence: message.state.moveSequence,
        sessionId: message.state.sessionId,
        turnId: message.state.turnId
      });
      sendResponse({ ok: true });
    }
    else if (message.type === "AGENT_CLICK_EVENT") {
      handleActivity();
      if (message.event.type === "mousePressed") {
        triggerClickShockwave(message.event.x, message.event.y);
      }
      sendResponse({ ok: true });
    }
    else if (message.type === "AGENT_STATUS_CHANGE") {
      updateControlledTabIndicator(message.status);
      sendResponse({ ok: true });
    }
    else if (message.type === "AGENT_DOM_CLICK") {
      handleActivity();
      const x = Number(message.x);
      const y = Number(message.y);
      const initialEl = document.elementFromPoint(x, y);
      const el = initialEl && initialEl.closest
        ? (initialEl.closest('button, a, input, textarea, select, [role="button"], [onclick], summary, label') || initialEl)
        : initialEl;
      if (!el) {
        sendResponse({ ok: false, error: "No element at target point" });
        return true;
      }
      const opts = { bubbles: true, cancelable: true, view: window, clientX: x, clientY: y, button: 0, buttons: 1, pointerId: 1, pointerType: "mouse", isPrimary: true };
      el.dispatchEvent(new PointerEvent("pointerover", opts));
      el.dispatchEvent(new MouseEvent("mouseover", opts));
      el.dispatchEvent(new PointerEvent("pointermove", opts));
      el.dispatchEvent(new MouseEvent("mousemove", opts));
      el.dispatchEvent(new PointerEvent("pointerdown", opts));
      el.dispatchEvent(new MouseEvent("mousedown", opts));
      el.focus?.({ preventScroll: true });
      el.dispatchEvent(new PointerEvent("pointerup", { ...opts, buttons: 0 }));
      el.dispatchEvent(new MouseEvent("mouseup", { ...opts, buttons: 0 }));
      el.dispatchEvent(new MouseEvent("click", { ...opts, buttons: 0 }));
      if (typeof el.click === "function") el.click();
      sendResponse({ ok: true, tagName: el.tagName, id: el.id || "", className: typeof el.className === "string" ? el.className : "" });
      return true;
    }
    else if (message.type === "AGENT_DOM_SCROLL") {
      handleActivity();
      const amount = Number(message.scrollY) || Math.round(window.innerHeight * 0.6);
      window.scrollBy({ top: amount, left: 0, behavior: "smooth" });
      sendResponse({ ok: true, scrollY: amount });
      return true;
    }
    else if (message.type === "CONTENT_PING") {
      sendResponse({ ok: true });
    }
    else if (message.type === "RUN_SILENT_TASK") {
      const task = message.task;
      const taskText = task.toLowerCase();

      let action = "click";
      if (taskText.includes("type") || taskText.includes("tape") || taskText.includes("saisir") || taskText.includes("écrire") || taskText.includes("ecris") || taskText.includes("saisis")) {
        action = "type";
      } else if (taskText.includes("scroll") || taskText.includes("défiler") || taskText.includes("descendre") || taskText.includes("monter")) {
        action = "scroll";
      }

      if (action === "scroll") {
        const direction = (taskText.includes("up") || taskText.includes("monter") || taskText.includes("haut")) ? -1 : 1;
        const amount = Math.round(window.innerHeight * 0.6 * direction);
        sendResponse({ success: true, action: "scroll", scrollY: amount, message: "Cible scroll détectée." });
        return true;
      }

      const elements = Array.from(document.querySelectorAll('button, a, input, select, textarea, [role="button"], [onclick], div, span, svg, li, summary'));
      const cleanTask = taskText
        .replace(/\b(cliquez|clique|cliquer|click|ouvrir|ouvre|open|press|selectionne|sélectionne|va sur|aller|type|tape|saisir|écrire|ecris|saisis|dans|sur|le|la|les|un|une|et|du|de|des|site|web|page|url|navigate|navigue|carte|bouton)\b/g, " ")
        .trim();
      const queryWordsRaw = cleanTask.split(/\s+/).filter(w => w.length >= 1);
      const semanticWords = [];
      if (queryWordsRaw.some(w => w === "hamburger" || w === "burger" || w === "menu")) {
        semanticWords.push("menu", "hamburger", "burger", "nav", "toggle");
      }
      if (queryWordsRaw.some(w => w === "bouton" || w === "button")) {
        semanticWords.push("btn", "button", "bouton");
      }
      if (queryWordsRaw.some(w => w === "recherche" || w === "chercher" || w === "search")) {
        semanticWords.push("search", "query", "q", "recherche", "find");
      }
      const queryWords = Array.from(new Set([...queryWordsRaw, ...semanticWords]));

      let bestEl = null;
      let bestScore = -1;

      elements.forEach(el => {
        const rect = el.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) return;
        if (rect.width * rect.height > window.innerWidth * window.innerHeight * 0.4) return;

        const style = window.getComputedStyle(el);
        if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0") return;

        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;
        const topEl = document.elementFromPoint(centerX, centerY);
        if (topEl && topEl !== el && !el.contains(topEl) && !topEl.contains(el)) return;

        const text = (el.innerText || el.textContent || "").toLowerCase().trim();
        const placeholder = (el.getAttribute("placeholder") || "").toLowerCase();
        const ariaLabel = (el.getAttribute("aria-label") || "").toLowerCase();
        const title = (el.getAttribute("title") || "").toLowerCase();
        const id = (el.id || "").toLowerCase();
        const className = (typeof el.className === "string" ? el.className : "").toLowerCase();
        const value = (el.value || "").toLowerCase();
        const name = (el.getAttribute("name") || "").toLowerCase();
        const role = (el.getAttribute("role") || "").toLowerCase();
        const href = (el.getAttribute("href") || "").toLowerCase();

        let score = 0;
        queryWords.forEach(word => {
          if (text.includes(word)) score += 55;
          if (placeholder.includes(word)) score += 60;
          if (ariaLabel.includes(word)) score += 75;
          if (title.includes(word)) score += 55;
          if (id.includes(word)) score += 55;
          if (name.includes(word)) score += 40;
          if (value.includes(word)) score += 30;
          if (className.includes(word)) score += 25;
          if (href.includes(word)) score += 35;
        });

        if (action === "type" && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) score += 40;
        if (action === "click" && (el.tagName === "BUTTON" || el.tagName === "A" || role === "button" || style.cursor === "pointer")) score += 25;
        if ((taskText.includes("hamburger") || taskText.includes("menu")) && (id.includes("menu") || ariaLabel.includes("menu") || className.includes("menu"))) score += 120;
        if (taskText.includes("trinity") && (text.includes("trinity") || id.includes("trinity") || className.includes("trinity") || href.includes("trinity"))) score += 120;

        if (score > bestScore && score > 0) {
          bestScore = score;
          bestEl = el;
        }
      });

      if (!bestEl) {
        sendResponse({ success: false, message: "Aucun élément interactif pertinent trouvé pour cette tâche." });
        return true;
      }

      bestEl.scrollIntoView({ block: "center", inline: "center", behavior: "smooth" });
      setTimeout(() => {
        const rect = bestEl.getBoundingClientRect();
        const x = Math.round(rect.left + rect.width / 2);
        const y = Math.round(rect.top + rect.height / 2);
        sendResponse({
          success: true,
          action,
          target: {
            x,
            y,
            rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
            element: { tagName: bestEl.tagName, id: bestEl.id, className: typeof bestEl.className === 'string' ? bestEl.className : "" },
            score: bestScore
          },
          message: `Cible détectée à (${x}, ${y}) pour ${bestEl.tagName}.`
        });
      }, 450);

      return true;
    }
  });

  // Clean up controlled indicator when unloading
  window.addEventListener("unload", () => {
    updateControlledTabIndicator("detached");
  });

  // ==========================================================
  // Cyber Macro Recorder Engine & UI Injection
  // ==========================================================
  const macroWidget = document.createElement("div");
  macroWidget.className = "cyber-macro-widget collapsed";
  // shadow.appendChild(macroWidget); // Disabled to avoid floating red dot on the right, matching clean Codex style

  let isRecording = false;
  let recordedSteps = [];
  let recordingStartTime = 0;

  function renderCollapsed() {
    macroWidget.className = "cyber-macro-widget collapsed";
    macroWidget.innerHTML = `
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
        <circle cx="12" cy="12" r="8" stroke="${isRecording ? '#ff0080' : '#ff6b00'}" stroke-width="2" />
        <circle cx="12" cy="12" r="4" fill="${isRecording ? '#ff0080' : '#ff6b00'}" />
      </svg>
    `;
  }

  function renderExpanded() {
    macroWidget.className = "cyber-macro-widget";
    const dateStamp = new Date().toISOString().slice(0, 10).replace(/-/g, '');
    const timeStamp = new Date().toTimeString().slice(0, 5).replace(':', '');
    
    macroWidget.innerHTML = `
      <div class="widget-header">
        <span class="widget-title">
          <span class="rec-dot ${isRecording ? 'active' : ''}"></span>
          ðŸ§¬ MACRO RECORDER SOTA
        </span>
        <span class="widget-close" id="macro-collapse-btn">âœ–</span>
      </div>
      <div class="widget-body">
        <div class="rec-status">
          <span>STATUT : ${isRecording ? '<span style="color:#ff0080; font-weight:bold;">ENREGISTREMENT</span>' : 'VEILLE'}</span>
          <span>${recordedSteps.length} ACTIONS</span>
        </div>
        
        <input type="text" class="widget-input" id="macro-name-input" placeholder="nom_playbook" value="playbook_${dateStamp}_${timeStamp}" />
        
        <div class="widget-actions-list" id="macro-actions-list">
          ${recordedSteps.length === 0 
            ? '<div style="text-align:center;color:#8fa0b0;margin-top:25px;">Aucune action enregistrÃ©e</div>' 
            : recordedSteps.map((step, idx) => {
                const details = step.type === 'click' 
                  ? (step.text ? `"${step.text}"` : step.selector.split(' > ').pop())
                  : (step.value ? `"${step.value}"` : 'vide');
                return `
                  <div class="action-entry">
                    <span>${String(idx+1).padStart(2, '0')}. <span class="action-type">${step.type.toUpperCase()}</span></span>
                    <span class="action-details">${details}</span>
                  </div>
                `;
              }).join('')
          }
        </div>
        
        <div class="widget-buttons">
          <button class="widget-btn ${isRecording ? 'btn-stop' : 'btn-rec'}" id="macro-toggle-btn">
            ${isRecording ? 'ARRÃŠTER' : 'REC'}
          </button>
          <button class="widget-btn" id="macro-clear-btn" ${recordedSteps.length === 0 || isRecording ? 'disabled' : ''}>
            CLEAR
          </button>
          <button class="widget-btn" id="macro-save-btn" ${recordedSteps.length === 0 || isRecording ? 'disabled' : ''}>
            SAUVER
          </button>
        </div>
      </div>
    `;

    // Wire up event handlers inside Shadow DOM
    macroWidget.querySelector("#macro-collapse-btn").onclick = (e) => {
      e.stopPropagation();
      renderCollapsed();
    };

    macroWidget.querySelector("#macro-toggle-btn").onclick = (e) => {
      e.stopPropagation();
      if (isRecording) {
        stopRecordingMacro();
      } else {
        startRecordingMacro();
      }
    };

    macroWidget.querySelector("#macro-clear-btn").onclick = (e) => {
      e.stopPropagation();
      recordedSteps = [];
      renderExpanded();
    };

    macroWidget.querySelector("#macro-save-btn").onclick = (e) => {
      e.stopPropagation();
      saveMacroPlaybook();
    };
  }

  function startRecordingMacro() {
    isRecording = true;
    recordedSteps = [];
    recordingStartTime = performance.now();
    renderExpanded();
    triggerClickShockwave(window.innerWidth / 2, window.innerHeight / 2);
  }

  function stopRecordingMacro() {
    isRecording = false;
    renderExpanded();
  }

  function saveMacroPlaybook() {
    const inputVal = macroWidget.querySelector("#macro-name-input").value.trim();
    const playbookName = (inputVal || "unnamed_playbook") + ".json";
    
    const playbook = {
      name: playbookName,
      url: window.location.href,
      title: document.title,
      timestamp: Date.now(),
      steps: recordedSteps
    };
    
    chrome.runtime.sendMessage({
      type: "AGENT_SAVE_MACRO",
      macro: playbook
    }, (res) => {
      const btn = macroWidget.querySelector("#macro-save-btn");
      if (btn) {
        const originalText = btn.textContent;
        btn.textContent = "SAUVÃ‰ !";
        btn.style.borderColor = "#00ff66";
        btn.style.color = "#00ff66";
        setTimeout(() => {
          btn.textContent = originalText;
          btn.style.borderColor = "";
          btn.style.color = "";
          recordedSteps = [];
          renderExpanded();
        }, 1500);
      }
    });
  }

  function getCssSelector(el) {
    if (!(el instanceof Element)) return "";
    const path = [];
    while (el && el.nodeType === Node.ELEMENT_NODE) {
      let selector = el.nodeName.toLowerCase();
      if (el.id) {
        selector += '#' + el.id;
        path.unshift(selector);
        break;
      } else {
        let sibling = el.previousElementSibling;
        let nth = 1;
        while (sibling) {
          if (sibling.nodeName.toLowerCase() === selector) {
            nth++;
          }
          sibling = sibling.previousElementSibling;
        }
        if (nth > 1) {
          selector += `:nth-of-type(${nth})`;
        }
      }
      path.unshift(selector);
      el = el.parentElement;
    }
    return path.join(' > ');
  }

  // Document passive listeners
  document.addEventListener("click", (e) => {
    if (!isRecording) return;
    if (container.contains(e.target)) return; // Ignore widget clicks
    
    const elapsed = Math.round(performance.now() - recordingStartTime);
    const selector = getCssSelector(e.target);
    
    recordedSteps.push({
      type: "click",
      selector: selector,
      x: e.clientX,
      y: e.clientY,
      text: e.target.textContent ? e.target.textContent.trim().slice(0, 30) : "",
      timestamp: elapsed
    });
    
    if (!macroWidget.classList.contains("collapsed")) {
      renderExpanded();
    }
  }, true);

  document.addEventListener("input", (e) => {
    if (!isRecording) return;
    if (container.contains(e.target)) return;
    
    const elapsed = Math.round(performance.now() - recordingStartTime);
    const selector = getCssSelector(e.target);
    
    const lastStep = recordedSteps[recordedSteps.length - 1];
    if (lastStep && lastStep.type === "input" && lastStep.selector === selector) {
      lastStep.value = e.target.value;
      lastStep.timestamp = elapsed;
    } else {
      recordedSteps.push({
        type: "input",
        selector: selector,
        value: e.target.value,
        timestamp: elapsed
      });
    }
    
    if (!macroWidget.classList.contains("collapsed")) {
      renderExpanded();
    }
  }, true);

  let isDragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let widgetStartX = 0;
  let widgetStartY = 0;
  let hasMoved = false;

  function initDraggable() {
    // Setup mousedown listener on the widget for dragging
    macroWidget.addEventListener("mousedown", (e) => {
      // Don't drag if clicking buttons, inputs or collapse button
      if (e.target.closest("button") || e.target.closest("input") || e.target.closest("#macro-collapse-btn") || e.target.closest(".widget-close")) {
        return;
      }
      
      // If expanded, only drag via the header
      if (!macroWidget.classList.contains("collapsed") && !e.target.closest(".widget-header")) {
        return;
      }

      isDragging = true;
      hasMoved = false;
      const rect = macroWidget.getBoundingClientRect();

      // Convert layout to absolute coordinates
      macroWidget.style.top = `${rect.top}px`;
      macroWidget.style.left = `${rect.left}px`;
      macroWidget.style.bottom = "auto";
      macroWidget.style.right = "auto";

      widgetStartX = rect.left;
      widgetStartY = rect.top;
      dragStartX = e.clientX;
      dragStartY = e.clientY;

      e.preventDefault();

      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
    });

    function onMouseMove(e) {
      if (!isDragging) return;
      const dx = e.clientX - dragStartX;
      const dy = e.clientY - dragStartY;

      if (Math.abs(dx) > 4 || Math.abs(dy) > 4) {
        hasMoved = true;
      }

      let newLeft = widgetStartX + dx;
      let newTop = widgetStartY + dy;

      const widgetWidth = macroWidget.offsetWidth;
      const widgetHeight = macroWidget.offsetHeight;

      // Bound within viewport
      newLeft = Math.max(10, Math.min(window.innerWidth - widgetWidth - 10, newLeft));
      newTop = Math.max(10, Math.min(window.innerHeight - widgetHeight - 10, newTop));

      macroWidget.style.left = `${newLeft}px`;
      macroWidget.style.top = `${newTop}px`;
    }

    function onMouseUp(e) {
      if (!isDragging) return;
      isDragging = false;
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);

      // If it was a click on the collapsed widget (didn't drag), expand it
      if (!hasMoved && macroWidget.classList.contains("collapsed")) {
        renderExpanded();
        // Make sure it doesn't expand off screen
        const rect = macroWidget.getBoundingClientRect();
        let newLeft = Math.max(10, Math.min(window.innerWidth - 300, rect.left));
        let newTop = Math.max(10, Math.min(window.innerHeight - 250, rect.top));
        macroWidget.style.left = `${newLeft}px`;
        macroWidget.style.top = `${newTop}px`;
      }
    }
  }

  // Initialize view and dragging
  renderCollapsed();
  initDraggable();

  // Let background worker know this tab is ready
  chrome.runtime.sendMessage({ type: "TAB_LOADED" }).catch(() => {});
})();

