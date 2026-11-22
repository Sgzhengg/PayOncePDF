import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import {
  IS_TAURI,
  getPageTextLayout,
  getPdfInfo,
  renderPagePreview,
} from "../lib/backend";
import type {
  AppError,
  PageTextLayout,
  PdfInfo,
  PreviewImage,
  SelectionAction,
  SelectionTool,
} from "../lib/types";
import {
  canApplySelection,
  canSelectOnCanvas,
  charPercentRect,
  countOccurrences,
  estimateRectEditWidth,
  expandToLines,
  hitCharIndices,
  screenRectToPdfRect,
  selectedText,
  unionRect,
} from "../lib/selection";
import type { RectPt } from "../lib/selection";

interface PdfPreviewProps {
  filePath: string | null;
  fileName: string | null;
  /** 把当前页号填入右侧参数面板的「页码」字段。 */
  onUsePage?: (page: number) => void;
  /** F18：把框选文字 + 页码带入右侧改字 / 批注 / 脱敏参数面板。 */
  onApplySelection?: (action: SelectionAction) => void;
  /** F19：按框选矩形直接改这一处（按坐标定位，不做全文查找）。 */
  onRectEdit?: (req: RectEditSelection) => void;
  /** 任务成功后预览的是输出文件，不是输入文件。 */
  resultMode?: boolean;
  /**
   * 结果预览仍允许框选：仅当这份结果是本会话 F19 自己产出的 `*_edited`，
   * 也就是「接着改刚生成的那份」这一种合法场景。其它输出（压缩、拆分、合并…）
   * 仍禁止框选，避免「看着结果却改了原文件」。
   */
  selectableInResult?: boolean;
  onBackToInput?: () => void;
  /**
   * F19 连续编辑：结果被原地更新时文件路径不变，只靠路径无法触发重载。
   * 每成功一次任务自增一次，预览据此重新读页数、重渲染、重取文字布局。
   */
  reloadToken?: number;
}

/** F19 请求：rect 为 PDF pt、左下原点。 */
export interface RectEditSelection {
  page: number;
  rect: RectPt;
  find: string;
  replace: string;
  /** 预览到的文字，仅在用户没手动改写选中文字时下发（用于发现错位）。 */
  expected?: string;
}

type AnnotKind = NonNullable<SelectionAction["kind"]>;

const ZOOMS = [
  { label: "75%", dpi: 72 },
  { label: "100%", dpi: 96 },
  { label: "150%", dpi: 144 },
  { label: "200%", dpi: 192 },
];

const ANNOT_KINDS: { value: AnnotKind; label: string }[] = [
  { value: "highlight", label: "高亮" },
  { value: "underline", label: "下划线" },
  { value: "strikeout", label: "删除线" },
  { value: "squiggly", label: "波浪线" },
  { value: "note", label: "便签" },
];

interface DragState {
  startX: number;
  startY: number;
  endX: number;
  endY: number;
}

function isPdf(name: string | null): boolean {
  return Boolean(name && /\.pdf$/i.test(name));
}

/**
 * F17 内置阅读视图 + F18 画布框选 + F19 按矩形精确替换（主用途决策 §5 的 P0）。
 *
 * 真机：PDFium 在本机渲染页面 PNG、返回字符级包围盒，base64 / JSON 经 IPC 回传，
 * 不落盘、不联网。
 *
 * 两条出口，边界必须说清楚：
 * - F19「直接改这一处」：把选区矩形交给引擎，按坐标定位只改这一处，不受同页
 *   重复文字影响；只支持同一行内的连续文字。
 * - F18「填入参数」：把选中的文字与页码带进右侧面板，执行仍走 F10/F11/F15 的
 *   文本查找，因此必须唯一命中（唯一性守卫），否则会静默把相同文字一起改掉。
 *
 * 浏览器预览 / 扫描件 / 无文字层：明确禁用框选，绝不伪造文字坐标或交互。
 * 旋转页支持框选：后端已把字符框变换到显示坐标系（见 geometry.rs）。
 */
