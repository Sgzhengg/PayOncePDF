// CONTRACT.md §4 —— 逐字实现前端 TypeScript 类型。
// 所有命令以单个 payload 参数调用：invoke(cmd, { payload })。
// 所有请求均带 jobId；所有命令返回 JobResult；失败返回 AppError。

export type ToolId =
  | "merge"
  | "split"
  | "compress"
  | "convert"
  | "ocr"
  | "sign"
  | "batch"
  | "edit"
  | "annotate"
  | "pages"
  | "decorate"
  | "export"
  | "objects"
  | "compare"
  | "create"
  | "share";

export interface AppInfo {
  version: string;
  platform: string;
  arch: string;
  sidecars: { name: string; ready: boolean; path: string; arch: string }[];
  offlineCapable: boolean;
  telemetry: false;
  preview: boolean;
}

export interface JobResult {
  jobId: string;
  ok: boolean;
  outputs: string[];
  message: string;
  reportPath?: string;
  elapsedMs?: number;
}

export interface AppError {
  code: string;
  message: string;
}

export interface ProgressEvent {
  jobId: string;
  done: number;
  total: number;
  message: string;
}

// F9：检查更新走 @tauri-apps/plugin-updater（前端插件），开关持久化在 localStorage。
export interface UpdateSettings {
  enabled: boolean;
  lastCheckedAt?: number;
}

export interface UpdateCheckResult {
  available: boolean;
  version?: string;
  notes?: string;
  error?: string;
  checkedAt: number;
}

export interface LicenseState {
  status: "trial" | "active" | "expired";
  trialDaysLeft: number;
  activatedAt?: number;
  canExport: boolean;
  keyHint?: string;
}

export interface MergeRequest {
  jobId: string;
  files: string[];
  output: string;
  password?: string;
  keepBookmarks: boolean;
}

export interface SplitRequest {
  jobId: string;
  file: string;
  password?: string;
  outputDir: string;
  mode: "everyN" | "ranges" | "bookmarks";
  n?: number;
  ranges?: string;
  nameTemplate: string; // 默认 {name}_{index}.pdf
}

export interface CompressRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  targetBytes: number;
  quality: "auto" | "high" | "balanced" | "small";
}

export interface ToImagesRequest {
  jobId: string;
  file: string;
  password?: string;
  outputDir: string;
  format: "png" | "jpg";
  dpi: 72 | 150 | 300;
  pages?: string;
}

export interface ImagesToPdfRequest {
  jobId: string;
  files: string[];
  output: string;
  pageSize: "auto" | "a4" | "letter";
  margin: number;
}

export interface OcrRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  outputFormat: "pdf" | "txt";
  languages: string[];
  pages?: string;
}

export interface SignRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  signaturePath: string;
  page: number;
  x: number;
  y: number;
  width: number;
  height: number;
  fields: Record<string, string>;
  flattenFields: boolean;
}

export interface BatchRequest {
  jobId: string;
  inputDir: string;
  outputDir: string;
  operation:
    | "compress"
    | "ocr"
    | "toImages"
    | "merge"
    | "edit"
    | "annotate"
    | "export"
    | "decorate"
    | "pages";
  options: Record<string, unknown>;
}

// ---- F10–F14：对标 PDFgear 桌面版补齐的命令（CONTRACT §3/§4） ----

export interface EditTextRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  find: string;
  replace: string;
  matchCase: boolean;
  wholeWord: boolean;
  pages?: string;
}

export type AnnotationKind = "highlight" | "underline" | "strikeout" | "squiggly" | "note";

/** F19 按矩形精确替换：rect 为 PDF point、左下原点，只改框选那一处。 */
export interface RectEditRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  page: number;
  left: number;
  bottom: number;
  right: number;
  top: number;
  replace: string;
  /** 预览到的选中文字；只用于发现「预览后被改过」的错位，不参与定位。 */
  expected?: string;
  /**
   * 连续编辑：输出就是本次会话生成的 `*_edited` 结果文件，允许原地累积。
   * 只有前端自己记录的产物才置真，后端还会再校验文件名。
   */
  inPlace?: boolean;
}

export interface AnnotateRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  find: string;
  kind: AnnotationKind;
  color: string; // #RRGGBB
  author: string;
  note: string;
  matchCase: boolean;
  pages?: string;
}

export type PageOpMode = "rotate" | "delete" | "extract" | "insert" | "crop";

export interface PageOpsRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  mode: PageOpMode;
  pages: string;
  degrees: number;
  insertAt: number;
  insertCount: number;
  margins: number[]; // [左, 上, 右, 下]，单位 pt
}

