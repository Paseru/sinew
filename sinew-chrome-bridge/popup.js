// popup.js â€” Premium UI interactive states for Sinew Chrome Bridge

document.addEventListener('DOMContentLoaded', () => {
  const dot = document.getElementById('status-dot');
  const text = document.getElementById('status-text');
  const count = document.getElementById('attached-count');
  const btn = document.getElementById('btn-refresh');

  function updateStatus() {
    // Send a message to background service worker to get connection status
    chrome.runtime.sendMessage({ action: "get_status" }, (response) => {
      // Handle the case where service worker is asleep or doesn't respond
      if (chrome.runtime.lastError || !response) {
        // Fallback checks by querying background state or variables if possible
        // For standard MV3, we can check if WebSocket is active via storage
        chrome.storage.local.get(['connected', 'attachedCount'], (data) => {
          if (data.connected) {
            setConnected(true);
            count.textContent = data.attachedCount || 0;
          } else {
            setConnected(false);
            count.textContent = 0;
          }
        });
        return;
      }

      if (response.connected) {
        setConnected(true);
        count.textContent = response.attachedCount || 0;
      } else {
        setConnected(false);
        count.textContent = 0;
      }
    });
  }

  function setConnected(isConnected) {
    if (isConnected) {
      dot.classList.add('connected');
      text.textContent = 'ConnectÃ©';
      text.style.color = '#30d158';
    } else {
      dot.classList.remove('connected');
      text.textContent = 'Hors ligne';
      text.style.color = '#ff453a';
    }
  }

  btn.addEventListener('click', () => {
    btn.style.transform = 'scale(0.95)';
    setTimeout(() => btn.style.transform = 'none', 100);
    
    // Notify background to reconnect
    chrome.runtime.sendMessage({ action: "reconnect" }, () => {
      setTimeout(updateStatus, 500);
    });
  });

  // Initial check
  updateStatus();
  // Poll status every second while popup is open
  const interval = setInterval(updateStatus, 1000);
  
  // Cleanup on close
  window.addEventListener('unload', () => clearInterval(interval));
});
