// 布局体检：连无头浏览器的 CDP，量各区域实际像素并截图。
//
// 用法：
//   1) npm run dev                       （预览服务 http://localhost:1420）
//   2) msedge.exe --headless=new --disable-gpu --remote-debugging-port=9223 \
//        --user-data-dir=<临时目录> --window-size=1180,760 http://localhost:1420/
//   3) node scripts/layout-shot.mjs [输出目录]
//
// 只量布局（宽高、是否溢出），不碰文件、不调引擎；深色/浅色各截一张，
// 再切一次「收起三块」截一张，用来确认收起后画布真的变大。
import fs from 'node:fs';

const OUT = process.argv[2] ?? 'tests/out';
const PORT = process.env.CDP_PORT ?? '9223';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function page() {
  for (let i = 0; i < 20; i += 1) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const p = list.find((t) => t.type === 'page' && t.url.includes('1420'));
      if (p) return p;
    } catch { /* 端口尚未就绪 */ }
    await sleep(300);
  }
  throw new Error('CDP 页面未就绪');
}

const p = await page();
const ws = new WebSocket(p.webSocketDebuggerUrl);
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });

let id = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  }
};
function send(method, params = {}) {
  id += 1;
  const mid = id;
  ws.send(JSON.stringify({ id: mid, method, params }));
  return new Promise((res) => pending.set(mid, res));
}

async function evaluate(expr) {
  const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.result?.exceptionDetails) throw new Error(r.result.exceptionDetails.text);
  return r.result?.result?.value;
}

// 列宽有 180ms 过渡，量之前先等它落定，否则读到的是过渡中的中间值
async function settled() { await sleep(320); }

async function shot(name) {
  await sleep(260);
  const r = await send('Page.captureScreenshot', { format: 'png' });
  const file = `${OUT}/${name}.png`;
  fs.writeFileSync(file, Buffer.from(r.result.data, 'base64'));
  console.log('shot', file);
  return file;
}

const metrics = `(() => {
  const box = (sel) => {
    const el = document.querySelector(sel);
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { w: Math.round(r.width), h: Math.round(r.height) };
  };
  const body = document.querySelector('.app-body');
  return {
    viewport: { w: innerWidth, h: innerHeight },
    body: box('.app-body'),
    sidebar: box('.sidebar'),
    workspace: box('.workspace'),
    viewer: box('.viewer'),
    canvas: box('.viewer-canvas'),
    dock: box('.dock'),
    params: box('.params'),
    columns: body ? getComputedStyle(body).gridTemplateColumns : null,
    dockVisible: Boolean(document.querySelector('.dock')),
    scroll: {
      workspace: (() => { const el = document.querySelector('.workspace'); return el ? [el.scrollHeight, el.clientHeight] : null; })(),
      app: [document.documentElement.scrollHeight, document.documentElement.clientHeight],
      dockScroll: (() => { const el = document.querySelector('.dock'); return el ? [el.scrollHeight, el.clientHeight] : null; })(),
    },
  };
})()`;

await send('Page.enable');
await send('Runtime.enable');

// 每次都以干净状态量：清掉记住的布局偏好并重载，否则上一轮的开关会影响本轮结论
if (process.env.RESET !== '0') {
  await evaluate("localStorage.removeItem('payoncepdf-layout')");
  await send('Page.reload', { ignoreCache: true });
  await sleep(1200);
}

if (process.env.VIEWPORT) {
  const [w, h] = process.env.VIEWPORT.split(',').map(Number);
  await send('Emulation.setDeviceMetricsOverride', {
    width: w, height: h, deviceScaleFactor: 1, mobile: false,
  });
  await sleep(300);
}

const classes = "document.querySelector('.app-body')?.className ?? '(none)'";
console.log('初始 class', await evaluate(classes));

await settled();
console.log('默认', await evaluate(classes));
console.log('默认', JSON.stringify(await evaluate(metrics), null, 1));
await shot('layout-1-default');

// 收起左栏
await evaluate(`document.querySelectorAll('.layout-toggle')[0].click()`);
await settled();
console.log('收起左栏', await evaluate(classes));
console.log('收起左栏', JSON.stringify(await evaluate(metrics), null, 1));
await shot('layout-2-left-hidden');

// 收起右栏
await evaluate(`document.querySelectorAll('.layout-toggle')[1].click()`);
await settled();
console.log('左右都收起', await evaluate(classes));
console.log('左右都收起', JSON.stringify(await evaluate(metrics), null, 1));
await shot('layout-3-both-hidden');

// 再展开回来，确认没有塌陷
await evaluate(`document.querySelectorAll('.layout-toggle')[0].click(); document.querySelectorAll('.layout-toggle')[1].click()`);
await shot('layout-4-restored');

// 模拟丢入一个文件：文件区应自动让位（默认收起），画布取回高度
await evaluate(`(() => {
  const dt = new DataTransfer();
  dt.items.add(new File([new Uint8Array(8)], 'text_symbols.pdf', { type: 'application/pdf' }));
  document.querySelector('.dropzone')?.dispatchEvent(new DragEvent('drop', { bubbles: true, dataTransfer: dt }));
  return true;
})()`);
await settled();
console.log('加入文件后', await evaluate(classes));
console.log('加入文件后', JSON.stringify(await evaluate(metrics), null, 1));
await shot('layout-5-with-file');

// 浅色主题下再看一眼，确认新加的开关/拖动条没有硬编码深色
await evaluate("document.querySelector('.theme-toggle')?.click(); true");
await sleep(320);
await shot('layout-6-light');

ws.close();
