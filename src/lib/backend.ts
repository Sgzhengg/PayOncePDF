import type {
  AnnotateRequest,
  AppError,
  AppInfo,
  BatchRequest,
  BookmarkEntry,
  BookmarkQuery,
  BookmarkRequest,
  CompressRequest,
  DecorateRequest,
  EditTextRequest,
  ExportRequest,
  FormRequest,
  ImagesToPdfRequest,
  JobResult,
  LicenseState,
  MergeRequest,
  OcrLanguageInfo,
  PdfInfo,
  PageTextLayout,
  PreviewImage,
  OcrRequest,
  PageOpsRequest,
  ProgressEvent,
  RectEditRequest,
  RedactRequest,
  ShareRequest,
  SignRequest,
  SplitRequest,
  TextToPdfRequest,
  ToImagesRequest,
  UpdateCheckResult,
  UpdateSettings,
} from "./types";

// CONTRACT §5：Tauri 环境检测。所有 Tauri 模块一律动态 import，
// 保证浏览器预览时不会因静态导入桌面插件而报错。
export const IS_TAURI = "__TAURI_INTERNALS__" in window;

// 命令名集中管理，避免前端调用后端没有的命令或拼写不一致（CONTRACT §3）。
export const COMMANDS = {
  getAppInfo: "get_app_info",
  merge: "merge_pdfs",
  split: "split_pdf",
  compress: "compress_pdf",
  toImages: "pdf_to_images",
  toPdf: "images_to_pdf",
  ocr: "ocr_pdf",
  sign: "sign_pdf",
  batch: "batch_run",
  editText: "replace_text",
  editTextAtRect: "edit_text_at_rect",
  annotate: "annotate_pdf",
  pageOps: "manage_pages",
  decorate: "decorate_pdf",
  export: "export_pdf",
  createForm: "create_form_pdf",
  redact: "redact_pdf",
  listBookmarks: "list_bookmarks",
  manageBookmarks: "manage_bookmarks",
  textToPdf: "text_to_pdf",
  share: "share_file",
  listOcrLanguages: "list_ocr_languages",
  installOcrLanguage: "install_ocr_language",
  getPdfInfo: "get_pdf_info",
  renderPagePreview: "render_page_preview",
  getPageTextLayout: "get_page_text_layout",
  cancel: "cancel_job",
  getLicense: "get_license_state",
  activateLicense: "activate_license",
  setLogging: "set_logging",
  openOutputFolder: "open_output_folder",
} as const;

// UI 顶部常驻横幅文本。首句为硬性要求原文。
export const PREVIEW_BANNER_TEXT =
  "预览模式：未调用本地引擎，文件不会被处理。浏览器选择只能得到文件名、没有真实路径，输出也不会真正写入磁盘。";

// 15 个功能入口对应的长任务 payload 联合。invokeCommand 只用于这些任务命令；
// listBookmarks / listOcrLanguages / installOcrLanguage 是独立查询命令，走各自封装。
export type CommandPayload =
  | MergeRequest
  | SplitRequest
  | CompressRequest
  | ToImagesRequest
  | ImagesToPdfRequest
  | OcrRequest
  | SignRequest
  | BatchRequest
  | EditTextRequest
  | RectEditRequest
  | AnnotateRequest
  | PageOpsRequest
  | DecorateRequest
  | ExportRequest
  | FormRequest
  | RedactRequest
  | BookmarkRequest
  | TextToPdfRequest
  | ShareRequest;

// 命令返回 JobResult；mock 额外附带 preview 标记（不改动契约 §4 的 JobResult）。
export type CommandResult = JobResult & { preview?: boolean };

export interface FileFilter {
  name: string;
  extensions: string[];
}

export interface PickOptions {
  multiple?: boolean;
  accept?: string;
  title?: string;
  filters?: FileFilter[];
}

