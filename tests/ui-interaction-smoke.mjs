// UI 交互冒烟：通过 Chrome DevTools Protocol 在浏览器预览模式下验证
// 「拖放 -> 文件列表 -> 开始处理 -> 进度事件 -> 完成提示 -> 面板切换 -> Esc 取消」链路。
//
// 用法：
//   1) npm run dev                       （预览服务 http://localhost:1420）
//   2) 启动带调试端口的浏览器（Windows 示例，user-data-dir 用独立临时目录）：
//      msedge.exe --headless=new --disable-gpu --remote-debugging-port=9222 \
//        --user-data-dir=<临时目录> http://localhost:1420
//   3) node tests/ui-interaction-smoke.mjs
//
// 每次运行请用全新浏览器实例（或先刷新页面），否则上一次的文件列表与 mock 任务会串扰。
// 仅覆盖浏览器预览模式（mock）；真实 Tauri 窗口的原生对话框与引擎调用需真机验收。
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const CDP_PORT = process.env.CDP_PORT ?? '9222';
async function getPage() {
  for (let i = 0; i < 20; i += 1) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json();
      const page = list.find((t) => t.type === 'page' && t.url.includes('1420'));
      if (page) return page;
    } catch { /* 端口尚未就绪 */ }
    await sleep(300);
  }
  throw new Error('CDP 页面未就绪');
}

