// #region browser interactivity

let windowId;
browser.tabs.query({ active: true, currentWindow: true })
  .then((tabs) => {
    windowId = tabs[0].windowId;
  });

async function tabOffset(offset) {
  const tabs = await browser.tabs.query({ windowId });
  const currentTab = tabs.find((tab) => tab.active);
  const currentIndex = tabs.indexOf(currentTab);
  const newIndex = (currentIndex + offset + tabs.length) % tabs.length;
  await browser.tabs.update(tabs[newIndex].id, { active: true });
}

async function windowOffset(offset) {
  const windows = await browser.windows.getAll({ populate: true });
  const currentWindow = windows.find((window) => window.id === windowId);
  const currentIndex = windows.indexOf(currentWindow);
  const newIndex = (currentIndex + offset + windows.length) % windows.length;
  windowId = windows[newIndex].id;
  await new Promise((resolve) => setTimeout(resolve, 100));
}

async function currentTab() {
  const [tab] = await browser.tabs.query({ windowId, active: true });
  return tab;
}

async function currentTabInfo() {
  const tabs = await browser.tabs.query({ windowId });
  const currentTab = tabs.find((tab) => tab.active);
  const currentTabIndex = tabs.indexOf(currentTab);
  const windows = await browser.windows.getAll({ populate: true });
  const currentWindow = windows.find((window) => window.id === windowId);
  const currentWindowIndex = windows.indexOf(currentWindow);
  const url = new URL(currentTab.url);
  return `W${currentWindowIndex + 1} T${currentTabIndex + 1
    }/${tabs.length} ${url.host}`;
}

async function scroll(pctX, pctY, pct) {
  const { id } = await currentTab();
  await browser.tabs.executeScript(id, {
    code: `(() => {
      const el = [...document.elementsFromPoint(
        window.innerWidth * ${pctX}, window.innerHeight * ${pctY}
      )].find((e) => Math.abs(e.scrollHeight - e.clientHeight) > 10);
      const prevTop = el?.scrollTop;
      el?.scrollBy(0, window.innerHeight * ${pct});
      if (!el || el?.scrollTop === prevTop) {
        window.scrollBy(0, window.innerHeight * ${pct});
      }
    })()`,
  });
}

async function zoomPage(addFactor) {
  const { id } = await currentTab();
  const factor = await browser.tabs.getZoom(id);
  let newFactor = factor + addFactor;
  if (newFactor < 0.3) newFactor = 0.3;
  if (newFactor > 5) newFactor = 5;
  await browser.tabs.setZoom(id, newFactor);
}

async function goForward() {
  const { id } = await currentTab();
  await browser.tabs.goForward(id);
}

async function goBack() {
  const { id } = await currentTab();
  await browser.tabs.goBack(id);
}

async function closeCurrentTab() {
  const { id } = await currentTab();
  await browser.tabs.remove(id);
}

async function reopenClosedTab() {
  const sessions = await browser.sessions.getRecentlyClosed();
  const lastSession = sessions
    .find((session) => session.tab && session.tab.windowId === windowId);
  if (!lastSession) return;
  await browser.sessions.restore(lastSession.tab.sessionId);
  await browser.tabs.update(lastSession.tab.id, { active: true });
  return new URL(lastSession.tab.url).host;
}

async function reloadCurrentTab() {
  const { id } = await currentTab();
  await browser.tabs.reload(id);
}

async function resizeViewport(width, height) {
  width = Math.round(width);
  height = Math.round(height);
  const window = await browser.windows.get(windowId);
  const tab = await currentTab();
  if (tab.width === width && tab.height === height) return;
  if (tab.width > width || tab.height > height) {
    await browser.windows.update(window.id, {
      width: width,
      height: height,
    });
  }
  const offsetWidth = window.width - tab.width;
  const offsetHeight = window.height - tab.height;
  await browser.windows.update(window.id, {
    width: width + offsetWidth,
    height: height + offsetHeight,
  });
  await new Promise((resolve) => setTimeout(resolve, 100));
}

async function openLinkUnderTap(pctX, pctY) {
  const { id } = await currentTab();
  const [url] = await browser.tabs.executeScript(id, {
    code:
      `[...document.elementsFromPoint(window.innerWidth * ${pctX}, window.innerHeight * ${pctY})]
       .find((e) => !!e.href)
       ?.href`,
  });
  if (!url) return;
  await browser.tabs.create({ url });
  await browser.tabs.update(id, { active: true });
  return new URL(url).host;
}

async function clickUnderTap(pctX, pctY) {
  const { id } = await currentTab();
  await browser.tabs.executeScript(id, {
    code:
      `document.elementFromPoint(window.innerWidth * ${pctX}, window.innerHeight * ${pctY})?.click()`,
  });
}

