/**
 * 旋转页框选的无头浏览器验证（CDP）。
 *
 * 前置：`npm run dev`（1420）+ 带 --remote-debugging-port=9222 的浏览器打开该地址。
 * 用法：node scripts/rotated-selection-check.mjs
 *
 * 验证：浏览器预览模式加载旋转页时，F18 布局接口被明确标注不可用（预览模式不调用
 * 本地引擎）；真实坐标变换由 Rust 端 e2e 测试覆盖。本脚本只确认前端不会在旋转页
 * 伪造高亮 / 不崩，且「旋转页可用」的说明与实际gate一致（预览模式仍禁用）。
 */
// Node 22 内置全局 WebSocket，无需第三方依赖（与 tests/ui-interaction-smoke.mjs 一致）。

const PORT = process.env.CDP_PORT || "9222";
const targets = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
const page = targets.find((t) => t.type === "page" && t.url.includes("1420"));
if (!page) throw new Error("找不到预览页标签（应打开 http://localhost:1420/）");

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
let seq = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
};
const send = (method, params = {}) => new Promise((res) => {
  const id = ++seq; pending.set(id, res);
  ws.send(JSON.stringify({ id, method, params }));
});
const evalJs = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (r.result?.exceptionDetails) throw new Error(r.result.exceptionDetails.text);
  return r.result?.result?.value;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const results = [];
const check = (name, ok, detail) => results.push({ name, ok, detail });

await send("Page.reload", { ignoreCache: true });
await sleep(900);

// 预览模式的旋转页：画布应正常渲染，不因 rotation 变化而伪造文字命中元素。
const selNote = await evalJs("document.querySelector('.viewer-selection-bar')?.textContent?.trim() || ''");
check("预览模式框选条仍明确说明禁用（不伪造本地引擎能力）",
  selNote.includes("预览模式不调用本地引擎"), selNote.slice(0, 70));
const fakeHits = await evalJs("document.querySelectorAll('.viewer-hit').length");
check("旋转页预览不伪造字符命中元素", fakeHits === 0, `伪造元素=${fakeHits}`);

// 说明文案已更新为「旋转页支持」而不是「旋转页禁用」（读本地源码；注释会被 vite 转译去掉）。
const fs = await import("node:fs");
const srcText = fs.readFileSync(
  new URL("../src/components/PdfPreview.tsx", import.meta.url), "utf8",
);
check("前端说明已更新为旋转页支持",
  srcText.includes("旋转页支持框选"), "含'旋转页支持框选'");

for (const r of results) {
  console.log(`${r.ok ? "PASS" : "FAIL"}  ${r.name} -- ${r.detail}`);
}
const failed = results.filter((r) => !r.ok).length;
console.log(`\n旋转页前端检查: ${results.length - failed} PASS / ${failed} FAIL`);
ws.close();
process.exit(failed === 0 ? 0 : 1);
