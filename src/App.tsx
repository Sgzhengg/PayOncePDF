import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import type {
  AnnotateRequest,
  AppInfo,
  BatchRequest,
  CompressRequest,
  DecorateRequest,
  EditTextRequest,
  ExportRequest,
  FormFieldSpec,
  FormRequest,
  BookmarkRequest,
  ImagesToPdfRequest,
  LicenseState,
  MergeRequest,
  OcrRequest,
  PageOpsRequest,
  ProgressEvent,
  RectEditRequest,
  RedactRequest,
  SelectionAction,
  ShareRequest,
  SignRequest,
  SplitRequest,
  TextToPdfRequest,
  ToImagesRequest,
  ToolId,
  UpdateSettings,
} from "./lib/types";
import type { CommandPayload } from "./lib/backend";
import {
  COMMANDS,
  IS_TAURI,
  activateLicense,
  cancelJob,
  getAppInfo,
  getLicenseState,
  getUpdateSettings,
  invokeCommand,
  openOutputFolder,
  setLogging,
  setUpdateEnabled,
  subscribeProgress,
} from "./lib/backend";
import { hintForCode, normalizeError } from "./lib/errors";
import { ALL_TOOLS, createDefaultParams } from "./lib/params";
import type { FileItem, InputItem, ParamsState } from "./lib/params";
import { PdfPreview } from "./components/PdfPreview";
import type { RectEditSelection } from "./components/PdfPreview";
import { Sidebar } from "./components/Sidebar";
import type { DialogView } from "./components/Sidebar";
import { PreviewBanner } from "./components/PreviewBanner";
import { DropZone } from "./components/DropZone";
import { FileList } from "./components/FileList";
import { ParamPanel } from "./components/ParamPanel";
import type { ParamOnChange } from "./components/ParamPanel";
import { ProgressBar } from "./components/ProgressBar";
import type { JobStatus, ProgressState } from "./components/ProgressBar";
import { AboutDialog } from "./components/AboutDialog";
import { LicensePanel } from "./components/LicensePanel";
import { NetworkPanel } from "./components/NetworkPanel";

