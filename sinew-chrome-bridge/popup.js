// popup.js — Dynamic high-performance UI state driver for Sinew Chrome Bridge

document.addEventListener('DOMContentLoaded', () => {
  const pill = document.getElementById('status-pill');
  const text = document.getElementById('status-text');
  const count = document.getElementById('attached-count');
  const btn = document.getElementById('btn-refresh');

  function updateStatus() {
    chrome.runtime.sendMessage({ action: "get_status" }, (response) => {
      if (chrome.runtime.lastError || !response) {
        // Service worker sleep fallback checks
        chrome.storage.local.get(['connected', 'attachedCount'], (data) => {
          setConnected(!!data.connected);
          count.textContent = data.attachedCount || 0;
        });
        return;
      }

      setConnected(!!response.connected);
      count.textContent = response.attachedCount || 0;
    });
  }

  function setConnected(isConnected) {
    if (isConnected) {
      pill.classList.remove('disconnected');
      pill.classList.add('connected');
      text.textContent = 'Connected';
    } else {
      pill.classList.remove('connected');
      pill.classList.add('disconnected');
      text.textContent = 'Disconnected';
    }
  }

  btn.addEventListener('click', () => {
    btn.style.transform = 'scale(0.95)';
    setTimeout(() => btn.style.transform = 'none', 100);
    
    chrome.runtime.sendMessage({ action: "reconnect" }, () => {
      setTimeout(updateStatus, 500);
    });
  });

  // Init & Poll status while open
  updateStatus();
  const interval = setInterval(updateStatus, 1000);
  window.addEventListener('unload', () => clearInterval(interval));
});
