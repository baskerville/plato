const form = document.getElementById('form');
const urlInput = document.getElementById('url');
const enabledInput = document.getElementById('enabled');

form.addEventListener('submit', async (e) => {
  e.preventDefault();
  
  await browser.storage.local.set({
    wsUrl: urlInput.value,
    enabled: enabledInput.checked
  });
  
  // deno-lint-ignore no-window-prefix
  window.close();
});

(async () => {
  const {wsUrl, enabled} = await browser.storage.local.get(['wsUrl', 'enabled']);
  
  urlInput.value = wsUrl || '';
  enabledInput.checked = enabled || false;
})();