// 参数 → 契约 §3 命令与 §4 请求类型的映射。所有请求带 jobId，密码等可选字段按需展开。
function buildRequest(
  tool: ToolId,
  p: ParamsState,
  list: FileItem[],
  jobId: string,
): { cmd: string; payload: CommandPayload } {
  const paths = list.map((f) => f.path);
  const first = paths[0] ?? "";
  switch (tool) {
    case "merge": {
      const payload: MergeRequest = {
        jobId,
        files: paths,
        output: p.merge.output,
        keepBookmarks: p.merge.keepBookmarks,
        ...(p.merge.password ? { password: p.merge.password } : {}),
      };
      return { cmd: COMMANDS.merge, payload };
    }
    case "split": {
      const payload: SplitRequest = {
        jobId,
        file: first,
        outputDir: p.split.outputDir,
        mode: p.split.mode,
        ...(p.split.mode === "everyN" ? { n: p.split.n } : {}),
        ...(p.split.mode === "ranges" ? { ranges: p.split.ranges } : {}),
        nameTemplate: p.split.nameTemplate,
        ...(p.split.password ? { password: p.split.password } : {}),
      };
      return { cmd: COMMANDS.split, payload };
    }
    case "compress": {
      const payload: CompressRequest = {
        jobId,
        file: first,
        output: p.compress.output,
        targetBytes: Math.max(1, Math.round(p.compress.targetMB * 1024 * 1024)),
        quality: p.compress.quality,
        ...(p.compress.password ? { password: p.compress.password } : {}),
      };
      return { cmd: COMMANDS.compress, payload };
    }
    case "convert": {
      if (p.convert.direction === "toImages") {
        const payload: ToImagesRequest = {
          jobId,
          file: first,
          outputDir: p.convert.outputDir,
          format: p.convert.format,
          dpi: p.convert.dpi,
          ...(p.convert.pages ? { pages: p.convert.pages } : {}),
          ...(p.convert.password ? { password: p.convert.password } : {}),
        };
        return { cmd: COMMANDS.toImages, payload };
      }
      const payload: ImagesToPdfRequest = {
        jobId,
        files: paths,
        output: p.convert.output,
        pageSize: p.convert.pageSize,
        margin: p.convert.margin,
      };
      return { cmd: COMMANDS.toPdf, payload };
    }
    case "ocr": {
      const payload: OcrRequest = {
        jobId,
        file: first,
        output: p.ocr.output,
        outputFormat: p.ocr.outputFormat,
        languages: [...p.ocr.languages],
        ...(p.ocr.pages ? { pages: p.ocr.pages } : {}),
        ...(p.ocr.password ? { password: p.ocr.password } : {}),
      };
      return { cmd: COMMANDS.ocr, payload };
    }
    case "sign": {
      const payload: SignRequest = {
        jobId,
        file: first,
        output: p.sign.output,
        signaturePath: p.sign.signaturePath,
        page: p.sign.page,
        x: p.sign.x,
        y: p.sign.y,
        width: p.sign.width,
        height: p.sign.height,
        fields: { ...p.sign.fields },
        flattenFields: p.sign.flattenFields,
        ...(p.sign.password ? { password: p.sign.password } : {}),
      };
      return { cmd: COMMANDS.sign, payload };
    }
    case "batch": {
      const payload: BatchRequest = {
        jobId,
        inputDir: p.batch.inputDir,
        outputDir: p.batch.outputDir,
        operation: p.batch.operation,
        options: { ...p.batch.options },
      };
      return { cmd: COMMANDS.batch, payload };
    }
    case "edit": {
      const payload: EditTextRequest = {
        jobId,
        file: first,
        output: p.edit.output,
        find: p.edit.find,
        replace: p.edit.replace,
        matchCase: p.edit.matchCase,
        wholeWord: p.edit.wholeWord,
        ...(p.edit.pages ? { pages: p.edit.pages } : {}),
        ...(p.edit.password ? { password: p.edit.password } : {}),
      };
      return { cmd: COMMANDS.editText, payload };
    }
    case "annotate": {
      const payload: AnnotateRequest = {
        jobId,
        file: first,
        output: p.annotate.output,
        find: p.annotate.find,
        kind: p.annotate.kind,
        color: p.annotate.color,
        author: p.annotate.author,
        note: p.annotate.note,
        matchCase: p.annotate.matchCase,
        ...(p.annotate.pages ? { pages: p.annotate.pages } : {}),
        ...(p.annotate.password ? { password: p.annotate.password } : {}),
      };
      return { cmd: COMMANDS.annotate, payload };
    }
    case "pages": {
      const payload: PageOpsRequest = {
        jobId,
        file: first,
        output: p.pages.output,
        mode: p.pages.mode,
        pages: p.pages.pages,
        degrees: p.pages.degrees,
        insertAt: p.pages.insertAt,
        insertCount: p.pages.insertCount,
        margins: [
          p.pages.marginLeft,
          p.pages.marginTop,
          p.pages.marginRight,
          p.pages.marginBottom,
        ],
        ...(p.pages.password ? { password: p.pages.password } : {}),
      };
      return { cmd: COMMANDS.pageOps, payload };
    }
    case "decorate": {
      const payload: DecorateRequest = {
        jobId,
        file: first,
        output: p.decorate.output,
        watermark: p.decorate.watermark,
        opacity: p.decorate.opacity,
        angle: p.decorate.angle,
        watermarkSize: p.decorate.watermarkSize,
        header: p.decorate.header,
        footer: p.decorate.footer,
        pageNumbers: p.decorate.pageNumbers,
        numberPosition: p.decorate.numberPosition,
        title: p.decorate.title,
        author: p.decorate.author,
        subject: p.decorate.subject,
        keywords: p.decorate.keywords,
        userPassword: p.decorate.userPassword,
        ownerPassword: p.decorate.ownerPassword,
        allowPrint: p.decorate.allowPrint,
        allowCopy: p.decorate.allowCopy,
        allowModify: p.decorate.allowModify,
        ...(p.decorate.password ? { password: p.decorate.password } : {}),
      };
      return { cmd: COMMANDS.decorate, payload };
    }
    case "export": {
      const payload: ExportRequest = {
        jobId,
        file: first,
        output: p.export.output,
        format: p.export.format,
        ...(p.export.pages ? { pages: p.export.pages } : {}),
        ...(p.export.password ? { password: p.export.password } : {}),
      };
      return { cmd: COMMANDS.export, payload };
    }
    case "objects": {
      const o = p.objects;
      const pw = o.password ? { password: o.password } : {};
      if (o.mode === "form") {
        // 每行：页码|名称|x|y|宽|高|默认值|多行(0/1)|必填(0/1)
        const fields: FormFieldSpec[] = o.fieldsSpec
          .split(/\r?\n/)
          .map((line) => line.trim())
          .filter(Boolean)
          .map((line) => {
            const s = line.split("|").map((x) => x.trim());
            return {
              name: s[1] ?? "",
              page: Number(s[0] ?? 1),
              x: Number(s[2] ?? 72),
              y: Number(s[3] ?? 700),
              width: Number(s[4] ?? 200),
              height: Number(s[5] ?? 24),
              default: s[6] ?? "",
              multiline: s[7] === "1",
              required: s[8] === "1",
            };
          });
        const payload: FormRequest = {
          jobId,
          file: first,
          output: o.output,
          fields,
          ...pw,
        };
        return { cmd: COMMANDS.createForm, payload };
      }
      if (o.mode === "redact") {
        const payload: RedactRequest = {
          jobId,
          file: first,
          output: o.output,
          terms: o.terms
            .split(/\r?\n|,/)
            .map((s) => s.trim())
            .filter(Boolean),
          matchCase: o.matchCase,
          ...(o.pages ? { pages: o.pages } : {}),
          ...pw,
        };
        return { cmd: COMMANDS.redact, payload };
      }
      const payload: BookmarkRequest = {
        jobId,
        file: first,
        output: o.output,
        action: o.action,
        spec: o.spec,
        target: o.target,
        newTitle: o.newTitle,
        ...pw,
      };
      return { cmd: COMMANDS.manageBookmarks, payload };
    }
    case "create": {
      const payload: TextToPdfRequest = {
        jobId,
        file: first,
        output: p.create.output,
        format: p.create.format,
      };
      return { cmd: COMMANDS.textToPdf, payload };
    }
    case "share": {
      const payload: ShareRequest = {
        jobId,
        file: first,
        action: p.share.action,
        subject: p.share.subject,
        body: p.share.body,
      };
      return { cmd: COMMANDS.share, payload };
    }
    default:
      throw new Error("未知功能");
  }
}

