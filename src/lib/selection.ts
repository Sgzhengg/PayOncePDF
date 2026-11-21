/**
 * F18 画布框选修订联动：屏幕坐标 ↔ PDF point 的纯几何函数。
 *
 * 全部函数无 DOM、无 React、无副作用，可由 Node 22 直接 import 做单元测试
 * （见 tests/selection-geometry.test.mjs）。PDF 坐标一律左下原点、单位 pt。
 *
 * 诚实边界：这里只做「定位」。选中文字最终交给已验证的 F10/F11/F15
 * 按文本查找命令执行；同页出现多次时前端必须拒绝静默全改（唯一性守卫）。
 */

export interface LayoutChar {
  /** PDFium text page 的全局字符索引（0-based）。 */
  index: number;
  ch: string;
  left: number;
  bottom: number;
  right: number;
  top: number;
}

export interface RectPt {
  left: number;
  bottom: number;
  right: number;
  top: number;
}

/** 屏幕拖拽矩形（左上原点、显示像素）→ PDF 矩形（左下原点、pt）。 */
export function screenRectToPdfRect(
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  displayWidth: number,
  displayHeight: number,
  pageWidthPt: number,
  pageHeightPt: number,
): RectPt {
  if (displayWidth <= 0 || displayHeight <= 0 || pageWidthPt <= 0 || pageHeightPt <= 0) {
    return { left: 0, bottom: 0, right: 0, top: 0 };
  }
  const x0 = Math.min(startX, endX);
  const x1 = Math.max(startX, endX);
  const y0 = Math.min(startY, endY);
  const y1 = Math.max(startY, endY);
  return {
    left: (x0 / displayWidth) * pageWidthPt,
    right: (x1 / displayWidth) * pageWidthPt,
    bottom: pageHeightPt - (y1 / displayHeight) * pageHeightPt,
    top: pageHeightPt - (y0 / displayHeight) * pageHeightPt,
  };
}

/** 两个矩形相交面积占第一个矩形的比例（0..1）。用于字符框命中判定。 */
export function overlapRatio(a: RectPt, b: RectPt): number {
  const areaA = Math.max(0, a.right - a.left) * Math.max(0, a.top - a.bottom);
  if (areaA <= 0) return 0;
  const w = Math.max(0, Math.min(a.right, b.right) - Math.max(a.left, b.left));
  const h = Math.max(0, Math.min(a.top, b.top) - Math.max(a.bottom, b.bottom));
  return (w * h) / areaA;
}

/**
 * 命中字符：中心点落在选区内，或字符框被选区覆盖达到 minRatio。
 * 零宽字符（空格）用中心点判定，避免拖过一整行时漏掉词间空格。
 * 返回的是 chars 数组下标，不是 PDFium 的 index 字段。
 */
export function hitCharIndices(
  chars: LayoutChar[],
  selection: RectPt,
  minRatio = 0.4,
): number[] {
  const out: number[] = [];
  for (let i = 0; i < chars.length; i += 1) {
    const c = chars[i];
    const cx = (c.left + c.right) / 2;
    const cy = (c.bottom + c.top) / 2;
    const centerInside =
      cx >= selection.left && cx <= selection.right &&
      cy >= selection.bottom && cy <= selection.top;
    const covered = overlapRatio(
      { left: c.left, bottom: c.bottom, right: c.right, top: c.top },
      selection,
    ) >= minRatio;
    if (centerInside || covered) out.push(i);
  }
  return out;
}

/** 按 PDFium 字符索引顺序拼接选中文字；换行归一为空格、压缩空白、去首尾。 */
export function selectedText(chars: LayoutChar[], indices: number[]): string {
  const picked = [...new Set(indices)]
    .filter((i) => i >= 0 && i < chars.length)
    .sort((a, b) => chars[a].index - chars[b].index || a - b);
  let out = "";
  for (const i of picked) {
    const ch = chars[i].ch;
    out += ch === "\r" || ch === "\n" || ch === "\t" ? " " : ch;
  }
  return out.replace(/\s+/g, " ").trim();
}

/** 统计选中文字在整页文字层里出现次数（默认不区分大小写，对齐 F10 默认 matchCase=false）。 */
export function countOccurrences(text: string, needle: string, caseSensitive = false): number {
  const hay = caseSensitive ? text : text.toLowerCase();
  const n = caseSensitive ? needle : needle.toLowerCase();
  if (!n) return 0;
  let count = 0;
  let pos = 0;
  for (;;) {
    const hit = hay.indexOf(n, pos);
    if (hit < 0) break;
    count += 1;
    pos = hit + Math.max(n.length, 1);
  }
  return count;
}

