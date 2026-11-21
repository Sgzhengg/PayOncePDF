// 工作台 v2 外观取证：浅色 / 暗色两张截图（浏览器预览模式，不是真机 Tauri 窗口）。
// 用法：先起 dev server + CDP 9222，再 node tests/capture-ui-v2.mjs
import { writeFileSync } from 'node:fs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
const page = list.find((t) => t.type === 'page' && t.url.includes('1420'));
if (!page) throw new Error('CDP 页面未就绪');

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let seq = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
};
const send = (method, params = {}) => new Promise((resolve) => {
  const id = ++seq;
  pending.set(id, resolve);
  ws.send(JSON.stringify({ id, method, params }));
});

await send('Page.enable');
await send('Page.enable');
await send('Page.bringToFront'); // 后台标签的 captureScreenshot 会挂起，必须先激活 1420 标签
await send('Emulation.setDeviceMetricsOverride', {
  width: 1440, height: 900, deviceScaleFactor: 1, mobile: false,
});
await send('Page.reload', { ignoreCache: true });
await sleep(1100);

async function shot(theme, out) {
  await send('Runtime.evaluate', {
    expression: `document.documentElement.dataset.theme='${theme}'; document.body.offsetHeight;`,
  });
  await sleep(400);
  const r = await Promise.race([
    send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false }),
    sleep(20000).then(() => { throw new Error('captureScreenshot timeout'); }),
  ]);
  if (!r.result?.data) throw new Error(`截图失败: ${JSON.stringify(r).slice(0, 200)}`);
  const buf = Buffer.from(r.result.data, 'base64');
  writeFileSync(out, buf);
  console.log(`OK  ${out}  ${buf.length} B  theme=${theme}`);
}

await shot('light', 'tests/out/updf-ui-light.png');
await shot('dark', 'tests/out/updf-ui-dark.png');
ws.close();
process.exit(0);