export const PDF_FILTERS: FileFilter[] = [{ name: "PDF 文件", extensions: ["pdf"] }];
export const IMAGE_FILTERS: FileFilter[] = [
  { name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"] },
];
export const INPUT_FILTERS: FileFilter[] = [...PDF_FILTERS, ...IMAGE_FILTERS];

// ---------------------------------------------------------------------------
// Tauri invoke 封装：统一 invoke(cmd, { payload })
// ---------------------------------------------------------------------------
async function tauriInvoke<T>(cmd: string, payload: object): Promise<T> {
  const core = await import("@tauri-apps/api/core");
  const args: Record<string, unknown> = { payload };
  return core.invoke<T>(cmd, args);
}

// ---------------------------------------------------------------------------
// 进度 / 结束事件（CONTRACT §1：payoncepdf://progress、payoncepdf://finished）
// ---------------------------------------------------------------------------
type ProgressCb = (event: ProgressEvent) => void;
type FinishedCb = (result: JobResult) => void;

const progressListeners = new Set<ProgressCb>();
const finishedListeners = new Set<FinishedCb>();

function emitProgress(event: ProgressEvent): void {
  for (const cb of progressListeners) cb(event);
}

function emitFinished(result: JobResult): void {
  for (const cb of finishedListeners) cb(result);
}

// 返回 Promise<unsubscribe>，App 在 useEffect 里注册与清理。
export async function subscribeProgress(cb: ProgressCb): Promise<() => void> {
  if (!IS_TAURI) {
    progressListeners.add(cb);
    return () => {
      progressListeners.delete(cb);
    };
  }
  const event = await import("@tauri-apps/api/event");
  return event.listen<ProgressEvent>("payoncepdf://progress", (ev) => cb(ev.payload));
}

export async function subscribeFinished(cb: FinishedCb): Promise<() => void> {
  if (!IS_TAURI) {
    finishedListeners.add(cb);
    return () => {
      finishedListeners.delete(cb);
    };
  }
  const event = await import("@tauri-apps/api/event");
  return event.listen<JobResult>("payoncepdf://finished", (ev) => cb(ev.payload));
}

// ---------------------------------------------------------------------------
// mockBackend：模拟分批进度；绝不伪造输出文件（outputs 恒为空数组）。
// ---------------------------------------------------------------------------
const cancelledJobs = new Set<string>();
let mockLicenseState: LicenseState = {
  status: "trial",
  trialDaysLeft: 7,
  canExport: true,
};

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

async function mockRun(payload: CommandPayload): Promise<CommandResult> {
  const jobId = payload.jobId;
  const total = 12;
  for (let done = 1; done <= total; done += 1) {
    await delay(150);
    if (cancelledJobs.has(jobId)) {
      cancelledJobs.delete(jobId);
      const cancelled: JobResult = {
        jobId,
        ok: false,
        outputs: [],
        message: "已取消，原文件未改动",
      };
      emitFinished(cancelled);
      return { ...cancelled, preview: true };
    }
    emitProgress({
      jobId,
      done,
      total,
      message: `预览模式：模拟进度 ${done}/${total}（未调用本地引擎）`,
    });
  }
  const result: CommandResult = {
    jobId,
    ok: true,
    outputs: [], // 不假装产生了真实输出文件
    message: "预览模式：未调用本地引擎，未产生任何输出文件。",
    preview: true,
  };
  emitFinished({ jobId, ok: true, outputs: [], message: result.message });
  return result;
}

// ---------------------------------------------------------------------------
// 命令调用
// ---------------------------------------------------------------------------
export async function invokeCommand(
  cmd: string,
  payload: CommandPayload,
): Promise<CommandResult> {
  if (!IS_TAURI) return mockRun(payload);
  return tauriInvoke<JobResult>(cmd, payload);
}

// 非 JobResult 的查询命令（预览模式返回静态/空数据，不伪造真实处理结果）。
const PREVIEW_LANGS: OcrLanguageInfo[] = [
  { code: "eng", label: "英语", installed: true },
  { code: "chi_sim", label: "简体中文", installed: true },
  { code: "jpn", label: "日语", installed: false },
  { code: "kor", label: "韩语", installed: false },
  { code: "fra", label: "法语", installed: false },
];

export async function listBookmarks(query: BookmarkQuery): Promise<BookmarkEntry[]> {
  if (!IS_TAURI) return [];
  return tauriInvoke<BookmarkEntry[]>(COMMANDS.listBookmarks, query);
}

export async function listOcrLanguages(): Promise<OcrLanguageInfo[]> {
  if (!IS_TAURI) return PREVIEW_LANGS;
  return tauriInvoke<OcrLanguageInfo[]>(COMMANDS.listOcrLanguages, {});
}

export async function installOcrLanguage(lang: string): Promise<string> {
  if (!IS_TAURI) return "预览模式不支持安装语言包（不会联网）";
  return tauriInvoke<string>(COMMANDS.installOcrLanguage, { lang });
}

export async function cancelJob(jobId: string): Promise<void> {
  if (!IS_TAURI) {
    cancelledJobs.add(jobId);
    return;
  }
  await tauriInvoke<null>(COMMANDS.cancel, { jobId });
}

// 返回提示字符串表示"预览模式不支持"；成功返回 null。
export async function openOutputFolder(path: string): Promise<string | null> {
  if (!IS_TAURI) {
    return "预览模式不支持打开输出文件夹";
  }
  await tauriInvoke<null>(COMMANDS.openOutputFolder, { path });
  return null;
}

export async function getAppInfo(): Promise<AppInfo> {
  if (!IS_TAURI) {
    return {
      version: "1.0.0 (preview)",
      platform: "browser",
      arch: "n/a",
      sidecars: [
        { name: "qpdf", ready: false, path: "-", arch: "-" },
        { name: "tesseract", ready: false, path: "-", arch: "-" },
        { name: "pdfium", ready: false, path: "-", arch: "-" },
      ],
      offlineCapable: true,
      telemetry: false,
      preview: true,
    };
  }
  return tauriInvoke<AppInfo>(COMMANDS.getAppInfo, {});
}

export async function getLicenseState(): Promise<LicenseState> {
  if (!IS_TAURI) return { ...mockLicenseState };
  return tauriInvoke<LicenseState>(COMMANDS.getLicense, {});
}

function maskKey(key: string): string {
  const parts = key.trim().toUpperCase().split("-");
  const last = parts[parts.length - 1] ?? "****";
  return `PAYONCE-****-****-${last}`;
}

export async function activateLicense(key: string): Promise<LicenseState> {
  if (!IS_TAURI) {
    if (!/^PAYONCE(-[A-Za-z0-9]{4}){3}$/i.test(key.trim())) {
      const err: AppError = { code: "E2004", message: "密钥格式不正确" };
      throw err;
    }
    mockLicenseState = {
      status: "active",
      trialDaysLeft: 0,
      activatedAt: Date.now(),
      canExport: true,
      keyHint: maskKey(key),
    };
    return { ...mockLicenseState };
  }
  return tauriInvoke<LicenseState>(COMMANDS.activateLicense, { key });
}

export async function setLogging(enabled: boolean): Promise<void> {
  if (!IS_TAURI) return;
  await tauriInvoke<null>(COMMANDS.setLogging, { enabled });
}

// ---------------------------------------------------------------------------
// F9 自动更新：直接走 @tauri-apps/plugin-updater（无需 Rust 命令）。
// 开关只存在本机 localStorage；除用户主动点击外不发起任何网络请求。
// ---------------------------------------------------------------------------
const UPDATE_ENABLED_KEY = "payoncepdf.updateEnabled";
const UPDATE_LAST_CHECK_KEY = "payoncepdf.updateLastCheckedAt";

export function getUpdateSettings(): UpdateSettings {
  let enabled = true;
  let lastCheckedAt: number | undefined;
  try {
    enabled = localStorage.getItem(UPDATE_ENABLED_KEY) !== "0";
    const raw = localStorage.getItem(UPDATE_LAST_CHECK_KEY);
    if (raw) lastCheckedAt = Number(raw) || undefined;
  } catch {
    // 隐私模式 / 禁用存储时退回默认值，不影响使用
  }
  return { enabled, lastCheckedAt };
}

export function setUpdateEnabled(enabled: boolean): UpdateSettings {
  try {
    localStorage.setItem(UPDATE_ENABLED_KEY, enabled ? "1" : "0");
  } catch {
    // 忽略存储不可用
  }
  return getUpdateSettings();
}

function rememberCheck(): void {
  try {
    localStorage.setItem(UPDATE_LAST_CHECK_KEY, String(Date.now()));
  } catch {
    // 忽略存储不可用
  }
}

function updateError(e: unknown): string {
  const msg = e instanceof Error ? e.message : String(e);
  return msg.includes("public key") || msg.includes("pubkey") || msg.includes("signature")
    ? "更新签名公钥尚未配置（发布前需替换 tauri.conf.json 的占位 pubkey）"
    : `检查更新失败：${msg}`;
}

export async function checkUpdate(): Promise<UpdateCheckResult> {
  const checkedAt = Date.now();
  if (!IS_TAURI) {
    return { available: false, checkedAt, error: "预览模式不支持检查更新" };
  }
  if (!getUpdateSettings().enabled) {
    return { available: false, checkedAt, error: "更新检查已关闭，可在「关于」中重新开启" };
  }
  rememberCheck();
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (update) {
      return {
        available: true,
        version: update.version,
        notes: update.body ?? undefined,
        checkedAt,
      };
    }
    return { available: false, checkedAt };
  } catch (e) {
    return { available: false, checkedAt, error: updateError(e) };
  }
}