const page = await getPage();
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = reject;
});
let seq = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  }
};
function send(method, params = {}) {
  return new Promise((resolve) => {
    const id = ++seq;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const r = await send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (r.result?.exceptionDetails) throw new Error(r.result.exceptionDetails.text);
  return r.result?.result?.value;
}

// 强制刷新，避免复用同一浏览器实例时上一次的文件列表 / 面板状态串扰。
// 2026-11-21：布局偏好存在 localStorage，上一轮点过「文件」展开后，下一轮 reload
// 仍是展开态，会让「有文件后自动收起」和后续「点开关展开看列表」两条假失败。
// 因此先清布局偏好（主题保留），再刷新，保证每轮都从默认态开始。
try {
  await evaluate("localStorage.removeItem('payoncepdf-layout'); true");
} catch {
  // 页面正在导航时 evaluate 可能失败，reload 后再试一次即可
}
await send('Page.reload', { ignoreCache: true });
await sleep(700);
let ready = false;
for (let i = 0; i < 40; i += 1) {
  ready = await evaluate(
    "Boolean(document.querySelector('.dropzone')) && (document.querySelector('.dropzone-count')?.textContent || '').includes('0 个文件')",
  );
  if (ready) break;
  await sleep(300);
}
if (!ready) throw new Error('页面在 12 秒内未就绪，后续断言不可信');

const results = [];
const check = (name, ok, detail) => {
  results.push(`${ok ? 'PASS' : 'FAIL'}  ${name} -- ${detail}`);
};

// 1. 首屏标语与预览横幅
const tagline = await evaluate("document.querySelector('.hero-line')?.textContent?.trim()");
check('首屏标语', tagline === 'Your files never leave this computer.', tagline);
const banner = await evaluate("document.querySelector('.preview-banner')?.textContent?.trim()");
check('预览模式横幅常驻', Boolean(banner && banner.includes('预览模式')), banner?.slice(0, 60));

// 2. 十五个功能入口（F10–F16 范围变更后）
const tools = await evaluate("Array.from(document.querySelectorAll('.tool-btn-label')).map(e => e.textContent.trim())");
check('十五个功能入口', tools.length === 15, tools.join(' / '));
const expectedNew = ['改文字', '批注', '页面', '水印', '导出', '文档对象', '文本转 PDF', '打印与扩展'];
check('新增八个对标入口齐备', expectedNew.every((t) => tools.includes(t)), expectedNew.map((t) => `${t}:${tools.includes(t)}`).join(' '));
const sidebarScroll = await evaluate("(() => { const e = document.querySelector('.tool-list'); return getComputedStyle(e).overflowY; })()");
check('侧栏可独立滚动（15 项不挤压工作区）', sidebarScroll === 'auto' || sidebarScroll === 'scroll', `overflow-y=${sidebarScroll}`);

// 2b. F18 框选条在浏览器预览模式必须「明确禁用」，不伪造文字坐标或交互
const selNote = await evaluate("document.querySelector('.viewer-selection-bar')?.textContent?.trim()");
check(
  'F18 预览模式框选条明确说明不调用本地引擎',
  Boolean(selNote && selNote.includes('预览模式不调用本地引擎')),
  selNote?.slice(0, 90),
);
const fakeHits = await evaluate("document.querySelectorAll('.viewer-hit, #viewer-sel-text, .viewer-marquee').length");
check('F18 预览模式不伪造框选高亮与输入', fakeHits === 0, `伪造元素=${fakeHits}`);
const selFoot = await evaluate("document.querySelector('.viewer-foot')?.textContent?.trim()");
check('F18 页脚标注框选不可用', Boolean(selFoot && selFoot.includes('框选不可用')), selFoot?.slice(0, 90));

// 3. 模拟拖放两个文件
await evaluate(`(() => {
  const dz = document.querySelector('.dropzone');
  const dt = new DataTransfer();
  dt.items.add(new File([new Uint8Array([37,80,68,70])], 'a.pdf', { type: 'application/pdf' }));
  dt.items.add(new File([new Uint8Array([37,80,68,70])], 'b.pdf', { type: 'application/pdf' }));
  dz.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt }));
  return true;
})()`);
await sleep(400);
// 2026-11-21 布局改版：有文件后底部「文件与导入」自动收起，把高度让给文档画布。
// 所以先确认它确实收起了，再点顶栏「文件」展开，之后才断言列表内容。
const dockAutoHidden = await evaluate("!document.querySelector('.dock')");
check('有文件后文件区自动收起', dockAutoHidden === true, `dock 存在=${!dockAutoHidden}`);
await evaluate("document.querySelector('[data-layout-toggle=\"dock\"]').click(); true");
await sleep(320);
const countText = await evaluate("document.querySelector('.dropzone-count')?.textContent?.trim()");
const rows = await evaluate("document.querySelectorAll('.filelist tbody tr, .filelist li').length");
check('拖放加入文件列表', countText?.includes('2') && rows >= 2, `${countText}; 列表行=${rows}`);

// 4. 开始处理 -> mock 进度 -> 完成
const runBtnEnabled = await evaluate("!document.querySelector('button[aria-label=\"开始处理并导出\"]').disabled");
check('有输入后导出按钮可用', runBtnEnabled === true, `disabled=${!runBtnEnabled}`);
await evaluate("document.querySelector('button[aria-label=\"开始处理并导出\"]').click(); true");
// 轮询而非固定 sleep：HMR / CPU 抢占会让 mock 计时器偶发推迟，固定等待会产生假阴性
// （2026-11-20 首次 F18 冒烟即因 0/0 误报一次）。
let midProgress = '';
let cancelEnabled = false;
for (let i = 0; i < 20; i += 1) {
  await sleep(100);
  midProgress = await evaluate("document.querySelector('.progress-count')?.textContent?.trim()");
  cancelEnabled = await evaluate("!Array.from(document.querySelectorAll('.action-row button')).find(b => b.textContent.includes('取消')).disabled");
  if (/[1-9]\s*\/\s*12/.test(midProgress || '') && cancelEnabled === true) break;
}
check('进度事件推进 / 取消按钮可用', /[1-9]\s*\/\s*12/.test(midProgress || '') && cancelEnabled === true, `进度=${midProgress}; 取消可用=${cancelEnabled}`);
await sleep(1800);
const statusText = await evaluate("document.querySelector('.status-text')?.textContent?.trim()");
const doneNote = await evaluate("document.querySelector('.actionbar .inline-note')?.textContent?.trim()");
const fakeOutputs = await evaluate("document.querySelectorAll('.output-list li').length");
check('任务完成且未伪造输出', Boolean(statusText?.includes('预览模式')) && fakeOutputs === 0, `状态=${statusText}; 提示=${doneNote ?? '无'}; 输出条数=${fakeOutputs}`);

// 5. 切换到拆分面板
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.tool-btn')).find(b => b.textContent.includes('拆分'));
  btn.click(); return true;
})()`);
await sleep(300);
const paramTitle = await evaluate("document.querySelector('.params-header h2')?.textContent?.trim()");
const splitBody = await evaluate("document.querySelector('.params-body')?.textContent?.trim().slice(0, 80)");
check('参数面板随功能切换', paramTitle === '拆分', `标题=${paramTitle}; 内容=${splitBody}`);

// 5b. 新增 F10 改文字面板：查找/替换/边界说明齐全
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.tool-btn')).find(b => b.textContent.includes('改文字'));
  btn.click(); return true;
})()`);
await sleep(300);
const editTitle = await evaluate("document.querySelector('.params-header h2')?.textContent?.trim()");
const editBody = await evaluate("document.querySelector('.params-body')?.textContent?.trim()");
check(
  'F10 就地改字面板与诚实边界',
  editTitle === '改文字' && Boolean(editBody?.includes('查找')) && Boolean(editBody?.includes('替换为')) && Boolean(editBody?.includes('不重排')),
  `标题=${editTitle}; 含查找/替换/不重排=${editBody?.includes('查找')}/${editBody?.includes('替换为')}/${editBody?.includes('不重排')}`,
);
// 5c. F14 导出面板明示「只导出文字层」
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.tool-btn')).find(b => b.textContent.includes('导出'));
  btn.click(); return true;
})()`);
await sleep(300);
const exportBody = await evaluate("document.querySelector('.params-body')?.textContent?.trim()");
check('F14 导出版式降级已明示', Boolean(exportBody?.includes('不还原版式')), `含不还原版式=${exportBody?.includes('不还原版式')}`);

// 5d. F15 文档对象（表单 / 脱敏 / 书签）
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.tool-btn')).find(b => b.textContent.includes('文档对象'));
  btn.click(); return true;
})()`);
await sleep(300);
const objectsBody = await evaluate("document.querySelector('.params-body')?.textContent?.trim()");
check(
  'F15 文档对象面板含脱敏与书签边界',
  Boolean(objectsBody?.includes('真删除文字对象')) && Boolean(objectsBody?.includes('书签管理')),
  `含真删除=${objectsBody?.includes('真删除文字对象')}; 含书签管理=${objectsBody?.includes('书签管理')}`,
);

