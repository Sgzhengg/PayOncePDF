// 左右栏拖拽调宽实测：连无头浏览器 CDP，模拟 pointerdown/move/up，
// 量网格列宽是否真的跟着走，收起/双击恢复是否仍然正确。
//
// 用法：
//   1) npm run dev                       （http://localhost:1420）
//   2) msedge.exe --headless=new --remote-debugging-port=9223 \
//        --user-data-dir=<临时目录> --window-size=1180,760 http://localhost:1420/
//   3) node scripts/resize-check.mjs
//
// 断言全在这里打印 PASS/FAIL，退出码非 0 表示有失败。
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
  if (r.result?.exceptionDetails) {
    throw new Error(JSON.stringify(r.result.exceptionDetails));
  }
  return r.result?.result?.value;
}

let failed = 0;
function check(name, ok, detail = '') {
  console.log(`${ok ? 'PASS' : 'FAIL'} - ${name}${detail ? `（${detail}）` : ''}`);
  if (!ok) failed += 1;
}

// 左栏网格列宽（第 1 列）与右栏（第 3 列），四舍五入
const widths = `(() => {
  const cols = getComputedStyle(document.querySelector('.app-body')).gridTemplateColumns
    .split(/\\s+/).map((v) => Math.round(parseFloat(v)));
  return { left: cols[0], mid: cols[1], right: cols[2] };
})()`;

const pointerSeq = (sel, dx) => `(() => {
  const el = document.querySelector('${sel}');
  if (!el) return 'missing';
  const r = el.getBoundingClientRect();
  const x0 = r.left + r.width / 2, y = r.top + r.height / 2;
  const mk = (type, x, buttons) => new PointerEvent(type, {
    bubbles: true, cancelable: true, pointerId: 7, pointerType: 'mouse',
    isPrimary: true, button: type === 'pointermove' || type === 'pointerup' ? -1 : 0,
    buttons, clientX: x, clientY: y,
  });
  el.dispatchEvent(mk('pointerdown', x0, 1));
  el.dispatchEvent(mk('pointermove', x0 + ${dx}, 1));
  el.dispatchEvent(mk('pointerup', x0 + ${dx}, 0));
  return 'ok';
})()`;

const dblClick = (sel) => `(() => {
  const el = document.querySelector('${sel}');
  if (!el) return 'missing';
  el.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  return 'ok';
})()`;

await send('Page.enable');
await send('Runtime.enable');
await evaluate("localStorage.removeItem('payoncepdf-layout')");
await send('Page.reload', { ignoreCache: true });
await sleep(1200);

// 1180×760：默认列宽 214 / 336（未进窄窗口媒体查询）
await send('Emulation.setDeviceMetricsOverride', { width: 1180, height: 760, deviceScaleFactor: 1, mobile: false });
await sleep(400);

const w0 = await evaluate(widths);
check('手柄存在（左右各一）',
  (await evaluate(`document.querySelectorAll('.col-resizer').length`)) === 2);
check(`默认列宽 214/336`, w0.left === 214 && w0.right === 336, JSON.stringify(w0));

// 左栏向右拖 70px → 284
await evaluate(pointerSeq('.col-resizer-left', 70));
await sleep(360);
const w1 = await evaluate(widths);
check(`左栏拖宽 214→284`, w1.left === 284, JSON.stringify(w1));

// 持久化：localStorage 里应记下 284
const stored = JSON.parse(await evaluate(`localStorage.getItem('payoncepdf-layout') ?? '{}'`));
check(`宽度写入 localStorage`, stored.leftW === 284, JSON.stringify(stored));

// 收起左栏：列宽必须归 0（内联样式不能盖过收起规则）
await evaluate(`document.querySelectorAll('.layout-toggle')[0].click()`);
await sleep(360);
const w2 = await evaluate(widths);
check(`拖宽后收起仍归 0`, w2.left === 0, JSON.stringify(w2));
await evaluate(`document.querySelectorAll('.layout-toggle')[0].click()`);
await sleep(360);

// 左栏向左拖 60 → 224
await evaluate(pointerSeq('.col-resizer-left', -60));
await sleep(360);
const w3 = await evaluate(widths);
check(`左栏拖窄 284→224`, w3.left === 224, JSON.stringify(w3));

// 右栏：鼠标左移 60 → 336+60=396
await evaluate(pointerSeq('.col-resizer-right', -60));
await sleep(360);
const w4 = await evaluate(widths);
check(`右栏拖宽 336→396`, w4.right === 396, JSON.stringify(w4));

// 双击左栏手柄 → 恢复默认 214
await evaluate(dblClick('.col-resizer-left'));
await sleep(360);
const w5 = await evaluate(widths);
check(`双击恢复左栏默认 214`, w5.left === 214, JSON.stringify(w5));

// 双击右栏手柄 → 恢复默认 336
await evaluate(dblClick('.col-resizer-right'));
await sleep(360);
const w6 = await evaluate(widths);
check(`双击恢复右栏默认 336`, w6.right === 336, JSON.stringify(w6));

// 重载后恢复用户宽度（先把左栏拖到 300 再刷新）
await evaluate(pointerSeq('.col-resizer-left', 86));
await sleep(360);
await send('Page.reload', { ignoreCache: true });
await sleep(1200);
const w7 = await evaluate(widths);
check(`刷新后保持 300`, w7.left === 300, JSON.stringify(w7));

console.log(failed === 0 ? '全部通过' : `${failed} 条失败`);
ws.close();
process.exit(failed === 0 ? 0 : 1);
