/**
 * selection-geometry.test.mjs - F18 框选几何与唯一性守卫的纯函数单测。
 *
 * 直接 import `src/lib/selection.ts`（Node 22 内置 TypeScript 类型剥离），
 * 不启动 Tauri、不加载 PDFium、不碰 DOM；验证的是真实坐标换算与判定逻辑，
 * 不是源码字符串断言。运行：npm test / node --test tests/selection-geometry.test.mjs
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  canApplySelection,
  canSelectOnCanvas,
  charPercentRect,
  countOccurrences,
  expandToLines,
  groupLineIndices,
  hitCharIndices,
  overlapRatio,
  screenRectToPdfRect,
  selectedText,
  unionRect,
} from '../src/lib/selection.ts';

/** 在一行上造字符：每个字宽 10pt、高 18pt，index 全局递增。 */
function line(text, y, startX = 10, startIndex = 0) {
  return [...text].map((ch, i) => ({
    index: startIndex + i,
    ch,
    left: startX + i * 10,
    bottom: y,
    right: startX + i * 10 + 9,
    top: y + 18,
  }));
}

test('屏幕矩形 → PDF pt：左上原点翻到左下原点，并按显示尺寸缩放', () => {
  // 显示尺寸 595×842（恰好等于 A4 pt），拖 (100,200) → (200,300)
  const r = screenRectToPdfRect(100, 200, 200, 300, 595, 842, 595, 842);
  // 比例换算会有浮点尾差，按 1e-9 容差比较。
  const near = (actual, expected) => assert.ok(Math.abs(actual - expected) < 1e-9, `${actual} ≈ ${expected}`);
  near(r.left, 100);
  near(r.right, 200);
  near(r.bottom, 842 - 300);
  near(r.top, 842 - 200);
  assert.ok(r.top > r.bottom);
});

test('反向拖拽被归一化；2 倍缩放时坐标按 pt 而非像素', () => {
  const a = screenRectToPdfRect(200, 300, 100, 200, 595, 842, 595, 842);
  const b = screenRectToPdfRect(100, 200, 200, 300, 595, 842, 595, 842);
  assert.deepEqual(a, b);
  // 页面 595pt 宽被渲染成 1190 显示像素：屏幕 x=200 → PDF x=100pt
  const c = screenRectToPdfRect(200, 0, 400, 100, 1190, 1684, 595, 842);
  assert.ok(Math.abs(c.left - 100) < 1e-9);
  assert.ok(Math.abs(c.right - 200) < 1e-9);
  assert.equal(c.top, 842);
  assert.equal(c.bottom, 792);
});

test('非法显示尺寸返回零矩形，不产生 NaN 坐标', () => {
  const r = screenRectToPdfRect(0, 0, 10, 10, 0, 0, 595, 842);
  assert.deepEqual(r, { left: 0, bottom: 0, right: 0, top: 0 });
  for (const v of Object.values(r)) assert.ok(Number.isFinite(v));
});

test('overlapRatio：包含 / 部分重叠 / 不相交', () => {
  const a = { left: 0, bottom: 0, right: 10, top: 10 };
  assert.equal(overlapRatio(a, a), 1);
  assert.equal(overlapRatio(a, { left: 5, bottom: 0, right: 15, top: 10 }), 0.5);
  assert.equal(overlapRatio(a, { left: 20, bottom: 20, right: 30, top: 30 }), 0);
  assert.equal(overlapRatio({ left: 0, bottom: 0, right: 0, top: 0 }, a), 0);
});

test('hitCharIndices：中心点命中、覆盖率命中、框外排除、零宽空格可命中', () => {
  const chars = [
    { index: 0, ch: 'A', left: 10, bottom: 100, right: 20, top: 120 },
    { index: 1, ch: 'B', left: 20, bottom: 100, right: 30, top: 120 },
    { index: 2, ch: 'C', left: 200, bottom: 100, right: 210, top: 120 },
    { index: 3, ch: ' ', left: 30, bottom: 100, right: 30, top: 120 },
  ];
  const sel = { left: 8, bottom: 98, right: 32, top: 122 };
  assert.deepEqual(hitCharIndices(chars, sel), [0, 1, 3]);
  // 只覆盖 A 的右半边（< 0.4 阈值）时靠中心点仍可命中；完全在框外的 C 不命中
  assert.deepEqual(hitCharIndices(chars, { left: 17, bottom: 98, right: 18, top: 122 }), []);
  assert.deepEqual(hitCharIndices(chars, { left: 200, bottom: 98, right: 212, top: 122 }), [2]);
});

test('selectedText：按 PDFium 字符索引排序、换行归一、空白压缩', () => {
  const chars = [
    { index: 3, ch: 'd', left: 0, bottom: 0, right: 1, top: 1 },
    { index: 1, ch: 'b', left: 0, bottom: 0, right: 1, top: 1 },
    { index: 2, ch: '\n', left: 0, bottom: 0, right: 1, top: 1 },
    { index: 0, ch: 'a', left: 0, bottom: 0, right: 1, top: 1 },
  ];
  assert.equal(selectedText(chars, [0, 1, 2, 3]), 'ab d');
  assert.equal(selectedText(chars, []), '');
  assert.equal(selectedText(chars, [0, 0, 99]), 'd');
});