/** 按垂直中心把字符聚成文本行；返回每组 chars 下标（组内按字符索引排序）。 */
export function groupLineIndices(chars: LayoutChar[], tolerancePt = 2): number[][] {
  const items = chars
    .map((c, i) => ({ i, cy: (c.top + c.bottom) / 2, index: c.index }))
    .sort((a, b) => a.cy - b.cy || a.index - b.index);
  const lines: { cy: number; members: number[] }[] = [];
  for (const item of items) {
    const line = lines[lines.length - 1];
    if (line && Math.abs(item.cy - line.cy) <= tolerancePt) {
      line.members.push(item.i);
      line.cy = (line.cy * (line.members.length - 1) + item.cy) / line.members.length;
    } else {
      lines.push({ cy: item.cy, members: [item.i] });
    }
  }
  return lines.map((l) => l.members.sort((a, b) => chars[a].index - chars[b].index || a - b));
}

/** 把选区扩到命中的整行：用于「一个词在页内重复」时唯一定位。 */
export function expandToLines(
  chars: LayoutChar[],
  indices: number[],
  tolerancePt = 2,
): number[] {
  const wanted = new Set(indices);
  const expanded = new Set<number>(indices);
  for (const line of groupLineIndices(chars, tolerancePt)) {
    if (line.some((i) => wanted.has(i))) line.forEach((i) => expanded.add(i));
  }
  return [...expanded].sort((a, b) => chars[a].index - chars[b].index || a - b);
}

/** 字符框 → 相对图片的百分比样式（高亮层用，缩放后仍对齐）。 */
export function charPercentRect(c: LayoutChar, pageWidthPt: number, pageHeightPt: number) {
  const pct = (v: number, total: number) => `${(v / total) * 100}%`;
  return {
    left: pct(c.left, pageWidthPt),
    top: pct(pageHeightPt - c.top, pageHeightPt),
    width: pct(Math.max(c.right - c.left, 0.2), pageWidthPt),
    height: pct(Math.max(c.top - c.bottom, 0.2), pageHeightPt),
  };
}

/** 唯一性守卫：只有「非空 + 本页恰好出现一次」才允许一键把选区带入执行命令。 */
export function canApplySelection(pageText: string, text: string, caseSensitive = false): boolean {
  return text.length > 0 && countOccurrences(pageText, text, caseSensitive) === 1;
}

/**
 * 选中字符的外接矩形（PDF pt、左下原点）。
 *
 * 用途：F19 按位置精确替换。用户点「扩到整行」后 hits 变了，必须把矩形同步
 * 扩到整行，否则界面显示的选中文字与实际替换范围会不一致。
 */
export function unionRect(chars: LayoutChar[], indices: number[]): RectPt | null {
  let left = Number.POSITIVE_INFINITY;
  let bottom = Number.POSITIVE_INFINITY;
  let right = Number.NEGATIVE_INFINITY;
  let top = Number.NEGATIVE_INFINITY;
  for (const i of indices) {
    const c = chars[i];
    if (!c) continue;
    if (c.left < left) left = c.left;
    if (c.bottom < bottom) bottom = c.bottom;
    if (c.right > right) right = c.right;
    if (c.top > top) top = c.top;
  }
  if (!Number.isFinite(left) || !Number.isFinite(top) || right <= left || top <= bottom) {
    return null;
  }
  return { left, bottom, right, top };
}

/**
 * 画布是否可以框选。
 *
 * 结果预览默认禁止：那时右侧参数面板仍指向原文件，会出现「看着结果、改了原文件」。
 * 唯一例外是本会话 F19 自己产出的 `*_edited`——它既是预览文件也是下一次的输入，
 * 选区坐标与执行目标同源，必须放行；否则第一次改完就再也框不动，等于只能改一处。
 */
export function canSelectOnCanvas(args: {
  canPreview: boolean;
  hasImage: boolean;
  hasTextLayer: boolean;
  layoutLoading: boolean;
  resultMode: boolean;
  selectableInResult: boolean;
}): boolean {
  if (!args.canPreview || !args.hasImage || !args.hasTextLayer || args.layoutLoading) {
    return false;
  }
  return !args.resultMode || args.selectableInResult;
}
