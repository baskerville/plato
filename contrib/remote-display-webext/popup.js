const form = document.getElementById('form');
const urlInput = document.getElementById('url');
const topicInput = document.getElementById('topic');
const enabledInput = document.getElementById('enabled');
const keyInput = document.getElementById('key');
const monochromeInput = document.getElementById('monochrome');
const qualityInput = document.getElementById('quality');

form.addEventListener('submit', async (e) => {
  e.preventDefault();

  await browser.storage.local.set({
    wsUrl: urlInput.value,
    topic: topicInput.value,
    enabled: enabledInput.checked,
    key: keyInput.value,
    monochrome: monochromeInput.checked,
    quality: qualityInput.value,
  });

  // deno-lint-ignore no-window-prefix no-window
  window.close();
});

(async () => {
  const { wsUrl, topic, enabled, key, monochrome, quality } =
    await browser.storage.local.get(['wsUrl', 'topic', 'enabled', 'key', 'monochrome', 'quality']);

  urlInput.value = wsUrl || '';
  topicInput.value = topic || '';
  enabledInput.checked = enabled || false;
  keyInput.value = key || '';
  monochromeInput.checked = !!(monochrome ?? false);
  qualityInput.value = quality ?? 100;
})();
