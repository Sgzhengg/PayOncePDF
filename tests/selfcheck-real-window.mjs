// 真实 Tauri 窗口里跑一次 F21 应用内自检（无人值守 + 可选 UI 面板验证）。
//
// 用法：node tests/selfcheck-real-window.mjs
// 原理：带 PAYONCEPDF_SELFCHECK=1 启动桌面程序，后端在启动期自动跑完整自检并写报告；
//       前端 UI 面板验证需要 WebView2 CDP 调试端口，不可用时自动跳过（不影响引擎自检）。
// 可选环境变量：PAYONCE_EXE（默认 debug 版）、CDP_PORT（默认 9334）。
// 产物：tests/out/selfcheck-real-window.json、selfcheck-dialog.png（仅 CDP 可用时）
import { spawn } from "node:child_process";
import {
  writeFileSync,
  readFileSync,
  existsSync,
  mkdirSync,
  rmSync,
} from "node:fs";
import path from "node:path";

const EXE = process.env.PAYONCE_EXE ?? "E:/PayOncePDF/src-tauri/target/debug/payoncepdf.exe";
const PORT = Number(process.env.CDP_PORT ?? 9334);
const OUT_DIR = "E:/PayOncePDF/tests/out";
const REPORT = path.join(OUT_DIR, "selfcheck-real-window.json");
const DONE = `${REPORT}.done`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitTarget(timeoutMs = 5000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/json/list`);
      if (r.ok) {
        const page = (await r.json()).find((t) => t.type === "page" && t.webSocketDebuggerUrl);
        if (page) return page;
      }
    } catch {
      /* CDP 尚未就绪 */
    }
    await sleep(250);
  }
  return null;
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
      throw new Error("EVAL_EXCEPTION " + JSON.stringify(r.exceptionDetails).slice(0, 800));
    }
    return r.result?.value;
  }
}

async function verifyUi(cdp) {
  const opened = await cdp.eval(`(() => {
    const b = [...document.querySelectorAll('.sidebar-footer button')]
      .find((x) => x.textContent.trim() === '运行自检');
    if (!b) return 'no-sidebar-button';
    b.click();
    return 'clicked';
  })()`, false);
  await sleep(400);
  const hasDialog = await cdp.eval(
    `Boolean(document.querySelector('.dialog[aria-label="运行自检"]'))`,
    false,
  );
  const clicked = await cdp.eval(`(() => {
    const b = [...document.querySelectorAll('.dialog .btn-primary')]
      .find((x) => x.textContent.includes('运行自检'));
    if (!b) return 'no-dialog-button';
    b.click();
    return 'clicked';
  })()`, false);
  let rendered = false;
  for (let i = 0; i < 60; i++) {
    rendered = await cdp.eval(`Boolean(document.querySelector('.selfcheck-summary'))`, false);
    if (rendered) break;
    await sleep(300);
  }
  const summaryText = rendered
    ? await cdp.eval(`document.querySelector('.selfcheck-summary').innerText`, false)
    : "";
  console.log("[6] UI 面板:", opened, "dialog=", hasDialog, "run=", clicked, "rendered=", rendered);
  console.log("    ", summaryText.replace(/\n/g, " | "));
  const shot = await cdp.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  const shotPath = path.join(OUT_DIR, "selfcheck-dialog.png");
  writeFileSync(shotPath, Buffer.from(shot.data, "base64"));
  console.log("[7] 截图:", shotPath);
}

async function main() {
  if (!existsSync(EXE)) throw new Error("exe missing: " + EXE);
  if (!existsSync(OUT_DIR)) mkdirSync(OUT_DIR, { recursive: true });
  for (const f of [REPORT, DONE]) {
    if (existsSync(f)) rmSync(f);
  }

  const child = spawn(EXE, [], {
    cwd: path.dirname(EXE),
    env: {
      ...process.env,
      PAYONCEPDF_SELFCHECK: "1",
      PAYONCEPDF_SELFCHECK_OUT: REPORT,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`,
    },
    stdio: "ignore",
    windowsHide: false,
  });
  child.on("exit", (c, s) => console.log(`[exit] code=${c} signal=${s}`));

  let ws;
  try {
    // 1) 等启动期自检写完报告（真实 qpdf / lopdf / PDFium，在真实进程里）。
    const deadline = Date.now() + 90000;
    while (!existsSync(DONE) && Date.now() < deadline) await sleep(300);
    if (!existsSync(REPORT)) throw new Error("自检报告未生成（超时或进程启动失败）");
    const report = JSON.parse(readFileSync(REPORT, "utf8"));
    console.log("[1] 真实窗口自检完成:", report.schema, "path=", report.path || report.write_error);
    console.log("[2] summary:", JSON.stringify(report.summary), "elapsed_ms=", report.elapsed_ms);
    console.log(
      "[3] coverage:",
      (report.coverage?.self_checked ?? []).length,
      "features /",
      (report.coverage?.needs_real_device ?? []).length,
      "manual items",
    );
    console.log("[4] 失败与跳过项:");
    for (const c of (report.cases ?? []).filter((x) => !x.ok || x.skipped)) {
      console.log(`    ${c.ok ? "SKIP" : "FAIL"} ${c.case} · ${c.ms}ms · ${c.detail}`);
    }
    if ((report.cases ?? []).every((c) => c.ok && !c.skipped)) {
      console.log("    （全部通过，无失败/跳过）");
    }

    // 2) 可选：CDP 可用时验证前端面板确实能跑起来并渲染结果。
    const target = await waitTarget(5000);
    if (!target) {
      console.log("[6] UI 面板验证跳过：本机 WebView2 未开放 CDP 调试端口（引擎自检不受影响）");
    } else {
      ws = new WebSocket(target.webSocketDebuggerUrl);
      await new Promise((res, rej) => {
        ws.addEventListener("open", res, { once: true });
        ws.addEventListener("error", rej, { once: true });
      });
      const cdp = new Cdp(ws);
      await cdp.send("Runtime.enable");
      await cdp.send("Page.enable");
      await verifyUi(cdp);
    }

    console.log("[8] 报告:", REPORT);
    if (report.summary?.failed > 0) process.exitCode = 1;
  } finally {
    try { ws?.close(); } catch {}
    await sleep(200);
    try { child.kill(); } catch {}
    await sleep(500);
  }
}

main().catch((e) => {
  console.error("FAILED:", e?.stack ?? String(e));
  process.exitCode = 2;
});