export function PdfPreview({
  filePath,
  fileName,
  onUsePage,
  onApplySelection,
  onRectEdit,
  reloadToken = 0,
  resultMode = false,
  selectableInResult = false,
  onBackToInput,
}: PdfPreviewProps) {
  const [info, setInfo] = useState<PdfInfo | null>(null);
  const [image, setImage] = useState<PreviewImage | null>(null);
  const [page, setPage] = useState(1);
  const [dpi, setDpi] = useState(96);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needPassword, setNeedPassword] = useState(false);
  const [password, setPassword] = useState("");
  const [passwordQuery, setPasswordQuery] = useState("");

  // F18 框选状态
  const [layout, setLayout] = useState<PageTextLayout | null>(null);
  const [layoutLoading, setLayoutLoading] = useState(false);
  const [layoutError, setLayoutError] = useState<string | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const [hits, setHits] = useState<number[]>([]);
  // F19：选中区域在 PDF 坐标系里的矩形；扩行时必须同步，否则替换范围与界面不一致
  const [selRect, setSelRect] = useState<RectPt | null>(null);
  const [textOverride, setTextOverride] = useState<string | null>(null);
  const [replaceValue, setReplaceValue] = useState("");
  // F19 宽度溢出预警：用户确认过「仍要改」后才放行，选区或替换文字一变就重置。
  const [overflowAck, setOverflowAck] = useState(false);
  const [annotKind, setAnnotKind] = useState<AnnotKind>("highlight");
  const wrapRef = useRef<HTMLDivElement | null>(null);

  const canPreview = IS_TAURI && Boolean(filePath) && isPdf(fileName);
  const pages = info?.pages ?? 0;

  // 1) 文件 / 密码 / 原地更新 → 读取页数与纸张尺寸
  //    原地更新（连续改字）时路径不变，此时不能把页码重置为 1，否则用户每改一处
  //    就被弹回首页；只有真的换了文件才重置。
  const lastPathRef = useRef<string | null>(null);
  useEffect(() => {
    let alive = true;
    const fileChanged = lastPathRef.current !== filePath;
    lastPathRef.current = filePath;
    if (fileChanged) {
      setInfo(null);
      setImage(null);
      // 切到本次改字的结果文件时页数不变，保留页码；否则在第 5 页改完就被弹回第 1 页，
      // 想接着改第 6 页得重新翻。其它输出（合并/拆分/压缩）仍回第 1 页。
      if (!selectableInResult) setPage(1);
    }
    setError(null);
    setNeedPassword(false);
    if (!canPreview || !filePath) return () => { alive = false; };
    setLoading(true);
    getPdfInfo(filePath, passwordQuery || undefined)
      .then((v) => {
        if (!alive) return;
        setInfo(v);
        setLoading(false);
      })
      .catch((e: unknown) => {
        if (!alive) return;
        const err = e as Partial<AppError>;
        setLoading(false);
        if (err?.code === "E1002") {
          setNeedPassword(true);
          setError("该 PDF 有密码，请输入后重试");
        } else {
          setError(err?.message ?? "无法读取该 PDF");
        }
      });
    return () => { alive = false; };
  }, [canPreview, filePath, passwordQuery, reloadToken, selectableInResult]);

  // 2) 页号 / 缩放变化 → 渲染单页
  const loadPage = useCallback(async () => {
    if (!canPreview || !filePath || !info) return;
    setLoading(true);
    try {
      const result = await renderPagePreview(filePath, page, dpi, passwordQuery || undefined);
      if (result) setImage(result);
      setError(null);
    } catch (e: unknown) {
      const err = e as Partial<AppError>;
      setError(err?.message ?? "页面渲染失败");
      setImage(null);
    } finally {
      setLoading(false);
    }
  }, [canPreview, filePath, info, page, dpi, passwordQuery, reloadToken]);

  useEffect(() => { void loadPage(); }, [loadPage]);

  // 3) 页号 / 文件 / 密码变化 → 读取该页字符级文字布局（与 DPI 无关）
  useEffect(() => {
    let alive = true;
    setLayout(null);
    setLayoutError(null);
    setHits([]);
    setDrag(null);
    setSelRect(null);
    setTextOverride(null);
    if (!canPreview || !filePath) return () => { alive = false; };
    setLayoutLoading(true);
    getPageTextLayout(filePath, page, passwordQuery || undefined)
      .then((v) => {
        if (!alive) return;
        setLayout(v);
        setLayoutLoading(false);
      })
      .catch((e: unknown) => {
        if (!alive) return;
        const err = e as Partial<AppError>;
        setLayoutError(err?.message ?? "文字层读取失败");
        setLayoutLoading(false);
      });
    return () => { alive = false; };
  }, [canPreview, filePath, page, passwordQuery, reloadToken]);

  // 4) 每次任务成功（reloadToken 自增）→ 清空「改成」。
  //    不清的话，上一处填的替换词会留在输入框里，新选区一放就带着旧值，
  //    手快点下去就是「把别的字改成上一处的词」，这类错最难发现。
  useEffect(() => { setReplaceValue(""); }, [reloadToken]);

  const goto = (next: number) => {
    if (!pages) return;
    setPage(Math.min(Math.max(next, 1), pages));
  };

  // 结果预览默认不可框选（参数面板仍指向原文件）；唯一例外是本会话 F19 产出的
  // `*_edited`，见 canSelectOnCanvas 的注释与 tests/selection-geometry.test.mjs。
  const canSelect = canSelectOnCanvas({
    canPreview,
    hasImage: Boolean(image),
    hasTextLayer: Boolean(layout?.hasTextLayer),
    layoutLoading,
    resultMode,
    selectableInResult,
  });

  const localPoint = (e: ReactPointerEvent<HTMLDivElement>) => {
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect) return null;
    return {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
      w: rect.width,
      h: rect.height,
    };
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!canSelect) return;
    const p = localPoint(e);
    if (!p) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setHits([]);
    setSelRect(null);
    setTextOverride(null);
    setOverflowAck(false);
    setDrag({ startX: p.x, startY: p.y, endX: p.x, endY: p.y });
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const p = localPoint(e);
    if (!p) return;
    setDrag({ ...drag, endX: p.x, endY: p.y });
  };

  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag || !layout) return;
    const p = localPoint(e);
    if (!p) { setDrag(null); return; }
    // 单击（无有效拖拽）视为取消选区，不把一次点击误判成选中一个字。
    if (Math.abs(p.x - drag.startX) < 3 && Math.abs(p.y - drag.startY) < 3) {
      setHits([]);
      setSelRect(null);
      setOverflowAck(false);
      setDrag(null);
      return;
    }
    const rect = screenRectToPdfRect(
      drag.startX, drag.startY, p.x, p.y,
      p.w, p.h, layout.pageWidthPt, layout.pageHeightPt,
    );
    setHits(hitCharIndices(layout.chars, rect));
    setSelRect(rect);
    setOverflowAck(false);
    setDrag(null);
  };

  const chars = layout?.chars ?? [];
  const selText = textOverride ?? selectedText(chars, hits);
  const occurrences = layout && selText ? countOccurrences(layout.text, selText) : 0;
  const unique = canApplySelection(layout?.text ?? "", selText);
  const hasSelection = hits.length > 0 && selText.length > 0;

  const expandSelection = () => {
    if (!layout) return;
    const expanded = expandToLines(layout.chars, hits);
    setHits(expanded);
    // 矩形同步扩到整行：F19 按这块矩形替换，显示与执行范围必须一致
    setSelRect(unionRect(layout.chars, expanded));
    setTextOverride(null);
    setOverflowAck(false);
  };

  // F19：按坐标定位替换。只有「有矩形 + 填了替换文字 + 没手动改写选中文字」才允许，
  // 因为后端会按矩形自己读出原文，手改的文字与矩形内容不一致时无法安全执行。
  const rectEditBaseReady =
    Boolean(selRect) && replaceValue.length > 0 && textOverride === null && Boolean(onRectEdit);
  // 宽度溢出预警：用选中字符框的真实宽度反推字宽，估算替换后是否可能压到右侧。
  // 手改了「选中文字」时 selText 与选区已脱钩，估算无意义，不显示预警。
  const widthEstimate =
    textOverride === null && replaceValue.length > 0
      ? estimateRectEditWidth(chars, hits, replaceValue)
      : null;
  const mayOverflow = Boolean(widthEstimate?.likelyOverflow);
  // 有溢出风险时，必须由用户明确确认过「仍要改」才放行；这是预警不是阻断。
  const rectEditReady = rectEditBaseReady && (!mayOverflow || overflowAck);

  const applyRectEdit = () => {
    if (!selRect || !layout || !onRectEdit || !rectEditReady) return;
    onRectEdit({
      page: layout.page,
      rect: selRect,
      find: selText,
      replace: replaceValue,
      expected: selText,
    });
  };

  const apply = (tool: SelectionTool) => {
    if (!unique || !layout || !onApplySelection) return;
    const action: SelectionAction = { tool, find: selText, page: layout.page };
    if (tool === "edit") action.replace = replaceValue;
    if (tool === "annotate") action.kind = annotKind;
    onApplySelection(action);
  };

  const marquee = drag
    ? {
        left: Math.min(drag.startX, drag.endX),
        top: Math.min(drag.startY, drag.endY),
        width: Math.abs(drag.endX - drag.startX),
        height: Math.abs(drag.endY - drag.startY),
      }
    : null;

  const paperMm = info
    ? `${Math.round((info.pageWidthPt * 25.4) / 72)} × ${Math.round((info.pageHeightPt * 25.4) / 72)} mm`
    : "—";

  return (
    <section className="viewer" aria-label="文档预览">
      <header className="viewer-head">
        <div className="viewer-heading">
          <span className="viewer-kicker">文档预览</span>
          {resultMode ? <span className="viewer-result-badge">结果预览</span> : null}
          <span className="viewer-file" title={fileName ?? ""}>{fileName ?? "未选择文件"}</span>
        </div>
        <div className="viewer-controls">
          <button
            type="button"
            className="viewer-btn"
            aria-label="上一页"
            disabled={!canPreview || page <= 1}
            onClick={() => goto(page - 1)}
          >‹</button>
          <span className="viewer-page">
            <input
              className="viewer-page-input"
              aria-label="当前页码"
              type="number"
              min={1}
              max={Math.max(pages, 1)}
              value={page}
              disabled={!canPreview}
              onChange={(e) => goto(Number(e.target.value) || 1)}
            />
            <span className="viewer-page-total">/ {pages || "—"}</span>
          </span>
          <button
            type="button"
            className="viewer-btn"
            aria-label="下一页"
            disabled={!canPreview || page >= pages}
            onClick={() => goto(page + 1)}
          >›</button>
          <select
            className="viewer-zoom"
            aria-label="缩放"
            value={dpi}
            disabled={!canPreview}
            onChange={(e) => setDpi(Number(e.target.value))}
          >
            {ZOOMS.map((z) => <option key={z.dpi} value={z.dpi}>{z.label}</option>)}
          </select>
          {resultMode && onBackToInput ? (
            <button type="button" className="viewer-btn" onClick={onBackToInput}>返回原文件</button>
          ) : null}
          <button
            type="button"
            className="viewer-btn viewer-btn-accent"
            title="把当前页号填入右侧参数面板的页码字段"
            disabled={!canPreview || !onUsePage}
            onClick={() => onUsePage?.(page)}
          >用当前页</button>
        </div>
      </header>

      {needPassword ? (
        <div className="viewer-password">
          <input
            type="password"
            aria-label="PDF 密码"
            placeholder="PDF 密码"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") setPasswordQuery(password); }}
          />
          <button type="button" onClick={() => setPasswordQuery(password)}>解锁</button>
        </div>
      ) : null}

      <div className="viewer-canvas">
        {canPreview && image ? (
          <div
            className={`viewer-page-wrap${canSelect ? " viewer-page-wrap-selectable" : ""}`}
            ref={wrapRef}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={() => setDrag(null)}
          >
            <img
              className="viewer-page-image"
              src={`data:image/png;base64,${image.pngBase64}`}
              alt={`第 ${image.page} 页预览`}
              width={image.width}
              height={image.height}
              draggable={false}
            />
            {canSelect && hits.length > 0 && layout ? (
              <div className="viewer-hit-layer" aria-hidden="true">
                {hits.map((i) => {
                  const c = chars[i];
                  if (!c) return null;
                  const box = charPercentRect(c, layout.pageWidthPt, layout.pageHeightPt);
                  return <span key={i} className="viewer-hit" style={box} />;
                })}
              </div>
            ) : null}
            {marquee ? <div className="viewer-marquee" style={marquee} aria-hidden="true" /> : null}
          </div>
        ) : canPreview ? (
          <div className="viewer-paper viewer-paper-loading" aria-live="polite">
            {loading ? "正在本机渲染…" : error ? `渲染失败：${error}` : "读取页面中…"}
          </div>
        ) : (
          <div className="viewer-paper viewer-paper-demo" aria-hidden="true">
            <div className="demo-sheet">
              <span className="demo-title">PayOnce PDF</span>
              <span className="demo-line w70" />
              <span className="demo-line w90" />
              <span className="demo-line w60" />
              <span className="demo-line w85" />
              <span className="demo-line w45" />
              <div className="demo-badge">示意页 · 非真实文档</div>
            </div>
            <p className="viewer-empty-note">
              {isPdf(fileName)
                ? "预览模式：浏览器不调用本地引擎，不渲染文件。"
                : "拖入 PDF 后，这里会直接显示页面。"}
              <br />
              <span>桌面版可翻页、缩放，并在页面上框选文字直接带入改字 / 批注 / 脱敏。</span>
            </p>
          </div>
        )}
      </div>

      {/* F18 框选动作条：只做定位与参数填充，执行仍由底部统一按钮完成 */}
      <div className="viewer-selection-bar" aria-live="polite">
        {!canPreview ? (
          <span className="viewer-sel-note">桌面版可用：在页面上拖选文字，一键带入改字 / 批注 / 脱敏。预览模式不调用本地引擎。</span>
        ) : resultMode && !selectableInResult ? (
          <span className="viewer-sel-note">当前预览的是输出文件；为避免「看着结果却改了原文件」，结果预览不提供框选。点「返回原文件」可继续修订。</span>
        ) : layoutError ? (
          <span className="viewer-sel-warn">文字层不可用：{layoutError}</span>
        ) : layoutLoading ? (
          <span className="viewer-sel-note">正在读取本页文字位置…</span>
        ) : layout && !layout.hasTextLayer ? (
          <span className="viewer-sel-warn">{layout.unsupportedReason ?? "本页不支持框选"}</span>
        ) : !canSelect ? (
          <span className="viewer-sel-note">页面渲染后可拖选文字。</span>
        ) : !hasSelection ? (
          <span className="viewer-sel-note">在页面上按住鼠标拖过一段文字，即可选中并带入右侧工具。</span>
        ) : (
          <>
            <div className="viewer-sel-row">
              <label className="viewer-sel-label" htmlFor="viewer-sel-text">选中文字</label>
              <input
                id="viewer-sel-text"
                className="viewer-sel-input viewer-sel-text"
                aria-label="选中的文字"
                value={selText}
                onChange={(e) => setTextOverride(e.target.value)}
              />
              <span className={unique ? "viewer-sel-count" : "viewer-sel-count viewer-sel-count-warn"}>
                本页出现 {occurrences} 次
              </span>
              {!unique && occurrences > 1 ? (
                <button type="button" className="viewer-btn" onClick={expandSelection}>扩到整行</button>
              ) : null}
            </div>
            <div className="viewer-sel-row">
              <label className="viewer-sel-label" htmlFor="viewer-sel-replace">改成</label>
              <input
                id="viewer-sel-replace"
                className="viewer-sel-input viewer-sel-replace"
                aria-label="替换后的文字"
                placeholder="替换后的文字"
                value={replaceValue}
                onChange={(e) => {
                  setReplaceValue(e.target.value);
                  setOverflowAck(false);
                }}
              />
              <button
                type="button"
                className="viewer-btn viewer-btn-accent"
                title="按框选坐标定位，只改选中的这一处；同页其它相同文字不受影响"
                disabled={!rectEditReady}
                onClick={applyRectEdit}
              >直接改这一处</button>
              {/* 结果预览里不提供「填入参数」：那三个按钮走文本查找，目标是原文件，
                  与当前预览的不是同一份，点下去必然错位。 */}
              {!resultMode ? (
                <>
                  <button
                    type="button"
                    className="viewer-btn"
                    disabled={!unique || !onApplySelection}
                    onClick={() => apply("edit")}
                  >填入改字参数</button>
                  <select
                    className="viewer-zoom"
                    aria-label="批注类型"
                    value={annotKind}
                    onChange={(e) => setAnnotKind(e.target.value as AnnotKind)}
                  >
                    {ANNOT_KINDS.map((k) => <option key={k.value} value={k.value}>{k.label}</option>)}
                  </select>
                  <button
                    type="button"
                    className="viewer-btn"
                    disabled={!unique || !onApplySelection}
                    onClick={() => apply("annotate")}
                  >填入批注参数</button>
                  <button
                    type="button"
                    className="viewer-btn"
                    disabled={!unique || !onApplySelection}
                    onClick={() => apply("redact")}
                  >填入脱敏参数</button>
                </>
              ) : null}
            </div>
            {mayOverflow && widthEstimate ? (
              <div className="viewer-sel-overflow" role="status">
                <span className="viewer-sel-warn">
                  宽度预警：估算新文字约 {widthEstimate.estimatedWidthPt.toFixed(1)} pt，
                  比选中的原文（{widthEstimate.originalWidthPt.toFixed(1)} pt）宽约 {widthEstimate.overflowPt.toFixed(1)} pt
                  （{Math.round(widthEstimate.overflowRatio * 100)}%）。引擎不会自动重排，多出的部分可能盖住右侧内容。
                </span>
                {overflowAck ? (
                  <span className="viewer-sel-note">
                    你已确认按原样替换。可缩短文字、或直接点「直接改这一处」执行。
                  </span>
                ) : (
                  <button
                    type="button"
                    className="viewer-btn viewer-btn-warn"
                    onClick={() => setOverflowAck(true)}
                  >已知风险，仍要改这一处</button>
                )}
              </div>
            ) : null}
            {resultMode ? (
              <span className="viewer-sel-note">
                正在继续修改本次结果文件：填好「改成」后点「直接改这一处」，改动会累积在同一份 <code>_edited</code> 文件里，原始文件始终不变。
              </span>
            ) : unique ? (
              <span className="viewer-sel-note">
                本页唯一命中：可直接点「直接改这一处」按坐标就地替换；也可以填入右侧参数再点底部「执行」。结果另存，原文件不变。
              </span>
            ) : (
              <span className="viewer-sel-warn">
                这段文字在本页出现 {occurrences} 次（或整页文字层里找不到它），已禁止「按文本」一键带入——那会把相同文字一起改掉。可直接用「直接改这一处」按坐标只改选中这一处；跨行或需要批量替换时，请点「扩到整行」或在右侧自行填写查找内容。
              </span>
            )}
          </>
        )}
      </div>

      <footer className="viewer-foot">
        <span>{canPreview ? `第 ${page} / ${pages || "—"} 页` : "尚未加载页面"}</span>
        <span>纸张 {paperMm}</span>
        <span>{dpi} DPI</span>
        <span>{canSelect ? "可框选文字" : "框选不可用"}</span>
        <span className="viewer-local">本机渲染 · 文件不上传</span>
      </footer>
    </section>
  );
}