/** 用户确认后才下载安装；签名校验由 Tauri Updater 负责，失败不会替换当前版本。 */
export async function installUpdate(): Promise<string> {
  if (!IS_TAURI) return "预览模式不支持安装更新";
  if (!getUpdateSettings().enabled) return "更新检查已关闭";
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) return "当前已是最新版本";
    await update.downloadAndInstall();
    return `已下载并安装 ${update.version}，重启 PayOnce PDF 后生效；旧版本文件保留，可回滚。`;
  } catch (e) {
    return `更新失败，当前版本未受影响：${updateError(e)}`;
  }
}

// ---------------------------------------------------------------------------
// 文件选择：Tauri 动态 import plugin-dialog；预览模式用 <input type="file">。
// ---------------------------------------------------------------------------
interface BrowserPickOptions {
  multiple: boolean;
  directory: boolean;
  accept?: string;
}

// 预览模式拿不到真实路径，只能返回文件名 / webkitRelativePath 的根目录名。
function browserPick(opts: BrowserPickOptions): Promise<string[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = opts.multiple;
    if (opts.directory) {
      input.setAttribute("webkitdirectory", "");
      input.setAttribute("directory", "");
    }
    if (opts.accept) input.accept = opts.accept;
    input.style.display = "none";
    let settled = false;
    const finish = (names: string[]): void => {
      if (settled) return;
      settled = true;
      input.remove();
      resolve(names);
    };
    input.addEventListener("change", () => {
      const files = Array.from(input.files ?? []);
      if (opts.directory) {
        const roots = new Set<string>();
        for (const f of files) {
          const rel = f.webkitRelativePath || f.name;
          roots.add(rel.split("/")[0] || f.name);
        }
        finish([...roots]);
      } else {
        finish(files.map((f) => f.name));
      }
    });
    input.addEventListener("cancel", () => finish([]));
    document.body.appendChild(input);
    input.click();
  });
}