// 5e. F16 文本转 PDF 明示「只做纯文本排版」
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.tool-btn')).find(b => b.textContent.includes('文本转 PDF'));
  btn.click(); return true;
})()`);
await sleep(300);
const createBody = await evaluate("document.querySelector('.params-body')?.textContent?.trim()");
check('F16 文本转 PDF 版式降级已明示', Boolean(createBody?.includes('只做纯文本排版')), `含纯文本排版=${createBody?.includes('只做纯文本排版')}`);

// 5f. F16 打印与扩展：语言包是唯一联网点
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.tool-btn')).find(b => b.textContent.includes('打印与扩展'));
  btn.click(); return true;
})()`);
await sleep(300);
const shareBody = await evaluate("document.querySelector('.params-body')?.textContent?.trim()");
check(
  'F16 语言包联网边界已明示',
  Boolean(shareBody?.includes('唯一')) && Boolean(shareBody?.includes('不含任何文件')),
  `含唯一=${shareBody?.includes('唯一')}; 含不含任何文件=${shareBody?.includes('不含任何文件')}`,
);
check(
  'F16 语言包清单为 42 种（与 sidecar.rs 一致）',
  Boolean(shareBody?.includes('42 种')),
  `含 42 种=${shareBody?.includes('42 种')}`,
);

// 5g. F13 中文水印：水印支持中文，页眉页脚仍限拉丁字符
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.tool-btn')).find(b => b.textContent.includes('水印'));
  btn.click(); return true;
})()`);
await sleep(300);
const decorateBody = await evaluate("document.querySelector('.params-body')?.textContent?.trim()");
check(
  'F13 中文水印与页眉页脚边界已区分',
  Boolean(decorateBody?.includes('水印文字支持中文')) && Boolean(decorateBody?.includes('页眉 / 页脚仍用 Helvetica/WinAnsi')),
  `含中文水印=${decorateBody?.includes('水印文字支持中文')}; 含页眉页脚边界=${decorateBody?.includes('页眉 / 页脚仍用 Helvetica/WinAnsi')}`,
);

// 6. 授权 / 关于对话框可打开
await evaluate(`(() => {
  const btn = Array.from(document.querySelectorAll('.footer-btn')).find(b => b.textContent.includes('授权'));
  btn.click(); return true;
})()`);
await sleep(300);
const dialogTitle = await evaluate("document.querySelector('.dialog h2, [role=dialog] h2')?.textContent?.trim()");
check('底部信息入口可打开对话框', Boolean(dialogTitle), `对话框标题=${dialogTitle ?? '未找到'}`);

// 7. Esc 关闭对话框；再次运行后用 Esc 取消任务
await evaluate("window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); true");
await sleep(250);
const dialogClosed = await evaluate("!document.querySelector('[role=dialog]')");
check('Esc 关闭对话框', dialogClosed === true, `对话框已关闭=${dialogClosed}`);
await evaluate("document.querySelector('button[aria-label=\"开始处理并导出\"]').click(); true");
await sleep(350);
await evaluate("window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); true");
await sleep(300);
const cancelledText = await evaluate("document.querySelector('.status-text')?.textContent?.trim()");
check('Esc 取消运行中任务', Boolean(cancelledText?.includes('已取消')), `状态=${cancelledText}`);

console.log(results.join('\n'));
const failed = results.filter((r) => r.startsWith('FAIL')).length;
console.log(`\nUI 交互冒烟: ${results.length - failed} PASS / ${failed} FAIL`);
ws.close();
process.exit(failed === 0 ? 0 : 1);