test('countOccurrences：默认不区分大小写，支持大小写敏感与空 needle', () => {
  assert.equal(countOccurrences('合同 2026 元 / 合同 2026 元', '2026'), 2);
  assert.equal(countOccurrences('Hello hello HELLO', 'hello'), 3);
  assert.equal(countOccurrences('Hello hello', 'hello', true), 1);
  assert.equal(countOccurrences('abc', ''), 0);
  assert.equal(countOccurrences('aaaa', 'aa'), 2);
});

test('groupLineIndices 按垂直中心分行，expandToLines 只扩命中的行', () => {
  const top = line('AB', 700, 10, 0);
  const bottom = line('CD', 660, 10, 2);
  const chars = [...top, ...bottom];
  const lines = groupLineIndices(chars);
  assert.equal(lines.length, 2);
  assert.deepEqual(lines[0], [2, 3]);
  assert.deepEqual(lines[1], [0, 1]);
  const expanded = expandToLines(chars, [0]);
  assert.deepEqual(expanded, [0, 1]);
  assert.equal(selectedText(chars, expanded), 'AB');
});

test('唯一性守卫：重复文字拒绝一键执行，扩到整行后可唯一定位', () => {
  // 两行都有 2026，但整行文字不同：框选单词重复 → 扩行 → 唯一。
  const chars = [...line('金额 2026 元', 700, 10, 0), ...line('编号 2026 号', 660, 10, 12)];
  const pageText = chars.map((c) => c.ch).join('');
  const wordHits = hitCharIndices(chars, { left: 40, bottom: 698, right: 90, top: 720 });
  const word = selectedText(chars, wordHits);
  assert.equal(word, '2026');
  assert.equal(countOccurrences(pageText, word), 2);
  assert.equal(canApplySelection(pageText, word), false);
  const wholeLine = selectedText(chars, expandToLines(chars, wordHits));
  assert.equal(wholeLine, '金额 2026 元');
  assert.equal(canApplySelection(pageText, wholeLine), true);
  assert.equal(canApplySelection(pageText, ''), false);
});

test('charPercentRect：top 翻转并输出百分比，零宽字符也有最小尺寸', () => {
  const c = { index: 0, ch: 'A', left: 59.5, bottom: 742, right: 119, top: 760 };
  const r = charPercentRect(c, 595, 842);
  assert.equal(r.left, '10%');
  assert.equal(r.width, '10%');
  assert.equal(r.top, `${((842 - 760) / 842) * 100}%`);
  assert.ok(r.height.endsWith('%'));
  const zero = charPercentRect({ ...c, right: c.left }, 595, 842);
  assert.ok(parseFloat(zero.width) > 0);
});

test('unionRect：F19 用选中字符的外接矩形，扩行后与界面选中一致', () => {
  const chars = line('Draft one', 700, 10, 0);
  // 前 5 个字符 = Draft
  const rect = unionRect(chars, [0, 1, 2, 3, 4]);
  assert.deepEqual(rect, { left: 10, bottom: 700, right: 59, top: 718 });
  // 整行：右边界必须扩到最后一个字符
  const whole = unionRect(chars, chars.map((_, i) => i));
  assert.equal(whole.left, 10);
  assert.equal(whole.right, 10 + 8 * 10 + 9);
  // 非法输入不返回伪造坐标
  assert.equal(unionRect(chars, []), null);
  assert.equal(unionRect(chars, [999]), null);
  // 零面积区域（同一字符重复计入不会产生退化矩形）
  assert.equal(unionRect([{ index: 0, ch: ' ', left: 10, bottom: 700, right: 10, top: 718 }], [0]), null);
});

// ── 结果预览的框选许可（真机回归：2026-11-21 用户报「改完一处后鼠标不能再选框」）──
test('非结果预览：页面就绪即可框选', () => {
  assert.equal(canSelectOnCanvas({
    canPreview: true, hasImage: true, hasTextLayer: true, layoutLoading: false,
    resultMode: false, selectableInResult: false,
  }), true);
});

test('结果预览但非本会话产物：禁止框选（避免看着结果改原文件）', () => {
  assert.equal(canSelectOnCanvas({
    canPreview: true, hasImage: true, hasTextLayer: true, layoutLoading: false,
    resultMode: true, selectableInResult: false,
  }), false);
});

test('结果预览是本会话 _edited：允许继续框选（否则只能改一处）', () => {
  assert.equal(canSelectOnCanvas({
    canPreview: true, hasImage: true, hasTextLayer: true, layoutLoading: false,
    resultMode: true, selectableInResult: true,
  }), true);
});

test('文字层加载中 / 无文字层 / 无图：任何情况都不框选', () => {
  const base = {
    canPreview: true, hasImage: true, hasTextLayer: true, layoutLoading: false,
    resultMode: false, selectableInResult: true,
  };
  assert.equal(canSelectOnCanvas({ ...base, layoutLoading: true }), false);
  assert.equal(canSelectOnCanvas({ ...base, hasTextLayer: false }), false);
  assert.equal(canSelectOnCanvas({ ...base, hasImage: false }), false);
  assert.equal(canSelectOnCanvas({ ...base, canPreview: false }), false);
});