function currentOutputPath(tool: ToolId, p: ParamsState): string {
  switch (tool) {
    case "merge":
      return p.merge.output;
    case "split":
      return p.split.outputDir;
    case "compress":
      return p.compress.output;
    case "convert":
      return p.convert.direction === "toImages" ? p.convert.outputDir : p.convert.output;
    case "ocr":
      return p.ocr.output;
    case "sign":
      return p.sign.output;
    case "batch":
      return p.batch.outputDir;
    case "edit":
      return p.edit.output;
    case "annotate":
      return p.annotate.output;
    case "pages":
      return p.pages.output;
    case "decorate":
      return p.decorate.output;
    case "export":
      return p.export.output;
    case "objects":
      return p.objects.output;
    case "create":
      return p.create.output;
    case "share":
      return ""; // 打印/邮件不产出新文件
    default:
      return "";
  }
}

/** F19：框选替换的输出与源文件同目录，名字加 _edited，绝不覆盖输入。 */
function editedOutputPath(src: string): string {
  const m = /^(.*)(\.[^./\\]+)$/.exec(src);
  return m ? `${m[1]}_edited${m[2]}` : `${src}_edited.pdf`;
}

/**
 * F19 连续编辑的输出目标。
 *
 * 改完一处后预览切到 `xxx_edited.pdf`，用户接着框选别处时，输入已经是这份结果，
 * 输出必须还是它自己——否则每改一次就从原件重来，上一处改动会被丢掉。
 * 所以：源的 stem 已是 `_edited` 就原样返回（后端只允许覆盖 `*_edited`，
 * 用户原件永远不会被写）；否则给出第一次的 `_edited` 名字。
 */
function chainedOutputPath(src: string, own: Set<string>): string {
  // 只认本次会话自己生成的结果文件。用户拿到的文件可能本来就叫 xxx_edited.pdf，
  // 靠文件名判断会覆盖他的原件。
  if (own.has(src)) return src;
  return editedOutputPath(src);
}

