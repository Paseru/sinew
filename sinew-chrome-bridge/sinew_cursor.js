// ðŸ§¬ Sinew Chrome Bridge â€” Cursor & Overlay Script
// Injected into web pages to draw a SOTA biologically-realistic virtual cursor with spring physics.

(function () {
  const OVERLAY_ROOT_ID = "Sinew-agent-overlay-root";
  if (window.__sinewChromeBridgeReady) {
    return; // Already injected
  }
  window.__sinewChromeBridgeReady = true;

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
      filter: drop-shadow(0 0 6px #ff6b00) drop-shadow(0 0 14px #ff0080) drop-shadow(0 0 22px #66f7ff);
      animation: sinew-cursor-glow-pulse 2.2s ease-in-out infinite alternate;
    }

    @keyframes sinew-cursor-glow-pulse {
      0% {
        filter: drop-shadow(0 0 4px #ff6b00) drop-shadow(0 0 10px #ff0080) drop-shadow(0 0 16px #66f7ff);
      }
      100% {
        filter: drop-shadow(0 0 8px #ff6b00) drop-shadow(0 0 18px #ff0080) drop-shadow(0 0 28px #66f7ff);
      }
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
          <stop offset="0%" stop-color="#ff6b00">
            <animate attributeName="stop-color" values="#ff6b00;#ff0080;#66f7ff;#ff6b00" dur="3s" repeatCount="indefinite" />
          </stop>
          <stop offset="100%" stop-color="#ff0080">
            <animate attributeName="stop-color" values="#ff0080;#66f7ff;#ff6b00;#ff0080" dur="3s" repeatCount="indefinite" />
          </stop>
        </linearGradient>
      </defs>
      <!-- Premium Cyber-Neon triangle cursor design pointing to (4,3) -->
      <path d="M4.5 3L23.5 11.5L14 14.5L11.5 24L4.5 3Z" fill="url(#cyber-grad)" stroke="white" stroke-width="1.2" stroke-linejoin="round" />
    </svg>
  `;
  overlay.appendChild(pointer);

  const label = document.createElement("div");
  label.className = "cursor-label";
  label.textContent = "Sinew ACTIVE";
  // overlay.appendChild(label); // Disabled to match clean minimalist style (no flashing text next to the cursor)

  shadow.appendChild(overlay);
  const appendOverlayRoot = () => {
    const root = document.documentElement || document.body;
    if (root && !container.isConnected) root.appendChild(container);
  };
  if (document.documentElement || document.body) appendOverlayRoot();
  else document.addEventListener("DOMContentLoaded", appendOverlayRoot, { once: true });

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

  const initialCursorPoint = (() => {
    const edgeX = Math.random() < 0.5 ? 0.10 + Math.random() * 0.20 : 0.70 + Math.random() * 0.20;
    return {
      x: Math.round(window.innerWidth * edgeX),
      y: Math.round(window.innerHeight * (0.18 + Math.random() * 0.64))
    };
  })();

  // Springs for coordinates, stretching/scooting, and blur
  const xSpring = new Spring(initialCursorPoint.x);
  const ySpring = new Spring(initialCursorPoint.y);
  const stretchSpring = new Spring(1, 0.85, 0.15); // Velocity-based length stretch
  const blurSpring = new Spring(0, 0.9, 0.12);     // Motion blur

  let activeState = {
    x: initialCursorPoint.x,
    y: initialCursorPoint.y,
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
    // Disabled to match clean minimalist style (no flashing pink boxes)
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
    const wasHidden = !activeState.visible;
    const shouldSnapIntoView = wasHidden && state.visible && Number.isFinite(state.x) && Number.isFinite(state.y);
    if (shouldSnapIntoView) {
      xSpring.value = state.x;
      xSpring.target = state.x;
      xSpring.velocity = 0;
      ySpring.value = state.y;
      ySpring.target = state.y;
      ySpring.velocity = 0;
    }
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
  // Controlled tab visual indicator (Sinew-style glowing bar)
  // ==========================================================
  let controlledIndicatorTimer = null;

  // ==========================================================
  // Favicon Badge Status Indicators (Sinew Style)
  // ==========================================================
  const BADGE_CREATED_FLAG = "sinewFaviconBadgeCreated";
  const ORIGINAL_HREF_KEY = "sinewOriginalFaviconHref";

  function getFaviconLinks() {
    return Array.from(document.querySelectorAll('link[rel~="icon"], link[rel="shortcut icon"]'));
  }

  function restoreOriginalFavicon() {
    const links = getFaviconLinks();
    for (const link of links) {
      if (link.dataset[BADGE_CREATED_FLAG] === "true") {
        link.remove();
      } else if (link.dataset[ORIGINAL_HREF_KEY]) {
        link.href = link.dataset[ORIGINAL_HREF_KEY];
        delete link.dataset[BADGE_CREATED_FLAG];
        delete link.dataset[ORIGINAL_HREF_KEY];
        link.removeAttribute("data-sinew-badge");
      }
    }
  }

  function setFaviconBadge(status) {
    restoreOriginalFavicon();
    if (status === "detached" || status === "idle" || !status) {
      return;
    }

    const links = getFaviconLinks();
    let targetLink = links[0];

    if (!targetLink) {
      targetLink = document.createElement("link");
      targetLink.rel = "icon";
      targetLink.dataset[BADGE_CREATED_FLAG] = "true";
      if (document.head) document.head.appendChild(targetLink);
      else document.documentElement.appendChild(targetLink);
    }

    if (!targetLink.dataset[ORIGINAL_HREF_KEY]) {
      targetLink.dataset[ORIGINAL_HREF_KEY] = targetLink.getAttribute("href") || "";
    }

    const originalHref = targetLink.dataset[ORIGINAL_HREF_KEY];
    let absoluteOriginalHref = "";
    if (originalHref && !originalHref.startsWith("data:")) {
      try {
        absoluteOriginalHref = new URL(originalHref, window.location.href).href;
      } catch (e) {
        absoluteOriginalHref = originalHref;
      }
    }
    
    let badgeColor = "#ff6b00"; // Active: Neon Orange
    if (status === "recording") {
      badgeColor = "#ff0080"; // Recording: Neon Pink
    } else if (status === "completed") {
      badgeColor = "#66f7ff"; // Completed: Neon Teal (Teal matches Completed)
    }

    let svgContent = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">`;
    if (absoluteOriginalHref) {
      const escapedHref = absoluteOriginalHref.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
      svgContent += `<image href="${escapedHref}" width="32" height="32" />`;
    } else {
      svgContent += `<circle cx="16" cy="16" r="14" fill="none" stroke="#8fa0b0" stroke-width="1.5" /><path d="M16 2a14 14 0 0 1 0 28M2 16a14 14 0 0 1 28 0M16 2v28M2 16h28" stroke="#8fa0b0" stroke-width="1" fill="none" />`;
    }
    
    // Draw high-fidelity neon status badge iconography
    if (status === "recording") {
      svgContent += `<circle cx="25" cy="25" r="6" fill="#111827" stroke="#ffffff" stroke-width="1" /><circle cx="25" cy="25" r="3" fill="#ff0080" /></svg>`;
    } else if (status === "completed") {
      svgContent += `<circle cx="25" cy="25" r="6" fill="#111827" stroke="#ffffff" stroke-width="1" /><path d="M22 25 l2 2 l4 -4" fill="none" stroke="#66f7ff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>`;
    } else { // active
      svgContent += `<path d="M20 20 l8 3.5 l-3.5 1 l-1 3.5 z" fill="#ff6b00" stroke="#ffffff" stroke-width="1" /></svg>`;
    }

    targetLink.href = "data:image/svg+xml," + encodeURIComponent(svgContent);
    targetLink.dataset.sinewBadge = "true";
  }

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

  // Keep-alive connection to prevent background service worker suspension
  function connectKeepAlive() {
    try {
      const port = chrome.runtime.connect({ name: "sinew-keep-alive" });
      port.onDisconnect.addListener(() => {
        setTimeout(connectKeepAlive, 5000);
      });
    } catch (e) {
      setTimeout(connectKeepAlive, 5000);
    }
  }
  connectKeepAlive();

  // Listening to messages from background worker
  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    const resolveSelector = (sel) => {
      if (!sel) return "";
      const s = String(sel).trim();
      if (s.startsWith('@ref')) {
        return `[data-sinew-ref="${s.slice(4)}"]`;
      }
      if (s.startsWith('@')) {
        return `[data-sinew-ref="${s.slice(1)}"]`;
      }
      return s;
    };

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
      if (message.state.visible) {
        updateControlledTabIndicator("active");
        setFaviconBadge("active");
      }
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
      setFaviconBadge(message.status);
      sendResponse({ ok: true });
    }
    else if (message.type === "AGENT_QUERY_SELECTOR") {
      handleActivity();
      const selector = String(message.selector || "");
      const el = selector ? document.querySelector(resolveSelector(selector)) : null;
      if (!el) {
        sendResponse({ ok: false, success: false, error: `Selector not found: ${selector}` });
        return true;
      }
      if (message.scroll !== false && typeof el.scrollIntoView === "function") {
        el.scrollIntoView({ block: "center", inline: "center", behavior: "auto" });
      }
      const rect = el.getBoundingClientRect();
      const style = window.getComputedStyle(el);
      sendResponse({
        ok: true,
        success: true,
        selector,
        tagName: el.tagName,
        id: el.id || "",
        className: typeof el.className === "string" ? el.className : "",
        text: String(el.innerText || el.textContent || "").replace(/\s+/g, " ").trim().slice(0, 500),
        value: "value" in el ? el.value : null,
        href: el.href || el.getAttribute?.("href") || "",
        visible: rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden" && style.opacity !== "0",
        boundingBox: { x: Math.round(rect.left), y: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height) },
        center: { x: Math.round(rect.left + rect.width / 2), y: Math.round(rect.top + rect.height / 2) }
      });
      return true;
    }
    else if (message.type === "AGENT_CLICK_SELECTOR") {
      handleActivity();
      const selector = String(message.selector || "");
      const el = selector ? document.querySelector(resolveSelector(selector)) : null;
      if (!el) {
        sendResponse({ ok: false, success: false, error: `Selector not found: ${selector}` });
        return true;
      }
      if (message.scroll !== false && typeof el.scrollIntoView === "function") {
        el.scrollIntoView({ block: "center", inline: "center", behavior: "auto" });
      }
      const rect = el.getBoundingClientRect();
      const x = Math.round(rect.left + rect.width / 2);
      const y = Math.round(rect.top + rect.height / 2);
      
      // Téléportation instantanée et onde de choc (Mode Turbo Visuel)
      if (typeof xSpring !== "undefined" && typeof ySpring !== "undefined") {
        xSpring.value = x;
        xSpring.target = x;
        xSpring.velocity = 0;
        ySpring.value = y;
        ySpring.target = y;
        ySpring.velocity = 0;
        if (typeof updateCursorState === "function") {
          updateCursorState({ x, y, visible: true });
        }
        if (typeof triggerClickShockwave === "function") {
          triggerClickShockwave(x, y);
        }
      }

      const opts = { bubbles: true, cancelable: true, view: window, clientX: x, clientY: y, button: 0, buttons: 1, pointerId: 1, pointerType: "mouse", isPrimary: true };
      try {
        el.dispatchEvent(new PointerEvent("pointerover", opts));
        el.dispatchEvent(new MouseEvent("mouseover", opts));
        el.dispatchEvent(new PointerEvent("pointerdown", opts));
        el.dispatchEvent(new MouseEvent("mousedown", opts));
        el.focus?.({ preventScroll: true });
        el.dispatchEvent(new PointerEvent("pointerup", { ...opts, buttons: 0 }));
        el.dispatchEvent(new MouseEvent("mouseup", { ...opts, buttons: 0 }));
        if (typeof el.click === "function" && el.tagName !== "A") {
          el.click();
        } else {
          el.dispatchEvent(new MouseEvent("click", { ...opts, buttons: 0 }));
        }
        sendResponse({ ok: true, success: true, action: "click_selector", selector, tagName: el.tagName, id: el.id || "", href: el.href || el.getAttribute?.("href") || "", center: { x, y } });
      } catch (err) {
        sendResponse({ ok: false, success: false, error: err.message });
      }
      return true;
    }
    else if (message.type === "AGENT_TYPE_SELECTOR") {
      handleActivity();
      const selector = String(message.selector || "");
      const text = String(message.text || "");
      const el = selector ? document.querySelector(resolveSelector(selector)) : null;
      if (!el) {
        sendResponse({ ok: false, success: false, error: `Selector not found: ${selector}` });
        return true;
      }
      const isEditable = el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable || el.getAttribute("role") === "textbox";
      if (!isEditable) {
        sendResponse({ ok: false, success: false, error: `Target is not editable: ${el.tagName}` });
        return true;
      }
      el.scrollIntoView?.({ block: "center", inline: "center", behavior: "auto" });
      el.focus?.({ preventScroll: true });

      // Téléportation du curseur sur le champ de saisie (Mode Turbo Visuel)
      try {
        const rect = el.getBoundingClientRect();
        const x = Math.round(rect.left + rect.width / 2);
        const y = Math.round(rect.top + rect.height / 2);
        if (typeof xSpring !== "undefined" && typeof ySpring !== "undefined") {
          xSpring.value = x;
          xSpring.target = x;
          xSpring.velocity = 0;
          ySpring.value = y;
          ySpring.target = y;
          ySpring.velocity = 0;
          if (typeof updateCursorState === "function") {
            updateCursorState({ x, y, visible: true });
          }
        }
      } catch (e) {}

      const setValue = (value) => {
        if (el.isContentEditable || el.getAttribute("role") === "textbox") el.textContent = value;
        else el.value = value;
        el.dispatchEvent(new InputEvent("input", { bubbles: true, cancelable: true, inputType: "insertText", data: value }));
        el.dispatchEvent(new Event("change", { bubbles: true }));
      };
      setValue(text);
      if (message.submit) {
        el.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
        const form = el.closest && el.closest("form");
        if (form && typeof form.requestSubmit === "function") form.requestSubmit();
        else if (form) form.submit();
        el.dispatchEvent(new KeyboardEvent("keyup", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
      }
      sendResponse({ ok: true, success: true, action: "type_selector", selector, tagName: el.tagName, id: el.id || "", text });
      return true;
    }
    else if (message.type === "AGENT_PRESS_KEY") {
      handleActivity();
      const selector = String(message.selector || "");
      const key = String(message.key || "Enter");
      const target = selector ? document.querySelector(resolveSelector(selector)) : (document.activeElement || document.body);
      if (!target) {
        sendResponse({ ok: false, success: false, error: selector ? `Selector not found: ${selector}` : "No active element" });
        return true;
      }
      target.scrollIntoView?.({ block: "center", inline: "center", behavior: "auto" });
      target.focus?.({ preventScroll: true });
      const codeByKey = { Enter: "Enter", Escape: "Escape", Tab: "Tab", Backspace: "Backspace", Delete: "Delete", ArrowUp: "ArrowUp", ArrowDown: "ArrowDown", ArrowLeft: "ArrowLeft", ArrowRight: "ArrowRight", Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown", Space: "Space" };
      const code = String(message.code || codeByKey[key] || (key.length === 1 ? `Key${key.toUpperCase()}` : key));
      const opts = { key, code, bubbles: true, cancelable: true, ctrlKey: !!message.ctrlKey, shiftKey: !!message.shiftKey, altKey: !!message.altKey, metaKey: !!message.metaKey };
      try {
        target.dispatchEvent(new KeyboardEvent("keydown", opts));
        if (key.length === 1) target.dispatchEvent(new KeyboardEvent("keypress", opts));
        if (key === "Enter") {
          const form = target.closest && target.closest("form");
          if (form && message.submit !== false) {
            if (typeof form.requestSubmit === "function") form.requestSubmit();
            else form.submit();
          }
        }
        target.dispatchEvent(new KeyboardEvent("keyup", opts));
        sendResponse({ ok: true, success: true, action: "press_key", selector: selector || null, key, code, tagName: target.tagName || "" });
      } catch (err) {
        sendResponse({ ok: false, success: false, error: err.message });
      }
      return true;
    }
    else if (message.type === "AGENT_SELECT_OPTION") {
      handleActivity();
      const selector = String(message.selector || "");
      const select = selector ? document.querySelector(resolveSelector(selector)) : null;
      if (!select) {
        sendResponse({ ok: false, success: false, error: `Selector not found: ${selector}` });
        return true;
      }
      if (select.tagName !== "SELECT") {
        sendResponse({ ok: false, success: false, error: `Target is not a SELECT: ${select.tagName}` });
        return true;
      }
      const value = message.value !== undefined ? String(message.value) : null;
      const label = message.label !== undefined ? String(message.label).toLowerCase() : null;
      const index = Number.isInteger(message.index) ? message.index : null;
      const options = Array.from(select.options || []);
      let option = null;
      if (value !== null) option = options.find(opt => opt.value === value);
      if (!option && label !== null) option = options.find(opt => String(opt.label || opt.textContent || "").trim().toLowerCase() === label || String(opt.textContent || "").toLowerCase().includes(label));
      if (!option && index !== null) option = options[index] || null;
      if (!option) {
        sendResponse({ ok: false, success: false, error: "Option not found", options: options.map((opt, i) => ({ index: i, value: opt.value, label: opt.label || opt.textContent || "" })).slice(0, 50) });
        return true;
      }
      select.scrollIntoView?.({ block: "center", inline: "center", behavior: "auto" });
      select.focus?.({ preventScroll: true });
      select.value = option.value;
      option.selected = true;
      select.dispatchEvent(new InputEvent("input", { bubbles: true, cancelable: true, inputType: "insertReplacementText", data: option.value }));
      select.dispatchEvent(new Event("change", { bubbles: true }));
      sendResponse({ ok: true, success: true, action: "select_option", selector, value: option.value, label: option.label || option.textContent || "", index: options.indexOf(option) });
      return true;
    }
    else if (message.type === "AGENT_WAIT_SELECTOR") {
      handleActivity();
      const selector = String(message.selector || "");
      const visibleOnly = message.visible !== false;
      const timeoutMs = Math.max(0, Number(message.timeoutMs) || 5000);
      const started = Date.now();
      const check = () => {
        const el = selector ? document.querySelector(resolveSelector(selector)) : null;
        if (el) {
          const rect = el.getBoundingClientRect();
          const style = window.getComputedStyle(el);
          const visible = rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden" && style.opacity !== "0";
          if (!visibleOnly || visible) {
            sendResponse({ ok: true, success: true, selector, visible, elapsedMs: Date.now() - started });
            return true;
          }
        }
        return false;
      };
      if (check()) return true;
      const timer = setTimeout(() => {
        observer.disconnect();
        clearInterval(poll);
        sendResponse({ ok: false, success: false, error: `Timeout waiting for selector: ${selector}`, elapsedMs: Date.now() - started });
      }, timeoutMs);
      const finishIfFound = () => {
        if (check()) {
          clearTimeout(timer);
          clearInterval(poll);
          observer.disconnect();
        }
      };
      const observer = new MutationObserver(finishIfFound);
      observer.observe(document.documentElement || document, { childList: true, subtree: true, attributes: true });
      const poll = setInterval(finishIfFound, 100);
      return true;
    }
    else if (message.type === "AGENT_EVALUATE") {
      handleActivity();
      try {
        const source = String(message.expression || "undefined");
        let value;
        try {
          value = (0, eval)(`(${source})`);
        } catch {
          value = (0, eval)(source);
        }
        Promise.resolve(value).then((resolved) => {
          let jsonValue = resolved;
          try { JSON.stringify(jsonValue); } catch { jsonValue = String(jsonValue); }
          sendResponse({ ok: true, success: true, value: jsonValue });
        }).catch((err) => sendResponse({ ok: false, success: false, error: err.message }));
      } catch (err) {
        sendResponse({ ok: false, success: false, error: err.message });
      }
      return true;
    }
    else if (message.type === "AGENT_DOM_CLICK") {
      handleActivity();
      const x = Number(message.x);
      const y = Number(message.y);
      const initialEl = document.elementFromPoint(x, y);
      const el = initialEl && initialEl.closest
        ? (initialEl.closest('button, a, input, textarea, select, [role="button"], [onclick], summary, label') || initialEl)
        : initialEl;
      const anchorEl = el && el.closest ? el.closest('a[href]') : null;
      const hrefToNavigate = anchorEl && anchorEl.href && !/^\s*(#|javascript:)/i.test(anchorEl.getAttribute('href') || '')
        ? anchorEl.href
        : "";
      const beforeHref = location.href;
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
      
      if (typeof el.click === "function" && el.tagName !== "A") {
        el.click();
      } else {
        const clickEvent = new MouseEvent("click", { ...opts, bubbles: true, cancelable: true, buttons: 0 });
        el.dispatchEvent(clickEvent);
      }
      sendResponse({ ok: true, tagName: el.tagName, id: el.id || "", className: typeof el.className === "string" ? el.className : "", href: hrefToNavigate });
      return true;
    }
    else if (message.type === "AGENT_DOM_TYPE") {
      handleActivity();
      const x = Number(message.x);
      const y = Number(message.y);
      const text = String(message.text || "");
      const delayMs = Math.max(10, Number(message.delayMs) || 70);
      const initialEl = document.elementFromPoint(x, y);
      const el = initialEl && initialEl.closest
        ? (initialEl.closest('input, textarea, [contenteditable="true"], [role="textbox"]') || initialEl)
        : initialEl;
      if (!el) {
        sendResponse({ ok: false, error: "No editable element at target point" });
        return true;
      }
      const isEditable = el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable || el.getAttribute("role") === "textbox";
      if (!isEditable) {
        sendResponse({ ok: false, error: `Target is not editable: ${el.tagName}` });
        return true;
      }
      el.focus?.({ preventScroll: true });
      const setValue = (value) => {
        if (el.isContentEditable || el.getAttribute("role") === "textbox") {
          el.textContent = value;
        } else {
          el.value = value;
        }
        el.dispatchEvent(new InputEvent("input", { bubbles: true, cancelable: true, inputType: "insertText", data: value }));
        el.dispatchEvent(new Event("change", { bubbles: true }));
      };
      let current = el.isContentEditable || el.getAttribute("role") === "textbox" ? (el.textContent || "") : (el.value || "");
      if (current) {
        current = "";
        setValue(current);
      }
      (async () => {
        for (const ch of text) {
          current += ch;
          el.dispatchEvent(new KeyboardEvent("keydown", { key: ch, bubbles: true, cancelable: true }));
          setValue(current);
          el.dispatchEvent(new KeyboardEvent("keyup", { key: ch, bubbles: true, cancelable: true }));
          await new Promise(resolve => setTimeout(resolve, delayMs + Math.random() * delayMs));
        }
        if (message.submit) {
          el.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
          const form = el.closest && el.closest("form");
          if (form && typeof form.requestSubmit === "function") form.requestSubmit();
          else if (form) form.submit();
          el.dispatchEvent(new KeyboardEvent("keyup", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
        }
        sendResponse({ ok: true, tagName: el.tagName, id: el.id || "", className: typeof el.className === "string" ? el.className : "", text });
      })();
      return true;
    }
    else if (message.type === "AGENT_DOM_SCROLL") {
      handleActivity();
      const amount = Number(message.scrollY) || Math.round(window.innerHeight * 0.6);
      window.scrollBy({ top: amount, left: 0, behavior: "smooth" });
      sendResponse({ ok: true, scrollY: amount });
      return true;
    }
    else if (message.type === "AGENT_PAGE_SNAPSHOT") {
      handleActivity();
      const limit = Math.max(1, Math.min(200, Number(message.limit) || 80));
      const clean = (value, max = 160) => String(value || "").replace(/\s+/g, " ").trim().slice(0, max);
      const roleFor = (el) => {
        const role = el.getAttribute("role");
        if (role) return role;
        if (el.tagName === "A" && el.getAttribute("href")) return "link";
        if (el.tagName === "BUTTON") return "button";
        if (el.tagName === "TEXTAREA") return "textbox";
        if (el.tagName === "INPUT") {
          const type = (el.getAttribute("type") || "text").toLowerCase();
          if (["button", "submit", "reset"].includes(type)) return "button";
          if (["checkbox", "radio", "range"].includes(type)) return type;
          return "textbox";
        }
        if (el.tagName === "SELECT") return "combobox";
        if (/^H[1-6]$/.test(el.tagName)) return "heading";
        return null;
      };
      const selectorFor = (el) => {
        const esc = (v) => window.CSS?.escape ? window.CSS.escape(String(v)) : String(v).replace(/[^a-zA-Z0-9_-]/g, "\\$&");
        const candidates = [];
        if (el.id) candidates.push(`#${esc(el.id)}`);
        for (const attr of ["data-testid", "data-test", "data-cy", "name", "aria-label"]) {
          const value = el.getAttribute(attr);
          if (value) candidates.push(`${el.tagName.toLowerCase()}[${attr}="${esc(value)}"]`);
        }
        if (el.tagName === "A" && el.getAttribute("href")) candidates.push(`a[href="${esc(el.getAttribute("href"))}"]`);
        const path = [];
        let cur = el;
        while (cur && cur.nodeType === Node.ELEMENT_NODE && cur !== document.documentElement && path.length < 6) {
          let part = cur.tagName.toLowerCase();
          if (cur.id) {
            part += `#${esc(cur.id)}`;
            path.unshift(part);
            break;
          }
          let nth = 1;
          let sib = cur.previousElementSibling;
          while (sib) {
            if (sib.tagName === cur.tagName) nth++;
            sib = sib.previousElementSibling;
          }
          if (nth > 1) part += `:nth-of-type(${nth})`;
          path.unshift(part);
          cur = cur.parentElement;
        }
        if (path.length) candidates.push(path.join(" > "));
        const unique = Array.from(new Set(candidates)).filter(Boolean).slice(0, 8);
        return { primary: unique[0] || null, candidates: unique };
      };
      const nodes = Array.from(document.querySelectorAll('a[href], button, input, textarea, select, [role], [aria-label], [title], [onclick], [tabindex], [contenteditable="true"], summary, label, h1, h2, h3'));
      const items = [];
      document.querySelectorAll('.sinew-ref-badge').forEach(b => b.remove());
      for (const el of nodes) {
        const rect = el.getBoundingClientRect();
        const style = window.getComputedStyle(el);
        const visible = rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden" && style.opacity !== "0" && rect.bottom >= 0 && rect.right >= 0 && rect.top <= window.innerHeight && rect.left <= window.innerWidth;
        if (!visible) continue;
        const editable = el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable || el.getAttribute("role") === "textbox";
        const clickable = el.tagName === "A" || el.tagName === "BUTTON" || el.getAttribute("role") === "button" || style.cursor === "pointer" || el.hasAttribute("onclick") || el.hasAttribute("tabindex") || el.tagName === "SUMMARY" || el.tagName === "LABEL";
        const role = roleFor(el);
        const text = clean(el.innerText || el.textContent || el.getAttribute("value") || "");
        const ariaName = clean(el.getAttribute("aria-label") || el.getAttribute("title") || el.getAttribute("alt") || el.getAttribute("placeholder") || "");
        if (!editable && !clickable && !ariaName && !text && !role) continue;
        const selector = selectorFor(el);
        const nodeId = items.length + 1;
        el.setAttribute("data-sinew-ref", String(nodeId));
        if (visible) {
          const badge = document.createElement("div");
          badge.className = "sinew-ref-badge";
          badge.textContent = `@ref${nodeId}`;
          badge.style.cssText = "position:absolute;left:" + Math.max(0, rect.left + window.scrollX) + "px;top:" + Math.max(0, rect.top + window.scrollY - 15) + "px;background:#ff0080;color:#ffffff;font-family:Arial,sans-serif;font-size:10px;font-weight:bold;padding:1px 4px;border-radius:3px;z-index:2147483645;pointer-events:none;box-shadow:0 0 4px #ff0080;opacity:0.85;";
          document.body.appendChild(badge);
        }
        items.push({
          nodeId,
          tagName: el.tagName,
          role,
          visible,
          clickable,
          editable,
          visibleText: text || null,
          ariaName: ariaName || null,
          href: el.getAttribute("href") || null,
          boundingBox: { x: Math.round(rect.left), y: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height) },
          center: { x: Math.round(rect.left + rect.width / 2), y: Math.round(rect.top + rect.height / 2) },
          selector,
          preview: clean([role, ariaName || text, el.getAttribute("href") || ""].filter(Boolean).join(" | "), 220)
        });
        if (items.length >= limit) break;
      }
      setTimeout(() => {
        document.querySelectorAll('.sinew-ref-badge').forEach(b => b.remove());
      }, 8000);
      sendResponse({ success: true, href: location.href, title: document.title, viewport: { width: window.innerWidth, height: window.innerHeight }, items });
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

      const elements = Array.from(document.querySelectorAll('button, a, input, select, textarea, [role="button"], [onclick], [aria-label], [title], div, span, svg, li, summary, article, section'));
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
      let typeText = "";
      let typeSubmit = /\b(entrée|entrer|enter|valide|submit|recherche)\b/.test(taskText);
      if (action === "type") {
        const cleanTypeText = (value) => String(value || "")
          .replace(/^[\s`"'“”‘’]+|[\s`"'“”‘’]+$/g, "")
          .replace(/^(exactement|exact|precisement|précisément)\s+/i, "")
          .replace(/[,.!?;:]+$/g, "")
          .trim();
        const quotedTypeMatch = task.match(/(?:tape|écris|ecris|saisis|type)\s+(?:exactement\s+)?[`"“'‘]([^`"”'’]+)[`"”'’]/i);
        const typeMatch = quotedTypeMatch || task.match(/(?:tape|écris|ecris|saisis|type)\s+(?:exactement\s+)?(.+?)(?:\s+puis|\s+et\b|[,;]\s*(?:valide|valides|appuie|clique|clic|click)|$)/i);
        typeText = cleanTypeText(typeMatch && typeMatch[1] ? typeMatch[1] : "");
        const domainMatch = task.match(/\b[a-z0-9-]+(?:\.[a-z0-9-]+)+(?:\/[^\s,;)]*)?\b/i);
        if ((!typeText || typeText.includes("google") || typeText.includes("exactement")) && domainMatch) typeText = cleanTypeText(domainMatch[0]);
        if (!typeText && taskText.includes("julienpiron")) typeText = "julienpiron.fr";
      }

      const wantsMenu = taskText.includes("hamburger") || taskText.includes("menu") || taskText.includes("burger");
      const wantsMenuClose = wantsMenu && /\b(referme|ferme|fermer|close|dismiss|x)\b/.test(taskText);
      const wantsMenuOpen = wantsMenu && !wantsMenuClose;

      let bestEl = null;
      let bestScore = -1;

      const directSearchRequested = (taskText.includes("google") || taskText.includes("recherche") || taskText.includes("search")) && (action === "click" || action === "type");
      if (directSearchRequested) {
        const directSearch = document.querySelector('textarea[name="q"], input[name="q"], textarea[aria-label*="search" i], input[aria-label*="search" i], textarea[aria-label*="rechercher" i], input[aria-label*="rechercher" i], [role="combobox"][aria-label*="search" i], [role="combobox"][aria-label*="rechercher" i]');
        if (directSearch && typeof directSearch.scrollIntoView === "function") {
          directSearch.scrollIntoView({ block: "center", inline: "center", behavior: "auto" });
          bestEl = directSearch;
          bestScore = 1200;
        }
      }

      if (!bestEl && action === "click") {
        const domainMatch = taskText.match(/\b[a-z0-9-]+(?:\.[a-z0-9-]+)+(?:\/[^\s,;)]*)?\b/i);
        if (domainMatch && !directSearchRequested) {
          const host = domainMatch[0].replace(/^https?:\/\//, "").replace(/^www\./, "").toLowerCase();
          const linkMatchesHost = (a) => {
            const rawHref = (a.getAttribute('href') || '').trim();
            if (!rawHref || rawHref.startsWith('#') || rawHref.startsWith('javascript:')) return false;
            const candidates = [rawHref];
            try {
              const parsed = new URL(rawHref, location.href);
              for (const key of ['url', 'q', 'u']) {
                const value = parsed.searchParams.get(key);
                if (value) candidates.push(value);
              }
            } catch {}
            return candidates.some(candidate => {
              try {
                const parsed = new URL(candidate, location.href);
                const candidateHost = parsed.hostname.replace(/^www\./, '').toLowerCase();
                return candidateHost === host || candidateHost.endsWith(`.${host}`);
              } catch {
                return false;
              }
            });
          };
          const directLink = Array.from(document.querySelectorAll('a[href]'))
            .filter(a => {
              const rect = a.getBoundingClientRect();
              if (rect.width <= 0 || rect.height <= 0) return false;
              if (rect.top < 90 || rect.left > window.innerWidth - 220) return false;
              const className = (typeof a.className === 'string' ? a.className : '').toLowerCase();
              const ariaLabel = (a.getAttribute('aria-label') || '').toLowerCase();
              if (/\bgb_|google apps|compte google|google account/.test(`${className} ${ariaLabel}`)) return false;
              return linkMatchesHost(a);
            })
            .sort((a, b) => {
              const ah = a.querySelector('h3') ? 1 : 0;
              const bh = b.querySelector('h3') ? 1 : 0;
              if (ah !== bh) return bh - ah;
              return a.getBoundingClientRect().top - b.getBoundingClientRect().top;
            })[0];
          if (directLink && typeof directLink.scrollIntoView === "function") {
            directLink.scrollIntoView({ block: "center", inline: "center", behavior: "auto" });
            bestEl = directLink;
            bestScore = 1250;
          }
        }
      }

      if (!bestEl && taskText.includes("trinity")) {
        const directTrinity = document.querySelector('#trinity-card, .trinity-card, article[id*="trinity" i], article[class*="trinity" i], a[href*="trinity" i], [data-project*="trinity" i], [data-id*="trinity" i]');
        if (directTrinity && typeof directTrinity.scrollIntoView === "function") {
          directTrinity.scrollIntoView({ block: "center", inline: "center", behavior: "auto" });
          bestEl = directTrinity;
          bestScore = 1000;
        }
      }

      if (!bestEl) elements.forEach(el => {
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
        const dataAttrs = Array.from(el.attributes || []).filter(attr => attr.name.startsWith("data-")).map(attr => `${attr.name} ${attr.value}`).join(" ").toLowerCase();

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
          if (dataAttrs.includes(word)) score += 35;
        });

        const isButtonLike = el.tagName === "BUTTON" || el.tagName === "A" || role === "button" || style.cursor === "pointer" || el.hasAttribute("onclick") || el.hasAttribute("tabindex");
        const iconOnly = text.length <= 3 && rect.width <= 96 && rect.height <= 96;
        const iconSignal = el.querySelectorAll ? el.querySelectorAll('svg,path,line,span').length : 0;
        const signature = `${id} ${className} ${ariaLabel} ${title} ${name} ${dataAttrs}`;
        const menuSignal = /(^|\s|_|-)(hamburger|burger|menu|nav|navbar|toggle|drawer|bars)(\s|$|_|-)/.test(signature);
        const menuGeometry = iconOnly && (text === "☰" || text === "≡" || iconSignal >= 2 || rect.top < window.innerHeight * 0.35);
        const closeSignal = /(^|\s|_|-)(close|fermer|dismiss|modal-close)(\s|$|_|-)/.test(signature) || (iconOnly && (text === "×" || text === "x"));

        if (action === "type" && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable || role === "textbox")) {
          score += 160;
          if (name === "q" || id === "search" || ariaLabel.includes("search") || ariaLabel.includes("recherche") || title.includes("search") || title.includes("recherche")) score += 180;
        }
        if (action === "click" && isButtonLike) score += 25;
        if (wantsMenu) {
          if (wantsMenuOpen && closeSignal) return;
          if (wantsMenuClose && !closeSignal && !menuSignal) return;
          if (menuSignal) score += 240;
          if (menuGeometry) score += 170;
          if (wantsMenuClose && closeSignal) score += 320;
          if (rect.top < window.innerHeight * 0.35 && (rect.left < 180 || rect.right > window.innerWidth - 180)) score += 80;
        }
        if (taskText.includes("trinity")) {
          if (el.tagName === "IFRAME" || className.includes("terminal-panel") || className.includes("terminal")) return;
          const hasTrinity = text.includes("trinity") || id.includes("trinity") || className.includes("trinity") || href.includes("trinity") || ariaLabel.includes("trinity") || title.includes("trinity");
          if (!hasTrinity) return;
          score += 220;
          if (id.includes("trinity-card") || className.includes("trinity-card")) score += 500;
          if (el.tagName === "ARTICLE" || className.includes("project-card")) score += 120;
        }

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
            viewport: { width: window.innerWidth, height: window.innerHeight },
             element: { tagName: bestEl.tagName, id: bestEl.id, className: typeof bestEl.className === 'string' ? bestEl.className : "", href: bestEl.href || bestEl.getAttribute?.('href') || "" },
            score: bestScore
          },
          text: action === "type" ? typeText : undefined,
          submit: action === "type" ? typeSubmit : undefined,
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
    // shadow.appendChild(macroWidget); // Disabled to avoid floating red dot on the right, matching clean minimalist style

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
    setFaviconBadge("recording");
    renderExpanded();
    triggerClickShockwave(window.innerWidth / 2, window.innerHeight / 2);
  }

  function stopRecordingMacro() {
    isRecording = false;
    setFaviconBadge(activeState.visible ? "active" : "detached");
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

