// 真实 Tauri 窗口内的 F17/F18 引擎验证 + 界面截图。
// 目的：证明 release 二进制在真实 WebView2 窗口里能渲染 PDF、能取到字符级布局，
// 而不是只在单元测试/浏览器预览里通过。
// 用法：node tests/real-window-engine-check.mjs
import { spawn } from "node:child_process";
import { writeFileSync, existsSync, mkdirSync } from "node:fs";
import path from "node:path";

const EXE = "E:/PayOncePDF/src-tauri/target/release/payoncepdf.exe";
const PORT = 9333;
const FILE = "E:/PayOncePDF/tests/fixtures/text_cjk_special.pdf";
const OUT_DIR = "E:/PayOncePDF/tests/out";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function log(...a) {
  console.log(...a);
}

async function waitTarget(port, timeoutMs = 25000) {
  const end = Date.now() + timeoutMs;
  let lastErr = "";
  while (Date.now() < end) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/json/list`);
      if (r.ok) {
        const list = await r.json();
        const page = list.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
        if (page) return page;
      }
    } catch (e) {
      lastErr = String(e?.message ?? e);
    }
    await sleep(300);
  }
  throw new Error(`CDP target not found on ${port}. last=${lastErr}`);
}

class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.seq = 0;
    this.pending = new Map();
    ws.addEventListener("message", (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) reject(new Error(JSON.stringify(msg.error)));
        else resolve(msg.result);
      }
    });
  }
  send(method, params = {}) {
    const id = ++this.seq;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  async eval(expression, awaitPromise = true) {
    const r = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise,
      returnByValue: true,
      allowUnsafeEvalBlockedByCSP: true,
    });
    if (r.exceptionDetails) {
      throw new Error("EVAL_EXCEPTION " + JSON.stringify(r.exceptionDetails).slice(0, 600));
    }
    return r.result?.value;
  }
}

async function main() {
  if (!existsSync(EXE)) throw new Error("release exe missing: " + EXE);
  if (!existsSync(FILE)) throw new Error("fixture missing: " + FILE);
  if (!existsSync(OUT_DIR)) mkdirSync(OUT_DIR, { recursive: true });

  const env = {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`,
  };
  log("[1] spawn", EXE);
  const child = spawn(EXE, [], {
    cwd: path.dirname(EXE),
    env,
    stdio: "ignore",
    windowsHide: false,
  });
  child.on("exit", (c, s) => log(`    child exit code=${c} signal=${s}`));

  let ws;
  try {
    const target = await waitTarget(PORT);
    log("[2] CDP target:", target.title || "(no title)", target.url?.slice(0, 80));
    ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((res, rej) => {
      ws.addEventListener("open", res, { once: true });
      ws.addEventListener("error", rej, { once: true });
    });
    const cdp = new Cdp(ws);
    await cdp.send("Runtime.enable");
    await cdp.send("Page.enable");

    // 等 React 挂载完成
    let ready = false;
    for (let i = 0; i < 40; i++) {
      const has = await cdp.eval(`Boolean(window.__TAURI_INTERNALS__)`, false);
      if (has) {
        ready = true;
        break;
      }
      await sleep(250);
    }
    log("[3] __TAURI_INTERNALS__ =", ready);
    if (!ready) throw new Error("Tauri internals not exposed to page");

    const probe = await cdp.eval(`(async () => {
      const I = window.__TAURI_INTERNALS__;
      const call = (cmd, payload) => I.invoke(cmd, { payload });
      const file = ${JSON.stringify(FILE)};
      const t0 = performance.now();
      const info = await call('get_pdf_info', { file });
      const layout = await call('get_page_text_layout', { file, page: 1 });
      const img = await call('render_page_preview', { file, page: 1, dpi: 110 });
      return {
        ms: Math.round(performance.now() - t0),
        info,
        layout: {
          chars: (layout.chars || []).length,
          textHead: (layout.text || '').slice(0, 100),
          hasTextLayer: layout.hasTextLayer,
          rotation: layout.rotation,
          w: layout.pageWidthPt,
          h: layout.pageHeightPt,
          first2: (layout.chars || []).slice(0, 2),
        },
        img: { w: img.width, h: img.height, page: img.page, pages: img.pages, b64len: img.pngBase64.length },
      };
    })()`);
    log("[4] 真实引擎探测:");
    log(JSON.stringify(probe, null, 2));

    // 取回真实渲染 PNG 并落盘（这是 PDFium 在真实窗口里的输出，不是 CSS 示意纸）
    const b64 = await cdp.eval(`(async () => {
      const I = window.__TAURI_INTERNALS__;
      const r = await I.invoke('render_page_preview', { payload: { file: ${JSON.stringify(FILE)}, page: 1, dpi: 110 } });
      return r.pngBase64;
    })()`);
    const pngPath = path.join(OUT_DIR, "real-f17-render.png");
    writeFileSync(pngPath, Buffer.from(b64, "base64"));
    log("[5] 真实渲染落盘:", pngPath, `${Buffer.from(b64, "base64").length} bytes`);

    // 尝试用事件通道把文件送进真实 UI（Tauri drag-drop 事件）
    const dropped = await cdp.eval(`(async () => {
      const I = window.__TAURI_INTERNALS__;
      try {
        await I.invoke('plugin:event|emit', { event: 'tauri://drag-drop', payload: { type: 'drop', paths: [${JSON.stringify(FILE)}], position: { x: 620, y: 420 } } });
        return 'emitted';
      } catch (e) { return 'emit_failed: ' + String(e); }
    })()`);
    log("[6] drag-drop 注入:", dropped);
    await sleep(1800);

    const afterDrop = await cdp.eval(`(() => {
      const img = document.querySelector('.viewer-page-wrap img');
      const status = document.querySelector('.viewer-status, .viewer-meta, .viewer-toolbar')?.innerText || '';
      const body = document.body.innerText || '';
      return {
        hasCanvasImg: Boolean(img),
        imgNatural: img ? [img.naturalWidth, img.naturalHeight] : null,
        statusHead: status.slice(0, 160),
        mentionsSelectable: body.includes('桌面版可用'),
        mentionsUnavailable: body.includes('框选不可用'),
      };
    })()`, false);
    log("[7] 注入后界面状态:", JSON.stringify(afterDrop, null, 2));

    const shot = await cdp.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    const shotPath = path.join(OUT_DIR, "real-window-loaded.png");
    writeFileSync(shotPath, Buffer.from(shot.data, "base64"));
    log("[8] 界面截图:", shotPath, `${Buffer.from(shot.data, "base64").length} bytes`);
  } finally {
    try { ws?.close(); } catch {}
    await sleep(300);
    try { child.kill(); } catch {}
    await sleep(800);
    log("[9] done");
  }
}

main().catch((e) => {
  console.error("FAILED:", e?.message ?? e);
  process.exitCode = 1;
});