export default function App() {
  const [activeTool, setActiveTool] = useState<ToolId>("merge");
  const [files, setFiles] = useState<FileItem[]>([]);
  const [params, setParams] = useState<ParamsState>(createDefaultParams);
  const [status, setStatus] = useState<JobStatus>("idle");
  const [progress, setProgress] = useState<ProgressState>({ done: 0, total: 0, message: "" });
  const [statusText, setStatusText] = useState("就绪");
  const [outputs, setOutputs] = useState<string[]>([]);
  // F18：任务成功后把输出 PDF 直接送进预览画布，改完立刻能看到结果页。
  const [resultFile, setResultFile] = useState<{ path: string; name: string } | null>(null);
  // F19 连续编辑：结果可能原地更新（路径不变），用一个自增计数让预览强制重载。
  const [reloadToken, setReloadToken] = useState(0);
  // 当前结果文件里，哪一份是本会话 F19 自己产出的（可继续框选、可原地累积）。
  const [chainableResultPath, setChainableResultPath] = useState<string | null>(null);
  // 本次会话由本工具生成的 _edited 产物绝对路径；只有它们允许被原地继续修改。
  // 用 ref 而不是 state：判断发生在点击回调里，不需要触发重渲染。
  const ownEditedRef = useRef<Set<string>>(new Set());
  const [dialog, setDialog] = useState<DialogView | null>(null);
  const [license, setLicense] = useState<LicenseState | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [logging, setLoggingState] = useState(false);
  const [updateSettings, setUpdateSettingsState] = useState<UpdateSettings>(() => getUpdateSettings());
  // 浅 / 暗双主题：跟随系统，可手动覆盖；偏好只存在本机 localStorage，不上传。
  const [theme, setTheme] = useState<"light" | "dark">(() => {
    const saved = localStorage.getItem("payoncepdf-theme");
    if (saved === "light" || saved === "dark") return saved;
    return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  });

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("payoncepdf-theme", theme);
  }, [theme]);

  // 2026-11-21 布局反馈：文档画布被上下挤压。左栏 / 右栏 / 底部文件区改为可收起，
  // 收起后网格列宽归零（不是简单 visibility），剩余空间全部给画布。
  // 「文件区」默认收起：没有文件时强制展开（总得能导入），有文件后让位给文档。
  // leftW/rightW 为 null 表示「跟随默认（含窄窗口的媒体查询）」；拖过手柄后就是像素值。
  const LEFT_W = { min: 160, max: 380, def: 214 } as const;
  const RIGHT_W = { min: 260, max: 520, def: 336 } as const;
  const clampW = (w: number, min: number, max: number) =>
    Math.min(max, Math.max(min, Math.round(w)));
  type LayoutPrefs = {
    left: boolean; right: boolean; dock: boolean;
    leftW: number | null; rightW: number | null;
  };
  const [layout, setLayout] = useState<LayoutPrefs>(() => {
    try {
      const raw = localStorage.getItem("payoncepdf-layout");
      if (raw) {
        const v = JSON.parse(raw) as Partial<LayoutPrefs>;
        return {
          left: v.left !== false,
          right: v.right !== false,
          dock: v.dock === true,
          leftW: typeof v.leftW === "number" ? clampW(v.leftW, LEFT_W.min, LEFT_W.max) : null,
          rightW: typeof v.rightW === "number" ? clampW(v.rightW, RIGHT_W.min, RIGHT_W.max) : null,
        };
      }
    } catch {
      // 存的是坏数据就回默认，绝不因为读配置把界面挂掉
    }
    return { left: true, right: true, dock: false, leftW: null, rightW: null };
  });
  useEffect(() => {
    try {
      localStorage.setItem("payoncepdf-layout", JSON.stringify(layout));
    } catch {
      // 隐私模式 / 磁盘满：记不住就记不住，不影响使用
    }
  }, [layout]);
  const togglePanel = useCallback((key: keyof Pick<LayoutPrefs, "left" | "right" | "dock">) => {
    setLayout((prev) => ({ ...prev, [key]: !prev[key] }));
  }, []);

  // 左右栏拖拽调宽：真机反馈「还是不能缩放」——之前只做了收起，没做拖拽改宽。
  // 拖动手柄实时改网格列宽，双击恢复默认，方向键 ±24px 微调；松手后的值记进 localStorage。
  const [resizing, setResizing] = useState(false);
  const dragRef = useRef<{ side: "left" | "right"; startX: number; startW: number } | null>(null);
  const onResizeStart = useCallback(
    (side: "left" | "right") => (e: ReactPointerEvent<HTMLDivElement>) => {
      if (!layout[side] || e.button !== 0) return;
      dragRef.current = {
        side,
        startX: e.clientX,
        startW: (side === "left" ? layout.leftW : layout.rightW) ?? (side === "left" ? LEFT_W.def : RIGHT_W.def),
      };
      setResizing(true);
      // 合成指针 / 已释放的 pointerId 会抛 NotFoundError；抓不到就把移动事件
      // 留在手柄自身上（手柄宽 9px，快速拖动可能丢帧，但不会崩）
      try {
        e.currentTarget.setPointerCapture(e.pointerId);
      } catch {
        // 忽略：不影响拖拽主流程
      }
    },
    [layout.left, layout.right, layout.leftW, layout.rightW],
  );
  const onResizeMove = useCallback((e: ReactPointerEvent<HTMLDivElement>) => {
    const d = dragRef.current;
    if (!d) return;
    const dx = e.clientX - d.startX;
    if (d.side === "left") {
      setLayout((p) => ({ ...p, leftW: clampW(d.startW + dx, LEFT_W.min, LEFT_W.max) }));
    } else {
      // 右栏在窗口右侧，鼠标左移 = 变宽
      setLayout((p) => ({ ...p, rightW: clampW(d.startW - dx, RIGHT_W.min, RIGHT_W.max) }));
    }
  }, []);
  const onResizeEnd = useCallback(() => {
    dragRef.current = null;
    setResizing(false);
  }, []);
  const resetWidth = useCallback((side: "left" | "right") => {
    setLayout((p) => ({ ...p, [side === "left" ? "leftW" : "rightW"]: null }));
  }, []);
  const nudgeWidth = useCallback((side: "left" | "right", delta: number) => {
    setLayout((p) => {
      const lim = side === "left" ? LEFT_W : RIGHT_W;
      const cur = (side === "left" ? p.leftW : p.rightW) ?? lim.def;
      const key = side === "left" ? "leftW" : "rightW";
      return { ...p, [key]: clampW(cur + delta, lim.min, lim.max) };
    });
  }, []);

  const jobIdRef = useRef<string | null>(null);
  const cancelledRef = useRef(false);

  const addItems = useCallback((items: InputItem[]) => {
    if (items.length === 0) return;
    setResultFile(null); // 新输入加入后回到输入预览，避免停留在上一次的输出文件
    setFiles((prev) => {
      const next = [...prev];
      items.forEach((item, i) => {
        const name = item.path.split(/[\\/]/).pop() || item.path;
        next.push({
          id: `${Date.now()}-${i}-${Math.random().toString(36).slice(2)}`,
          name,
          path: item.path,
          size: item.size,
          status: "ready",
        });
      });
      return next;
    });
  }, []);

  const handleFolder = useCallback(
    (dir: string) => {
      if (activeTool === "batch") {
        setParams((prev) => ({ ...prev, batch: { ...prev.batch, inputDir: dir } }));
        setStatusText(`已设置批量输入目录：${dir}`);
      } else {
        addItems([{ path: dir, size: 0 }]);
      }
    },
    [activeTool, addItems],
  );

  const removeFile = useCallback((id: string) => {
    setFiles((prev) => prev.filter((f) => f.id !== id));
  }, []);

  const moveFile = useCallback((id: string, direction: -1 | 1) => {
    setFiles((prev) => {
      const i = prev.findIndex((f) => f.id === id);
      const j = i + direction;
      if (i < 0 || j < 0 || j >= prev.length) return prev;
      const next = [...prev];
      const tmp = next[i];
      next[i] = next[j];
      next[j] = tmp;
      return next;
    });
  }, []);

  const clearFiles = useCallback(() => setFiles([]), []);

  const sortByName = useCallback(() => {
    setFiles((prev) => [...prev].sort((a, b) => a.name.localeCompare(b.name)));
  }, []);

  function patchParams<T extends ToolId>(tool: T, patch: Partial<ParamsState[T]>): void {
    setParams((prev) => ({ ...prev, [tool]: { ...prev[tool], ...patch } } as ParamsState));
  }

  const paramOnChange: ParamOnChange = {
    merge: (p) => patchParams("merge", p),
    split: (p) => patchParams("split", p),
    compress: (p) => patchParams("compress", p),
    convert: (p) => patchParams("convert", p),
    ocr: (p) => patchParams("ocr", p),
    sign: (p) => patchParams("sign", p),
    batch: (p) => patchParams("batch", p),
    edit: (p) => patchParams("edit", p),
    annotate: (p) => patchParams("annotate", p),
    pages: (p) => patchParams("pages", p),
    decorate: (p) => patchParams("decorate", p),
    export: (p) => patchParams("export", p),
    objects: (p) => patchParams("objects", p),
    create: (p) => patchParams("create", p),
    share: (p) => patchParams("share", p),
  };

  /**
   * F18：预览画布框选 → 把「选中的文字 + 页码」带进对应工具的参数面板。
   * 这里只填参数、切工具，不绕过 execute() 外壳：授权、进度、原子提交、
   * 写后读回校验与底部「执行」按钮仍是唯一出口。唯一性守卫在 PdfPreview 内。
   */
  const applySelection = useCallback((action: SelectionAction) => {
    const label = action.find.length > 18 ? `${action.find.slice(0, 18)}…` : action.find;
    const pageStr = String(action.page);
    setResultFile(null);
    if (action.tool === "edit") {
      setActiveTool("edit");
      setParams((prev) => ({
        ...prev,
        edit: {
          ...prev.edit,
          find: action.find,
          replace: action.replace ?? "",
          pages: pageStr,
        },
      }));
      setStatusText(`已把第 ${action.page} 页选中的「${label}」填入改字；点底部「执行」开始，结果另存、原文件不变`);
    } else if (action.tool === "annotate") {
      setActiveTool("annotate");
      setParams((prev) => ({
        ...prev,
        annotate: {
          ...prev.annotate,
          find: action.find,
          kind: action.kind ?? prev.annotate.kind,
          pages: pageStr,
        },
      }));
      setStatusText(`已把第 ${action.page} 页选中的「${label}」填入批注；点底部「执行」开始`);
    } else {
      setActiveTool("objects");
      setParams((prev) => ({
        ...prev,
        objects: {
          ...prev.objects,
          mode: "redact",
          terms: action.find,
          pages: pageStr,
        },
      }));
      setStatusText(`已把第 ${action.page} 页选中的「${label}」填入脱敏；点底部「执行」开始`);
    }
  }, []);

  /**
   * 统一的任务启动外壳：授权检查、jobId、进度/取消、输出文件进结果预览。
   * F1–F19 所有写操作都从这里走，确保没有第二条绕过 execute() 的路径。
   */
  const startJob = async (
    build: (jobId: string) => { cmd: string; payload: CommandPayload },
    opts?: { keepPreview?: boolean; chainableResult?: boolean },
  ): Promise<string[] | null> => {
    if (license && !license.canExport) {
      setStatusText("试用已结束；文件仍在，激活后可导出。仅限制导出，不删除文件。");
      return null;
    }
    const jobId = crypto.randomUUID();
    jobIdRef.current = jobId;
    cancelledRef.current = false;
    setStatus("running");
    setStatusText("");
    setOutputs([]);
    // F19 连续编辑：预览正在显示的文件就是本次要改的文件，中途不能退回输入，
    // 否则预览会重新加载输入文件并把页码重置为 1，用户每改一处就被弹回首页。
    if (!opts?.keepPreview) setResultFile(null);
    setProgress({ done: 0, total: 0, message: "准备中…" });
    try {
      const { cmd, payload } = build(jobId);
      const result = await invokeCommand(cmd, payload);
      if (cancelledRef.current || jobIdRef.current !== jobId) return null;
      setProgress((prev) => ({
        done: prev.total || 1,
        total: prev.total || 1,
        message: "",
      }));
      setOutputs(result.outputs);
      setStatus(result.ok ? "done" : "error");
      setStatusText(result.message);
      // F18 闭环：输出是 PDF 时自动切到结果预览，用户可立刻目视核对改后的那一页。
      const firstOutput = result.ok ? result.outputs[0] : undefined;
      if (firstOutput && /\.pdf$/i.test(firstOutput)) {
        setResultFile({
          path: firstOutput,
          name: firstOutput.split(/[\\/]/).pop() || firstOutput,
        });
        // 与 setResultFile 同一次批量更新，保证「结果可继续框选」和「文件已切换」
        // 在同一帧生效：晚一帧会让预览先按普通结果处理（重置页码、禁用框选）。
        if (opts?.chainableResult) setChainableResultPath(firstOutput);
        // 路径可能没变（连续改字原地更新），必须靠计数触发重新渲染。
        setReloadToken((n) => n + 1);
      }
      const nextStatus: FileItem["status"] = result.ok ? "done" : "error";
      setFiles((prev) => prev.map((f) => ({ ...f, status: nextStatus })));
      return result.ok ? result.outputs : null;
    } catch (e) {
      if (cancelledRef.current || jobIdRef.current !== jobId) return null;
      const appErr = normalizeError(e);
      setStatus("error");
      setStatusText(hintForCode(appErr.code));
      const nextStatus: FileItem["status"] = "error";
      setFiles((prev) =>
        prev.map((f) => ({ ...f, status: nextStatus, note: appErr.code })),
      );
      return null;
    }
  };

  const runJob = async () => {
    if (activeTool !== "batch" && files.length === 0) {
      setStatusText("请先添加至少一个文件");
      return;
    }
    if (activeTool === "batch" && (!params.batch.inputDir || !params.batch.outputDir)) {
      setStatusText("请先指定输入目录与输出目录");
      return;
    }
    await startJob((jobId) => buildRequest(activeTool, params, files, jobId));
  };

  /**
   * F19：框选直接改这一处。按坐标定位，不做全文查找，因此同页重复文字不受影响。
   * 与 F10 共用 execute() 外壳、授权、原子提交与写后读回校验。
   */
  const runRectEdit = useCallback(
    async (sel: RectEditSelection) => {
      // 必须与预览显示的是同一个文件：矩形是在预览页上框出来的，
      // 拿另一份文件去改会坐标错位。改完预览会切到 *_edited.pdf，
      // 第二次框选就应当接着改那份结果，而不是回到原始文件。
      const src = resultFile?.path ?? files[0]?.path;
      if (!src) {
        setStatusText("请先添加一个 PDF 文件");
        return;
      }
      if (sel.replace.length === 0) {
        setStatusText("请先在「改成」里填入替换后的文字");
        return;
      }
      const inPlace = ownEditedRef.current.has(src);
      const outs = await startJob((jobId) => {
        const payload: RectEditRequest = {
          jobId,
          file: src,
          output: chainedOutputPath(src, ownEditedRef.current),
          ...(inPlace ? { inPlace: true } : {}),
          page: sel.page,
          left: sel.rect.left,
          bottom: sel.rect.bottom,
          right: sel.rect.right,
          top: sel.rect.top,
          replace: sel.replace,
          ...(sel.expected ? { expected: sel.expected } : {}),
        };
        return { cmd: COMMANDS.editTextAtRect, payload };
      }, { keepPreview: true, chainableResult: true });
      // 后端可能因重名把输出改成 xxx_edited_1.pdf，必须以实际输出为准记账。
      // 后端可能因重名把输出改成 xxx_edited_1.pdf，必须以实际输出为准记账，
      // 下一轮判断能否原地累积（inPlace）靠的就是这个集合。
      const made = outs?.[0];
      if (made) ownEditedRef.current.add(made);
    },
    // startJob 只依赖 license 与若干 setState，闭包内不读取其他可变状态
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [files, license, resultFile],
  );

  const cancel = async () => {
    const jobId = jobIdRef.current;
    if (!jobId) return;
    cancelledRef.current = true;
    jobIdRef.current = null;
    setStatus("cancelled");
    setStatusText("已取消，原文件未改动");
    setProgress({ done: 0, total: 0, message: "" });
    await cancelJob(jobId);
  };

  const openFolder = async () => {
    const target = outputs.length > 0 ? outputs[0] : currentOutputPath(activeTool, params);
    const note = await openOutputFolder(target);
    if (note) setStatusText(note);
  };

  const handleActivate = async (key: string) => {
    const next = await activateLicense(key);
    setLicense(next);
    setStatusText(next.status === "active" ? "激活成功，可继续导出" : "授权状态已更新");
  };

  const handleLogging = (value: boolean) => {
    setLoggingState(value);
    void setLogging(value).catch(() => {});
  };

  const handleUpdateEnabled = (value: boolean) => {
    setUpdateSettingsState(setUpdateEnabled(value));
    setStatusText(value ? "已开启更新检查" : "已关闭更新检查；不会再发起更新请求");
  };

  useEffect(() => {
    let alive = true;
    getAppInfo()
      .then((v) => {
        if (alive) setAppInfo(v);
      })
      .catch(() => {});
    getLicenseState()
      .then((v) => {
        if (alive) setLicense(v);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    subscribeProgress((ev: ProgressEvent) => {
      if (jobIdRef.current && ev.jobId === jobIdRef.current && !cancelledRef.current) {
        setProgress({ done: ev.done, total: ev.total, message: ev.message });
      }
    }).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Esc：优先关闭对话框，运行中则取消任务。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (dialog) {
        setDialog(null);
        return;
      }
      if (status === "running") void cancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dialog, status]);

  const hasInput =
    activeTool === "batch"
      ? Boolean(params.batch.inputDir && params.batch.outputDir)
      : files.length > 0;
  const fileNames = files.map((f) => f.name);
  // 没有文件时必须展开导入区，否则用户找不到入口；有文件时尊重用户选择。
  const dockVisible = layout.dock || files.length === 0;

  return (
    <div className="app">
      <PreviewBanner />
      <header className="topbar">
        <div className="topbar-path">
          <span className="topbar-crumb">工作台</span>
          <span className="topbar-sep">/</span>
          <strong>{ALL_TOOLS.find((t) => t.id === activeTool)?.label ?? "工具"}</strong>
          <span className="topbar-hint">{ALL_TOOLS.find((t) => t.id === activeTool)?.hint ?? ""}</span>
        </div>
        <div className="topbar-actions">
          <span className="hero-line">Your files never leave this computer.</span>
          <div className="layout-toggles" role="group" aria-label="布局显示">
            <button
              type="button"
              className={`layout-toggle${layout.left ? " is-on" : ""}`}
              data-layout-toggle="left"
              aria-pressed={layout.left}
              title={layout.left ? "隐藏左侧工具栏" : "显示左侧工具栏"}
              onClick={() => togglePanel("left")}
            >工具</button>
            <button
              type="button"
              className={`layout-toggle${layout.right ? " is-on" : ""}`}
              data-layout-toggle="right"
              aria-pressed={layout.right}
              title={layout.right ? "隐藏右侧参数面板" : "显示右侧参数面板"}
              onClick={() => togglePanel("right")}
            >参数</button>
            <button
              type="button"
              className={`layout-toggle${dockVisible ? " is-on" : ""}`}
              data-layout-toggle="dock"
              aria-pressed={dockVisible}
              title={dockVisible ? "收起文件与导入区" : "展开文件与导入区"}
              onClick={() => togglePanel("dock")}
            >文件{files.length > 0 ? ` ${files.length}` : ""}</button>
          </div>
          <span className="topbar-status"><i className="status-dot" aria-hidden="true" />本机处理 · 文件不上传</span>
          <button
            type="button"
            className="theme-toggle"
            aria-label="切换浅色或深色主题"
            onClick={() => setTheme((t) => (t === "dark" ? "light" : "dark"))}
          >{theme === "dark" ? "浅色" : "深色"}</button>
        </div>
      </header>
      <div
        className={
          "app-body"
          + (layout.left ? "" : " is-left-hidden")
          + (layout.right ? "" : " is-right-hidden")
        }
        data-resizing={resizing ? "1" : undefined}
        style={
          {
            // 用户拖过宽度才覆盖默认值；没拖过留给 CSS（含窄窗口媒体查询）管。
            // 收起时必须输出 0px：内联样式优先级高于 .is-left-hidden 的规则，
            // 否则拖过宽度的栏收不起来。
            ...(layout.leftW != null
              ? { "--left-w": layout.left ? `${layout.leftW}px` : "0px" }
              : null),
            ...(layout.rightW != null
              ? { "--right-w": layout.right ? `${layout.rightW}px` : "0px" }
              : null),
          } as CSSProperties
        }
      >
        <div
          className="col-resizer col-resizer-left"
          role="separator"
          aria-orientation="vertical"
          aria-label="调整左侧工具栏宽度"
          title="拖动调宽 / 调窄；双击恢复默认"
          tabIndex={0}
          onPointerDown={onResizeStart("left")}
          onPointerMove={onResizeMove}
          onPointerUp={onResizeEnd}
          onPointerCancel={onResizeEnd}
          onDoubleClick={() => resetWidth("left")}
          onKeyDown={(e) => {
            if (e.key === "ArrowLeft") { nudgeWidth("left", -24); e.preventDefault(); }
            if (e.key === "ArrowRight") { nudgeWidth("left", 24); e.preventDefault(); }
          }}
        />
        <Sidebar
          active={activeTool}
          onSelect={setActiveTool}
          onOpen={setDialog}
          onCollapse={() => togglePanel("left")}
        />

        <main className="workspace">
          <PdfPreview
            filePath={resultFile?.path ?? files[0]?.path ?? null}
            fileName={resultFile?.name ?? files[0]?.name ?? null}
            resultMode={Boolean(resultFile)}
            selectableInResult={Boolean(resultFile && chainableResultPath === resultFile.path)}
            reloadToken={reloadToken}
            onBackToInput={() => {
              setResultFile(null);
              // 回到原件后再改，是从原件重新开始，会另存为新结果（旧结果保留），
              // 不再累积到之前那份 _edited 上。说清楚，别让用户以为改动丢了。
              if (ownEditedRef.current.size > 0) {
                setStatusText(
                  "已回到原始文件。继续框选修改会另存为新结果，之前的 _edited 文件保留、不会被覆盖。",
                );
              }
            }}
            onApplySelection={applySelection}
            onRectEdit={runRectEdit}
            onUsePage={(n) => {
              setParams((prev) => {
                const cur = prev[activeTool as keyof ParamsState] as unknown as
                  | Record<string, unknown>
                  | undefined;
                if (!cur || !("pages" in cur)) return prev;
                return { ...prev, [activeTool]: { ...cur, pages: String(n) } } as ParamsState;
              });
              setStatusText(`已把第 ${n} 页填入右侧参数面板`);
            }}
          />
          {dockVisible ? (
            <section className="dock" aria-label="文件与导入">
              <div className="dock-head">
                <span className="dock-title">文件与导入</span>
                <span className="dock-meta">{files.length} 个文件</span>
                {files.length > 0 ? (
                  <button
                    type="button"
                    className="dock-collapse"
                    title="收起这一区，把高度让给文档（顶栏「文件」可再展开）"
                    onClick={() => togglePanel("dock")}
                  >收起 ▾</button>
                ) : null}
              </div>
              <DropZone
                tool={activeTool}
                fileCount={files.length}
                onAddPaths={addItems}
                onFolder={handleFolder}
                compact
              />
              <FileList
                files={files}
                tool={activeTool}
                onRemove={removeFile}
                onMove={moveFile}
                onClear={clearFiles}
              />
            </section>
          ) : null}
        </main>

        <ParamPanel
          tool={activeTool}
          params={params}
          onChange={paramOnChange}
          fileCount={files.length}
          fileNames={fileNames}
          filePaths={files.map((f) => f.path)}
          onSortByName={sortByName}
          onCollapse={() => togglePanel("right")}
        />

        <div
          className="col-resizer col-resizer-right"
          role="separator"
          aria-orientation="vertical"
          aria-label="调整右侧参数面板宽度"
          title="拖动调宽 / 调窄；双击恢复默认"
          tabIndex={0}
          onPointerDown={onResizeStart("right")}
          onPointerMove={onResizeMove}
          onPointerUp={onResizeEnd}
          onPointerCancel={onResizeEnd}
          onDoubleClick={() => resetWidth("right")}
          onKeyDown={(e) => {
            if (e.key === "ArrowLeft") { nudgeWidth("right", -24); e.preventDefault(); }
            if (e.key === "ArrowRight") { nudgeWidth("right", 24); e.preventDefault(); }
          }}
        />
      </div>

      <ProgressBar
        status={status}
        progress={progress}
        statusText={statusText}
        outputs={outputs}
        canExport={license?.canExport ?? true}
        preview={!IS_TAURI}
        hasInput={hasInput}
        onRun={runJob}
        onCancel={cancel}
        onOpenFolder={openFolder}
      />

      {dialog === "about" ? (
        <AboutDialog
          info={appInfo}
          logging={logging}
          onLoggingChange={handleLogging}
          updateSettings={updateSettings}
          onUpdateEnabledChange={handleUpdateEnabled}
          onClose={() => setDialog(null)}
        />
      ) : null}
      {dialog === "license" ? (
        <LicensePanel
          state={license}
          onActivate={handleActivate}
          onClose={() => setDialog(null)}
        />
      ) : null}
      {dialog === "network" ? <NetworkPanel onClose={() => setDialog(null)} /> : null}
    </div>
  );
}