export interface DecorateRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  watermark: string;
  opacity: number;
  angle: number;
  watermarkSize: number;
  header: string;
  footer: string;
  pageNumbers: boolean;
  numberPosition: "footerCenter" | "footerRight" | "headerRight";
  title: string;
  author: string;
  subject: string;
  keywords: string;
  userPassword: string;
  ownerPassword: string;
  allowPrint: boolean;
  allowCopy: boolean;
  allowModify: boolean;
}

export type ExportFormat = "txt" | "html" | "docx" | "xlsx";

export interface ExportRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  format: ExportFormat;
  pages?: string;
}

// ---- F15：表单 / 脱敏 / 书签 ----

export interface FormFieldSpec {
  name: string;
  page: number;
  x: number;
  y: number;
  width: number;
  height: number;
  default: string;
  multiline: boolean;
  required: boolean;
}

export interface FormRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  fields: FormFieldSpec[];
}

export interface RedactRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  terms: string[];
  matchCase: boolean;
  pages?: string;
}

export interface BookmarkEntry {
  level: number;
  title: string;
  page: number;
}

export interface BookmarkQuery {
  file: string;
  password?: string;
}

export interface BookmarkRequest {
  jobId: string;
  file: string;
  password?: string;
  output: string;
  action: "set" | "add" | "rename" | "delete";
  spec: string;
  target: string;
  newTitle: string;
}

// ---- F20：文档比对（纯文字层，只读） ----

export interface CompareSensitivity {
  ignoreCase: boolean;
  ignoreWhitespace: boolean;
  ignorePunctuation: boolean;
}

export interface CompareQuery {
  oldFile: string;
  newFile: string;
  oldPassword?: string;
  newPassword?: string;
  pages?: string;
  sensitivity?: CompareSensitivity;
}

export interface CompareDifference {
  kind: "added" | "removed" | "changed";
  page: number;
  oldText: string;
  newText: string;
}

export interface CompareReport {
  oldPages: number;
  newPages: number;
  comparedPages: number;
  added: number;
  removed: number;
  changed: number;
  differences: CompareDifference[];
  warnings: string[];
}

export interface CompareExportRequest {
  jobId: string;
  query: CompareQuery;
  output: string;
}

// ---- F16：文本转 PDF / 打印邮件 / OCR 语言包 ----

export interface TextToPdfRequest {
  jobId: string;
  file: string;
  output: string;
  format: "txt" | "csv" | "html";
}

export interface ShareRequest {
  jobId: string;
  file: string;
  action: "print" | "email";
  subject: string;
  body: string;
}

export interface LanguageRequest {
  lang: string;
}

export interface OcrLanguageInfo {
  code: string;
  label: string;
  installed: boolean;
}

// ---- F17 内置阅读视图（UPDF 对标 L1）-------------------------------------
export interface PdfInfoRequest {
  file: string;
  password?: string;
}

export interface PdfInfo {
  pages: number;
  pageWidthPt: number;
  pageHeightPt: number;
  openedWithPassword: boolean;
}

export interface PreviewRequest {
  file: string;
  page: number;
  dpi?: number;
  password?: string;
}

export interface PreviewImage {
  pngBase64: string;
  width: number;
  height: number;
  page: number;
  pages: number;
}

// ---- F18 画布框选修订联动（主用途决策 §5 的 P0 交互闭环）-----------------
export interface PageTextLayoutRequest {
  file: string;
  page: number;
  password?: string;
}

/** PDF point、页面左下原点；与后端 `engine::render::TextCharBox` 镜像。 */
export interface TextCharBox {
  index: number;
  ch: string;
  left: number;
  bottom: number;
  right: number;
  top: number;
}

export interface PageTextLayout {
  page: number;
  pages: number;
  pageWidthPt: number;
  pageHeightPt: number;
  chars: TextCharBox[];
  /** 按 chars 顺序拼接的整页文字层，用于唯一性守卫。 */
  text: string;
  /** 超过 6 万字符：chars/text 为空，必须禁用框选。 */
  truncated: boolean;
  /** 扫描件、纯图片页或超 6 万字符时为 false；旋转页字符框已变换到显示坐标，可为 true。 */
  hasTextLayer: boolean;
  /** 页面 /Rotate（0 / 90 / 180 / 270）；非 0 时字符框已变换到显示坐标系。 */
  rotation: number;
  /** 不可框选的明确原因；可框选时字段不存在。 */
  unsupportedReason?: string;
}

export type SelectionTool = "edit" | "annotate" | "redact";

/** 从预览画布把「选中的文字 + 页码」带进右侧参数面板。 */
export interface SelectionAction {
  tool: SelectionTool;
  find: string;
  page: number;
  replace?: string;
  kind?: "highlight" | "underline" | "strikeout" | "squiggly" | "note";
}
