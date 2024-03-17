const form = document.getElementById('form');
const urlInput = document.getElementById('url');
const topicInput = document.getElementById('topic');
const enabledInput = document.getElementById('enabled');

form.addEventListener('submit', async (e) => {
  e.preventDefault();

  await browser.storage.local.set({
    wsUrl: urlInput.value,
    topic: topicInput.value,
    enabled: enabledInput.checked
  });

  // deno-lint-ignore no-window-prefix no-window
  window.close();
});

(async () => {
  const { wsUrl, topic, enabled } = await browser.storage.local.get(['wsUrl', 'topic', 'enabled']);

  urlInput.value = wsUrl || '';
  topicInput.value = topic || '';
  enabledInput.checked = enabled || false;
})();
