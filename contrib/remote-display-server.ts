import { Application, Router } from "https://deno.land/x/oak@v13.0.0/mod.ts";

const app = new Application();
const router = new Router();

let deviceSocket: WebSocket | undefined;
let browserSocket: WebSocket | undefined;
let deviceWidth = 0;
let deviceHeight = 0;

// #region browser socket
router.get("/browser", (ctx) => {
  if (browserSocket) {
    browserSocket.close();
    console.log("Closing previous browser connection");
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

  browserSocket.onmessage = (m) => {
    if (m.data instanceof ArrayBuffer || m.data instanceof Blob) {
      console.log("Browser sends image");
    } else {
      console.log(`Browser sends ${m.data}`);
    }
    try {
      deviceSocket?.send(m.data);
    } catch (e) {
      console.error("Error sending to device: ", e);
      deviceSocket = undefined;
    }
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
    console.log(`Device sends ${m.data}`);
    const msg = JSON.parse(m.data);
    switch (msg.type) {
      case "size":
        deviceWidth = msg.value.width;
        deviceHeight = msg.value.height;
        /* falls through */
      default:
        try {
          browserSocket?.send(m.data);
        } catch (e) {
          console.error("Error sending to browser: ", e);
          browserSocket = undefined;
        }
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
      console.error(`Unauthorized access from ${ctx.request.ip}`);
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
