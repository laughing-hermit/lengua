import { app, BrowserWindow } from "electron";
import { writeFileSync } from "node:fs";

const address = process.env.LENGUA_URL ?? "";
const smoke = process.env.LENGUA_SMOKE ?? "";
const smokeBody = process.env.LENGUA_SMOKE_BODY ?? "";
const smokeOut = process.env.LENGUA_SMOKE_OUT ?? "";

function request() {
  const [method, path] = smoke.split(" ");
  const init = { method };
  if (smokeBody !== "") {
    init.headers = { "content-type": "application/json" };
    init.body = smokeBody;
  }
  return `fetch(${JSON.stringify(path)}, ${JSON.stringify(init)}).then(async (r) => ({ status: r.status, body: await r.text() }))`;
}

async function probe(window) {
  const title = window.webContents.getTitle();
  const answer = await window.webContents.executeJavaScript(request());
  writeFileSync(smokeOut, JSON.stringify({ title, status: answer.status, body: answer.body }));
}

function failed(error) {
  if (smokeOut !== "") {
    writeFileSync(smokeOut, JSON.stringify({ error: String(error) }));
  }
  app.exit(1);
}

function open() {
  const window = new BrowserWindow({
    width: 1100,
    height: 760,
    show: smoke === "",
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  window.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  window.webContents.on("will-navigate", (event, url) => {
    if (!url.startsWith(address)) {
      event.preventDefault();
    }
  });
  window.webContents.once("did-finish-load", () => {
    if (smoke !== "") {
      probe(window).then(() => app.quit(), failed);
    }
  });
  window.loadURL(address).catch(failed);
}

if (address === "") {
  failed(new Error("LENGUA_URL is not set: start the app with `lengua-run --desktop <folder>`"));
} else {
  app.whenReady().then(open, failed);
  app.on("window-all-closed", () => app.quit());
}
