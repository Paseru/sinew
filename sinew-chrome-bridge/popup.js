// popup.js — Dynamic high-performance UI state driver for Sinew Chrome Bridge

document.addEventListener('DOMContentLoaded', () => {
  const bodyContainer = document.getElementById('body-container');
  const pill = document.getElementById('status-pill');
  const text = document.getElementById('status-text');
  const count = document.getElementById('attached-count');
  const btn = document.getElementById('btn-refresh');
  const restartBtn = document.getElementById('btn-restart');
  const diag = document.getElementById('diagnostic');
  const btnSettings = document.getElementById('btn-settings');
  const diagPanel = document.getElementById('diagnostics-panel');

  // Toggle diagnostics panel
  btnSettings.addEventListener('click', () => {
    diagPanel.classList.toggle('expanded');
  });

  function diagnosticText(response, fallback = '') {
    const when = response?.lastConnectedAt ? new Date(response.lastConnectedAt).toLocaleTimeString() : 'never';
    const causes = response?.diagnostics?.causes || [];
    if (response?.connected) {
      return `native host connected · tabs ${response.attachedCount || 0} · since ${when}${causes.length ? ` · ${causes.join(' · ')}` : ''}`;
    }
    return `${response?.lastNativeError || causes.join(' · ') || fallback || 'native host not connected yet'}`;
  }

  function updateStatus() {
    chrome.runtime.sendMessage({ action: "get_status" }, (response) => {
      if (chrome.runtime.lastError || !response) {
        const reason = chrome.runtime.lastError?.message || 'service worker sleeping';
        chrome.storage.local.get(['connected', 'attachedCount', 'lastNativeError', 'lastConnectedAt', 'diagnostics'], (data) => {
          setConnected(!!data.connected);
          count.textContent = data.attachedCount || 0;
          diag.textContent = diagnosticText(data, reason);
        });
        return;
      }

      setConnected(!!response.connected);
      count.textContent = response.attachedCount || 0;
      diag.textContent = diagnosticText(response);
    });
  }

  function setConnected(isConnected) {
    if (isConnected) {
      pill.classList.remove('disconnected');
      pill.classList.add('connected');
      text.textContent = 'Connected';
      bodyContainer.classList.remove('disconnected');
      bodyContainer.classList.add('connected');
    } else {
      pill.classList.remove('connected');
      pill.classList.add('disconnected');
      text.textContent = 'Disconnected';
      bodyContainer.classList.remove('connected');
      bodyContainer.classList.add('disconnected');
      
      // Auto expand diagnostics if disconnected to help developers
      diagPanel.classList.add('expanded');
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

  restartBtn.addEventListener('click', () => {
    restartBtn.disabled = true;
    restartBtn.querySelector('span').textContent = 'Restarting...';
    chrome.runtime.sendMessage({ action: "restart_bridge" }, (response) => {
      setTimeout(() => {
        restartBtn.disabled = false;
        restartBtn.querySelector('span').textContent = response && response.success === false ? 'Restart failed' : 'Restart bridge';
        updateStatus();
      }, 1200);
    });
  });

  // Init & Poll status while open
  updateStatus();
  const interval = setInterval(updateStatus, 1000);
  window.addEventListener('unload', () => clearInterval(interval));
});