async function offsetContrastFilter(offset) {
  const { id } = await currentTab();
  const [newContrast] = await browser.tabs.executeScript(id, {
    code: `(() => {
      const el = document.documentElement;
      const match = el.style.filter?.match(/contrast\\((\\d+)%\\)/);
      const contrast = match ? parseInt(match[1]) : 100;
      const invert = el.style.filter?.includes("invert");
      const offsetContrast = contrast + ${offset};
      if (offsetContrast < 100) {
        el.style.filter = \`\${invert ? "" : "grayscale() invert() "}contrast(100%)\`;
      } else {
        el.style.filter = \`\${
          (offsetContrast === 100) && !invert ? "" : "grayscale() "}\${
          invert ? "invert() " : ""
        }contrast(\${offsetContrast}%)\`;
      }
      return el.style.filter.match(/contrast\\((\\d+)%\\)/)[1];
    })()`,
  });
  return newContrast;
}
// #endregion

// #region main loop

let scaleFactor = 1;
let deviceWidth = 0;
let deviceHeight = 0;
let mq;

import {
  ColorSpace,
  DitherMethod,
  ImageMagick,
  initializeImageMagick,
  MagickFormat,
  QuantizeSettings,
  MagickGeometry
} from "https://esm.sh/@imagemagick/magick-wasm@0.0.28";
import { Foras, Memory, zlib } from "https://esm.sh/@hazae41/foras@2.1.4";
import mqtt from "https://esm.sh/mqtt@5.4.0";
const connectMq = mqtt.connect;

const wasmFile =
  "https://esm.sh/@imagemagick/magick-wasm@0.0.28/dist/magick.wasm";
await fetch(wasmFile, { cache: "force-cache" })
  .then(a => a.arrayBuffer())
  .then(a => initializeImageMagick(a));
await Foras.initBundledOnce();

let topic = "remote-display-webext";
function send(msg) {
  if (mq?.connected) {
    mq.publish(`${topic}/device`, JSON.stringify(msg));
  }
}

function sendNotice(notice) {
  send({ type: "notify", value: notice });
}

async function sendImage() {
  if (!mq?.connected) return;
  if (!deviceWidth || !deviceHeight) {
    send({ type: "updateSize" });
    console.log("cannot update image without size");
    return;
  }
  console.log("capturing");
  const { id } = await currentTab();
  const dataUrl = await browser.tabs.captureTab(id, {
    scale: scaleFactor,
  });
  const buf = await fetch(dataUrl)
    .then(a => a.arrayBuffer())
    .then(a => new Uint8Array(a));

  ImageMagick.read(buf, (img) => {
    const mg = new MagickGeometry(deviceWidth, deviceHeight);
    mg.ignoreAspectRatio = true;
    const qs = new QuantizeSettings();
    qs.colors = 2;
    qs.colorSpace = ColorSpace.Gray;
    qs.ditherMethod = DitherMethod.FloydSteinberg;
    img.resize(mg);
    img.quantize(qs);
    img.write(MagickFormat.Pnm, (data) => {
      mq.publish(`${topic}/device`, zlib(new Memory(data)).bytes);
    });
  });

  await new Promise((resolve) => {
    const updated = (_, m) => {
      const msg = JSON.parse(m.toString());
      mq.off("message", updated);
      if (msg.type === "displayUpdated") {
        resolve();
      }
    }
    mq.on("message", updated);
  });
}

let timeout;
browser.tabs.onUpdated.addListener(async (_id, changeInfo, tab) => {
  if (!tab.active) return;
  if (changeInfo.status !== "complete") return;
  const myTab = await currentTab();
  if (tab.id !== myTab.id) return;
  if (timeout) {
    clearTimeout(timeout);
  }
  timeout = setTimeout(async () => {
    console.log("updating from tab load", changeInfo, tab);
    await sendImage();
    const info = await currentTabInfo();
    sendNotice(info);
  }, 1000);
});

