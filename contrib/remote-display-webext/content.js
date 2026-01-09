class AnimationTracker {
  constructor(options = {}) {
    const { checkInterval = 50, maxWaitTime = 5000 } = options;
    this.checkInterval = checkInterval;
    this.maxWaitTime = maxWaitTime;
    this.timeoutId = null;
    this.screenshotTimeout = null;
  }

  getAnimationCount() {
    try {
      return document.getAnimations()
        .filter(a => a.playState !== "finished")
        .length;
    } catch (error) {
      console.error('Error getting animations:', error);
      return 0;
    }
  }

  queueScreenshot() {
    if (this.screenshotTimeout) clearTimeout(this.screenshotTimeout);
    this.screenshotTimeout = setTimeout(() => {
      this.screenshotTimeout = null;
      console.log('Sending screenshot request');
      browser.runtime.sendMessage({
        type: 'CAPTURE_SCREENSHOT'
      });
    }, 100);
  }

  waitForAnimations() {
    if (this.timeoutId) {
      console.log("Stopped previously running animation tracking");
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }
    const initialCount = this.getAnimationCount();
    console.log(`Starting animation tracking. Initial count: ${initialCount}`);

    let elapsedTime = 0;
    let lastCount = initialCount;
    let highestCount = initialCount;

    const checkAnimations = () => {
      const currentCount = this.getAnimationCount();

      if (currentCount !== lastCount) {
        console.log(`Animation count changed: ${lastCount} → ${currentCount}`);
        lastCount = currentCount;
        highestCount = Math.max(highestCount, currentCount);
      }

      if (highestCount > 0 && currentCount === 0) {
        console.log(`There were ${highestCount} animations, but we now have zero. Taking screenshot.`);
        this.queueScreenshot();
        return;
      }

      if (currentCount < initialCount) {
        console.log(`Animations decreased from ${initialCount} to ${currentCount}`);
        this.queueScreenshot();
        return;
      }

      elapsedTime += this.checkInterval;

      // Stop checking if we've reached the maximum wait time
      if (elapsedTime >= this.maxWaitTime) {
        console.log(`Animation tracking timed out after ${this.maxWaitTime}ms with ${currentCount} animations still running`);
        return;
      }

      // Continue checking
      this.timeoutId = setTimeout(checkAnimations, this.checkInterval);
    };

    checkAnimations();
  }
}

function isInIframe() {
  try {
    return window.self !== window.top;
  } catch (e) {
    return true;
  }
}

const generateTextFragment = async (selection) => {
  const src = browser.runtime.getURL('fragment-generation-utils.bundle.mjs');
  const { generateFragment } = await import(src);
  const result = generateFragment(selection);

  if (result.status !== 0) {
    return null;
  }

  let url = `${location.origin}${location.pathname}${location.search}`;

  const fragment = result.fragment;
  const prefix = fragment.prefix
    ? `${encodeURIComponent(fragment.prefix)}-,`
    : '';
  const suffix = fragment.suffix
    ? `,-${encodeURIComponent(fragment.suffix)}`
    : '';
  const start = encodeURIComponent(fragment.textStart);
  const end = fragment.textEnd ? `,${encodeURIComponent(fragment.textEnd)}` : '';

  url += `#:~:text=${prefix}${start}${end}${suffix}`;

  return url;
};


const tracker = new AnimationTracker();
browser.runtime.onMessage.addListener(async (message) => {
  if (isInIframe()) return;
  if (message.type === 'GENERATE_TEXT_FRAGMENT') {
    const selection = window.getSelection();
    if (selection && selection.rangeCount > 0) {
      const url = await generateTextFragment(selection);
      if (url) window.location.href = url;
    } else {
      console.log('No text selected for text fragment generation');
    }
  } else if (message.type === 'WAIT_FOR_ANIMATIONS') {
    tracker.waitForAnimations();
  }
  return;
});
console.log('Animation tracking content script loaded');