export async function pickFiles(options?: PickOptions): Promise<string[]> {
  const multiple = options?.multiple ?? true;
  if (!IS_TAURI) {
    return browserPick({ multiple, directory: false, accept: options?.accept });
  }
  const dialog = await import("@tauri-apps/plugin-dialog");
  const result: unknown = await dialog.open({
    multiple,
    title: options?.title ?? "选择文件",
    filters: options?.filters ?? INPUT_FILTERS,
  });
  if (typeof result === "string") return [result];
  return Array.isArray(result) ? (result as string[]) : [];
}

export async function pickFolder(options?: { title?: string }): Promise<string | null> {
  if (!IS_TAURI) {
    const roots = await browserPick({
      multiple: false,
      directory: true,
    });
    return roots[0] ?? null;
  }
  const dialog = await import("@tauri-apps/plugin-dialog");
  const result: unknown = await dialog.open({
    directory: true,
    multiple: false,
    title: options?.title ?? "选择文件夹",
  });
  return typeof result === "string" ? result : null;
}

// 预览模式无原生保存对话框：返回 null，由 UI 提示并保留默认输出名。
export async function pickSave(options?: {
  defaultPath?: string;
  filters?: FileFilter[];
  title?: string;
}): Promise<string | null> {
  if (!IS_TAURI) return null;
  const dialog = await import("@tauri-apps/plugin-dialog");
  const result: unknown = await dialog.save({
    defaultPath: options?.defaultPath,
    filters: options?.filters ?? PDF_FILTERS,
    title: options?.title ?? "保存为",
  });
  return typeof result === "string" ? result : null;
}

// ---------------------------------------------------------------------------
// 拖放：Tauri 用 getCurrentWindow().onDragDropEvent()；预览用 HTML5 onDrop。
// ---------------------------------------------------------------------------
export async function subscribeDragDrop(cb: (paths: string[]) => void): Promise<() => void> {
  if (!IS_TAURI) return () => {};
  const win = await import("@tauri-apps/api/window");
  return win.getCurrentWindow().onDragDropEvent((event) => {
    if (event.payload.type === "drop") cb(event.payload.paths);
  });
}

// ---------------------------------------------------------------------------
// F17 内置阅读视图：两个纯本地查询命令。
// 浏览器预览模式返回 null —— 不伪造渲染结果、不假装读到文件（CONTRACT §5）。
// ---------------------------------------------------------------------------
export async function getPdfInfo(file: string, password?: string): Promise<PdfInfo | null> {
  if (!IS_TAURI) return null;
  return tauriInvoke<PdfInfo>(COMMANDS.getPdfInfo, { file, password: password ?? null });
}

export async function renderPagePreview(
  file: string,
  page: number,
  dpi: number,
  password?: string,
): Promise<PreviewImage | null> {
  if (!IS_TAURI) return null;
  return tauriInvoke<PreviewImage>(COMMANDS.renderPagePreview, {
    file,
    page,
    dpi,
    password: password ?? null,
  });
}

/**
 * F18：读取单页字符级文字布局（PDF pt、左下原点）。
 * 真机走 PDFium 进程内查询，纯内存、不落盘、不联网；浏览器预览返回 null，
 * 前端必须据此禁用框选，绝不伪造文字坐标。
 */
export async function getPageTextLayout(
  file: string,
  page: number,
  password?: string,
): Promise<PageTextLayout | null> {
  if (!IS_TAURI) return null;
  return tauriInvoke<PageTextLayout>(COMMANDS.getPageTextLayout, {
    file,
    page,
    password: password ?? null,
  });
}