async function onMessage(msg) {
  console.log("message", msg);
  switch (msg.type) {
    case "size": {
      const { width, height } = msg.value;
      deviceWidth = width;
      deviceHeight = height;
      await sendImage();
      break;
    }
    case "swipe": {
      const { dir, start, end } = msg.value;
      switch (dir) {
        case "north":
        case "south": {
          const dy = end.y - start.y;
          await scroll(
            start.x / deviceWidth,
            start.y / deviceHeight,
            -(dy / deviceHeight),
          );
          await sendImage();
          break;
        }
        case "east":
        case "west": {
          const offset = dir === "east" ? -1 : 1;
          const dx = end.x - start.x;
          // change windows if swipe covers more than half the screen
          await (Math.abs(dx) > deviceHeight / 2
            ? windowOffset(offset)
            : tabOffset(offset));
          await sendImage();
          const info = await currentTabInfo();
          sendNotice(info);
          break;
        }
      }
      break;
    }
    case "button": {
      const { button, status } = msg.value;
      if (!["released", "repeated"].includes(status)) break;
      // scroll half pages
      switch (button) {
        case "forward": {
          await scroll(0.5, 0.5, 0.5);
          break;
        }
        case "backward": {
          await scroll(0.5, 0.5, -0.5);
          break;
        }
      }
      if (status === "released") await sendImage();
      break;
    }
    case "arrow": {
      const { dir } = msg.value;
      switch (dir) {
        case "east":
          await goForward();
          break;
        case "west":
          await goBack();
          break;
        case "north": {
          const info = await currentTabInfo();
          sendNotice(`closing ${info}`);
          await closeCurrentTab();
          await sendImage();
          const newInfo = await currentTabInfo();
          sendNotice(newInfo);
          break;
        }
      }
      break;
    }
    case "pinch":
      await zoomPage(-0.1);
      await sendImage();
      break;
    case "spread":
      await zoomPage(0.1);
      await sendImage();
      break;
    case "rotate":
      if (Math.abs(msg.value.angle) < 20) break;
      if (msg.value.angle > 0) {
        const restoredTab = await reopenClosedTab();
        if (restoredTab) sendNotice(`restored ${restoredTab}`);
        else sendNotice("no tab restored");
        await sendImage();
      } else {
        await reloadCurrentTab();
      }
      break;
    case "holdFingerShort":
      await resizeViewport(deviceWidth / scaleFactor, deviceHeight / scaleFactor);
      await sendImage();
      send({ type: "refreshDisplay" });
      break;
    case "holdFingerLong": {
      const [{ x, y }] = msg.value;
      const tab = await openLinkUnderTap(x / deviceWidth, y / deviceHeight);
      if (tab) sendNotice(`${tab} opened`);
      else sendNotice("no link under finger");
      break;
    }
    case "tap": {
      const { x, y } = msg.value;
      await clickUnderTap(x / deviceWidth, y / deviceHeight);
      await sendImage();
      break;
    }
    case "corner": {
      const { dir } = msg.value;
      switch (dir) {
        case "southWest":
        case "southEast":
          {
            const newContrast = await offsetContrastFilter(
              dir === "southWest" ? -25 : 25,
            );
            sendNotice(`contrast ${newContrast}%`);
            await sendImage();
          }
          break;
        case "northWest":
        case "northEast": {
          const newScaleFactor = scaleFactor + (dir === "northWest" ? -0.1 : 0.1);
          if (newScaleFactor < 0.1 || newScaleFactor > 2) break;
          scaleFactor = newScaleFactor;
          sendNotice(`scale ${Math.round(scaleFactor * 100)}%`);
          await resizeViewport(deviceWidth / scaleFactor, deviceHeight / scaleFactor);
          await sendImage();
        }
      }
      break;
    }
  }
}

const defaultConfig = {
  wsUrl: "wss://broker.hivemq.com:8884/mqtt",
  topic: "remote-display-webext",
  enabled: false,
};

async function getConfig() {
  const result = await browser.storage.local.get(["wsUrl", "topic", "enabled"]);
  topic = result.topic || defaultConfig.topic;
  await browser.storage.local.set({ ...defaultConfig, ...result });
  return { ...defaultConfig, ...result };
}


function refreshConnection(config) {
  if (config.enabled && !mq?.connected) {
    mq = connectMq(config.wsUrl);
    mq.subscribe(`${config.topic}/browser`);
    mq.on("message", (_topic, message) => onMessage(JSON.parse(message.toString())));
    mq.on("connect", () => {
      console.log("connected");
    });
    mq.on("disconnect", () => {
      console.log("disconnected");
    });
  } else if (!config.enabled && mq?.connected) {
    mq.end();
  }
}

getConfig().then(refreshConnection).catch(console.error);

browser.storage.local.onChanged.addListener((changes) => {
  if (
    (
      ("wsUrl" in changes && changes.wsUrl.newValue !== changes.wsUrl.oldValue)
      || ("topic" in changes && changes.topic.newValue !== changes.topic.oldValue)
    ) &&
    mq?.connected
  ) {
    mq.end();
    getConfig().then(refreshConnection).catch(console.error);
  }
  if ("enabled" in changes && changes.enabled.newValue !== changes.enabled.oldValue) {
    getConfig().then(refreshConnection).catch(console.error);
  }
});

// #endregion
