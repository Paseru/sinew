// popup.js — Dynamic high-performance UI state driver for Sinew Chrome Bridge

document.addEventListener('DOMContentLoaded', () => {
  const pill = document.getElementById('status-pill');
  const text = document.getElementById('status-text');
  const count = document.getElementById('attached-count');
  const btn = document.getElementById('btn-refresh');
  const diag = document.getElementById('diagnostic');

  function updateStatus() {
    chrome.runtime.sendMessage({ action: "get_status" }, (response) => {
      if (chrome.runtime.lastError || !response) {
        const reason = chrome.runtime.lastError?.message || 'service worker sleeping / no response';
        chrome.storage.local.get(['connected', 'attachedCount', 'lastNativeError', 'lastConnectedAt'], (data) => {
          setConnected(!!data.connected);
          count.textContent = data.attachedCount || 0;
          diag.textContent = data.lastNativeError
            ? `Diagnostic: ${data.lastNativeError}`
            : `Diagnostic: ${reason}`;
        });
        return;
      }

      setConnected(!!response.connected);
      count.textContent = response.attachedCount || 0;
      const when = response.lastConnectedAt ? new Date(response.lastConnectedAt).toLocaleTimeString() : 'never';
      diag.textContent = response.connected
        ? `Diagnostic: native host connected · tabs ${response.attachedCount || 0} · since ${when}`
        : `Diagnostic: ${response.lastNativeError || 'native host not connected yet'}`;
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
    
    btn.disabled = true;
    btn.querySelector('span').textContent = 'Checking...';
    chrome.runtime.sendMessage({ action: "reconnect" }, () => {
      setTimeout(() => {
        btn.disabled = false;
        btn.querySelector('span').textContent = 'Reconnect';
        updateStatus();
      }, 800);
    });
  });

  // Init & Poll status while open
  updateStatus();
  const interval = setInterval(updateStatus, 1000);
  window.addEventListener('unload', () => clearInterval(interval));
});
