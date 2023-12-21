import { Application, Router } from "https://deno.land/x/oak@v12.6.0/mod.ts";
import { Foras, zlib } from "https://deno.land/x/foras@2.0.8/src/deno/mod.ts";
import $ from "https://deno.land/x/dax@0.34.0/mod.ts";

await Foras.initBundledOnce();

// #region support functions
const bytes = Intl.NumberFormat("en", {
  notation: "compact",
  style: "unit",
  unit: "byte",
  unitDisplay: "narrow",
});

/** Use ImageMagick `convert` to create a deflated PBM. */
async function convertToBlob(
  data: Uint8Array,
  width: number,
  height: number,
): Promise<Blob> {
  const start = performance.now();
  const image =
    await $`convert - -resize ${width}x${height}! -dither FloydSteinberg -remap pattern:gray50 pnm:-`
      .stdin(data)
      .bytes();
  const compressed = zlib(image);
  console.log(
    `converted ${bytes.format(data.byteLength)} frame to ${
      bytes.format(compressed.byteLength)
    } (${bytes.format(image.byteLength)} inflated) in ${
      Math.round(performance.now() - start)
    }ms`,
  );
  return new Blob([compressed], { type: "application/x-deflate" });
}
// #endregion

const app = new Application();
const router = new Router();

let deviceSocket: WebSocket | undefined;
let browserSocket: WebSocket | undefined;
let deviceWidth = 0;
let deviceHeight = 0;

// #region browser socket
router.get("/browser", (ctx) => {
  if (browserSocket) {
    ctx.response.status = 400;
    ctx.response.body = "Only one browser connection allowed";
    return;
  }
  browserSocket = ctx.upgrade();
  console.log("Browser starting connection");

  browserSocket.onopen = () => {
    console.log("Browser connected");
    if (!deviceSocket || !deviceWidth || !deviceHeight) return;
    browserSocket?.send(
      JSON.stringify({
        type: "size",
        value: { width: deviceWidth, height: deviceHeight },
      }),
    );
  };

  browserSocket.onclose = () => {
    browserSocket = undefined;
    console.log("Browser disconnected");
  };

  browserSocket.onmessage = async (m) => {
    if (m.data instanceof ArrayBuffer || m.data instanceof Blob) {
      console.log("Browser sends image");
      if (!deviceSocket) return;
      const data = m.data instanceof ArrayBuffer
        ? m.data
        : await m.data.arrayBuffer();
      convertToBlob(new Uint8Array(data), deviceWidth, deviceHeight)
        .then((blob) => deviceSocket?.send(blob));
      return;
    }
    console.log("Browser sends: ", m.data);
    deviceSocket?.send(m.data);
  };
});
// #endregion

// #region device socket
router.get("/device", (ctx) => {
  if (deviceSocket) {
    ctx.response.status = 400;
    ctx.response.body = "Only one device connection allowed";
    return;
  }
  deviceSocket = ctx.upgrade();
  console.log("Device starting connection");

  deviceSocket.onopen = () => {
    console.log("Device connected");
  };

  deviceSocket.onclose = () => {
    deviceSocket = undefined;
    console.log("Device disconnected");
  };

  deviceSocket.onmessage = (m) => {
    if (m.data instanceof ArrayBuffer) return;
    console.log("Device sends: ", m.data);
    const msg = JSON.parse(m.data);
    switch (msg.type) {
      case "size":
        deviceWidth = msg.value.width;
        deviceHeight = msg.value.height;
        /* falls through */
      default:
        browserSocket?.send(m.data);
    }
  };
});
// #endregion

const password = Deno.env.get("REMOTE_DISPLAY_PASSWORD");
if (password) {
  app.use(async (ctx, next) => {
    const auth = ctx.request.headers.get("authorization");
    const expectedAuth = `Basic ${btoa(`plato:${password}`)}`;
    if (auth !== expectedAuth) {
      console.log(`Unauthorized access from ${ctx.request.ip}`);
      ctx.response.status = 401;
      ctx.response.headers.set("WWW-Authenticate", "Basic");
      ctx.response.body = "Unauthorized";
      return;
    }
    await next();
  });
  console.log("Password protected with username 'plato'");
}

app.use(router.routes());
app.use(router.allowedMethods());

const port = parseInt(Deno.env.get("PORT") || "8222");
console.log("Listening at port " + port);
await app.listen({ port });
