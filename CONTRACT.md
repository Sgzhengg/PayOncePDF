# PayOnce PDF 前后端接口契约（M0 基准，所有开发必须遵守）

> 本文件是 `PRD.md` 第 7/13/16 节的机器可执行映射。前端、Rust 后端、CI、文档四个工作区都以此为准。
> 任何改动必须先改本文件，再改代码。**禁止**出现"前端调了后端没有的命令"或"命令名不一致"。

## 1. 通用约定

- Tauri 2 命令统一用 **单个 `payload` 参数**：`invoke("merge_pdfs", { payload })`，Rust 侧签名 `fn merge_pdfs(payload: MergeRequest, app: AppHandle) -> Result<JobResult, AppError>`。
- 所有**任务类**请求都带 `jobId: string`（前端用 `crypto.randomUUID()` 生成）；只读/工具型查询命令（`BookmarkQuery`、`EmptyPayload`、`LanguageRequest`）不带 `jobId`。
- 任务类命令返回 `JobResult`；失败返回 `Err(AppError)`，序列化为 `{ "code": "E1001", "message": "..." }`。六个例外：`list_bookmarks` → `Vec<BookmarkEntry>`、`list_ocr_languages` → `Vec<OcrLanguageInfo>`、`install_ocr_language` → `String`（成功消息文本）、`get_pdf_info` → `PdfInfo`、`render_page_preview` → `PreviewImage`、`get_page_text_layout` → `PageTextLayout`，失败仍返回 `AppError`。后三个是 **F17/F18 内置阅读视图与画布框选**的纯本地查询命令，不占导出授权、不发进度/结束事件、不联网、不落盘。
- 长任务通过事件 `payoncepdf://progress` 推送 `{ jobId, done, total, message }`；结束时推送 `payoncepdf://finished`（`JobResult`）。
- 取消：`cancel_job({ payload: { jobId } })`；后端用 `AtomicBool` 注册表，每个文件/每页处理后检查，取消时返回 `E2003`。
- 导出前所有命令必须调用 `license::ensure_export_allowed()`；试用到期时返回 `E2005`，**绝不删除或改写用户原文件**。
- 输出一律写到用户指定路径；绝不原地覆盖输入文件；临时文件写 `std::env::temp_dir()/payoncepdf-<jobId>/`，任务结束清理。

## 2. 错误码（PRD §13，扩展码以 E2xxx 标记且不改动原表）

| 码 | 含义 | 用户提示 |
|---|---|---|
| E1001 | 文件损坏 / 无法解析 | 文件无法读取，已跳过 |
| E1002 | 需要密码 | 该文件有密码，请输入 |
| E1003 | 密码错误 | 密码不正确 |
| E1004 | 磁盘空间不足 | 请清理磁盘后重试 |
| E1005 | 页数超限（>5000） | 单文件超过 5000 页，已拒绝 |
| E1006 | OCR 语言包缺失 | 请安装中文语言包 |
| E1007 | 输出被占用 | 目标文件正被其他程序使用 |
| E2001 | sidecar 缺失或架构不匹配 | 组件缺失，请重新安装或运行自检 |
| E2002 | PDFium 渲染引擎未加载 | 渲染组件缺失，请重新安装 |
| E2003 | 用户取消 | 已取消，原文件未改动 |
| E2004 | 内部错误（含参数非法） | 操作失败，原文件未改动 |
| E2005 | 试用到期，导出被限制 | 试用已结束；文件仍在，激活后可导出 |

## 3. 命令清单

| 命令 | Payload 类型 | 对应 PRD |
|---|---|---|
| `get_app_info` | `{}` → `AppInfo` | §8 自检 / §17 sidecar 自检 |
| `merge_pdfs` | `MergeRequest` → `JobResult` | F1 |
| `split_pdf` | `SplitRequest` → `JobResult` | F2 |
| `compress_pdf` | `CompressRequest` → `JobResult` | F3 |
| `pdf_to_images` | `ToImagesRequest` → `JobResult` | F4 |
| `images_to_pdf` | `ImagesToPdfRequest` → `JobResult` | F4 |
| `ocr_pdf` | `OcrRequest` → `JobResult` | F5 |
| `sign_pdf` | `SignRequest` → `JobResult` | F6 |
| `batch_run` | `BatchRequest` → `JobResult` | F7；`report.csv` 表头固定为 `filename,elapsed_ms,result,error_code,output`；批量操作扩展到 9 种（见 §10） |
| `replace_text` | `EditTextRequest` → `JobResult` | **F10** 就地替换文字（不重排） |
| `edit_text_at_rect` | `RectEditRequest` → `JobResult` | **F19** 按框选矩形精确替换（PDF pt、左下原点）：按坐标定位、不做全文查找，同页重复文字只改选中那一处；旋转页/跨行/字符层与内容流不一致一律拒绝且不写文件；另要求选区内字号一致（上下标整段框选会被拒，单独框上标可改）。支持连续编辑：同一会话内对同一份结果文件反复改（结果预览仍可框选，其它工具的输出则禁止框选），`inPlace=true` 时原地累积（仅允许 `*_edited` / `*_edited_N`，用户原件不可覆盖）。字符范围随原字体：英文/数字/中文（需内嵌）/Latin-1 符号（± ° µ × ÷）可写；Latin-1 之外的数学符号（≈ ≤ ≥ ∑ ∫ √ π）只有 PDF 内嵌含该字形的字体才可写 |
| `annotate_pdf` | `AnnotateRequest` → `JobResult` | **F11** 批注（5 种标准注解字典） |
| `manage_pages` | `PageOpsRequest` → `JobResult` | **F12** 旋转/删除/抽取/插入/裁剪 |
| `decorate_pdf` | `DecorateRequest` → `JobResult` | **F13** 水印/页眉页脚/页码/元数据/AES-256 |
| `export_pdf` | `ExportRequest` → `JobResult` | **F14** TXT/HTML/DOCX/XLSX 文字层导出 |
| `create_form_pdf` | `FormRequest` → `JobResult` | **F15** 创建 AcroForm 文本域 |
| `redact_pdf` | `RedactRequest` → `JobResult` | **F15** 真脱敏（删文字对象 + 黑框） |
| `list_bookmarks` | `BookmarkQuery` → `Vec<BookmarkEntry>` | **F15** 只读列出书签（含层级 `level`，不占导出授权、不发进度事件） |
| `manage_bookmarks` | `BookmarkRequest` → `JobResult` | **F15** 多级书签 set/add/rename/delete（spec 行首 `>` 表层级） |
| `text_to_pdf` | `TextToPdfRequest` → `JobResult` | **F16** TXT/CSV/HTML → PDF（纯文本排版，中文嵌入系统 CJK 字体） |
| `share_file` | `ShareRequest` → `JobResult` | **F16** 系统打印 / mailto 邮件（`outputs` 为空数组） |
| `list_ocr_languages` | `{}` → `Vec<OcrLanguageInfo>` | **F16** 只读列出 42 种语言及安装状态 |
| `install_ocr_language` | `{ lang }` → `String` | **F16** 用户主动下载静态字典；**全产品唯一运行时联网点** |
| `get_pdf_info` | `PdfInfoRequest` → `PdfInfo` | **F17** 只读查询页数与首页纸张尺寸；加密文档返回 E1002 |
| `render_page_preview` | `PreviewRequest` → `PreviewImage` | **F17** PDFium 单页渲染为 PNG base64（48–200 DPI），纯内存、不落盘、不联网、不上传 |
| `get_page_text_layout` | `PageTextLayoutRequest` → `PageTextLayout` | **F18** 单页字符级文字布局（PDF pt、左下原点，含整页文字层）；6 万字符上限；旋转页 / 扫描件返回 `hasTextLayer=false` + `unsupportedReason`，前端必须禁用框选 |
| `cancel_job` | `{ jobId }` → `null` | §16 取消 |
| `get_license_state` | `{}` → `LicenseState` | F8 |
| `activate_license` | `{ key }` → `LicenseState` | F8 |
| `set_logging` | `{ enabled }` → `null` | §6.3 |
| `open_output_folder` | `{ path }` → `null` | §16 打开输出文件夹 |
| ~~F9 命令~~ | **不走 Rust 命令**：`checkUpdate()` / `installUpdate()` / `setUpdateEnabled()` 在 `src/lib/backend.ts` 直接调用 `@tauri-apps/plugin-updater`（capability 已含 `updater:default`），开关持久化在 localStorage | F9 检查更新可关闭、签名校验、失败保留旧版本 |

## 4. TypeScript 类型（前端 `src/lib/types.ts`；Rust 用 serde 镜像，字段一律 snake_case → 前端 camelCase 由 Tauri 自动转换的**风险较高**，因此统一约定：**Rust 结构体字段全部用 camelCase，通过 `#[serde(rename_all = "camelCase")]` 由 `snake_case` Rust 字段自动映射**）

```ts
export type ToolId =
  | "merge" | "split" | "compress" | "convert" | "ocr" | "sign" | "batch"
  | "edit" | "annotate" | "pages" | "decorate" | "export"
  | "objects" | "create" | "share";   // 15 项（F15/F16 见 §11）
export interface AppInfo {
  version: string; platform: string; arch: string;
  sidecars: { name: string; ready: boolean; path: string; arch: string }[];
  offlineCapable: boolean; telemetry: false; preview: boolean;
}
export interface JobResult {
  jobId: string; ok: boolean; outputs: string[];
  message: string; reportPath?: string; elapsedMs?: number;
}
export interface AppError { code: string; message: string }
export interface ProgressEvent { jobId: string; done: number; total: number; message: string }
export interface LicenseState {
  status: "trial" | "active" | "expired";
  trialDaysLeft: number; activatedAt?: number; canExport: boolean; keyHint?: string;
}
export interface UpdateSettings { enabled: boolean; lastCheckedAt?: number }
export interface UpdateCheckResult {
  available: boolean; version?: string; notes?: string;
  error?: string; checkedAt: number;
}
export interface MergeRequest {
  jobId: string; files: string[]; output: string;
  password?: string; keepBookmarks: boolean;
}
export interface SplitRequest {
  jobId: string; file: string; password?: string; outputDir: string;
  mode: "everyN" | "ranges" | "bookmarks";
  n?: number; ranges?: string; nameTemplate: string; // 默认 {name}_{index}.pdf
}
export interface CompressRequest {
  jobId: string; file: string; password?: string; output: string;
  targetBytes: number; quality: "auto" | "high" | "balanced" | "small";
}
export interface ToImagesRequest {
  jobId: string; file: string; password?: string; outputDir: string;
  format: "png" | "jpg"; dpi: 72 | 150 | 300; pages?: string;
}
export interface ImagesToPdfRequest {
  jobId: string; files: string[]; output: string;
  pageSize: "auto" | "a4" | "letter"; margin: number;
}
export interface OcrRequest {
  jobId: string; file: string; password?: string; output: string;
  outputFormat: "pdf" | "txt"; languages: string[]; pages?: string;
}
export interface SignRequest {
  jobId: string; file: string; password?: string; output: string;
  signaturePath: string; page: number; x: number; y: number;
  width: number; height: number;
  fields: Record<string, string>; flattenFields: boolean;
}
export interface BatchRequest {
  jobId: string; inputDir: string; outputDir: string;
  operation: "compress" | "ocr" | "toImages" | "merge";
  options: Record<string, unknown>;
}
```

## 5. 前端适配层（`src/lib/backend.ts`）

- `export const IS_TAURI = "__TAURI_INTERNALS__" in window;`
- Tauri 环境：`invoke(cmd, { payload })`；浏览器预览环境：`mockBackend`（模拟进度事件与结果，**必须在 UI 上明确标注"预览模式：未调用本地引擎"**）。
- 文件选择用 `@tauri-apps/plugin-dialog` 的 `open({ multiple: true, filters: [...] })`；浏览器预览用 `<input type="file">`，只能拿到文件名不能拿到真实路径（预览模式下明示）。
- 拖放：Tauri 用 `getCurrentWindow().onDragDropEvent()`（payload.type === "drop" → `paths`）；浏览器用 HTML5 `onDrop`。
- 打开输出文件夹用 `openOutputFolder(path)`：Tauri 走 `invoke("open_output_folder")` 或 plugin-opener；预览模式提示"预览模式不支持"。

## 6. UI 结构（PRD §16，硬性）

- 左侧：**15 个功能入口**：`合并 / 拆分 / 压缩 / 转换 / OCR / 签名 / 批量 / 改文字 / 批注 / 页面 / 水印 / 导出 / 文档对象 / 文本转 PDF / 打印与扩展` + 底部"关于 / 授权 / 网络活动"。列表使用 `overflow-y: auto` 可独立滚动，不挤压底部信息区。
- 中间：拖拽区 + 文件列表（可排序、可删除、显示页数/大小/状态）。
- 右侧：参数面板，随功能切换（目标大小、DPI、语言、页码范围、文件模板等）。
- 底部：进度条 + 取消 + 打开输出文件夹 + 状态文字。
- 首屏一句话（英文原文，不可改写）：**"Your files never leave this computer."**
- 键盘可完成全部操作（可见 focus、Enter/Space 触发、Esc 取消）；支持 `prefers-contrast: more` 高对比度。
- 禁止出现：登录、上传、云、AI 聊天、订阅按钮、遥测、**高保真** Word/Excel 转换入口（PRD §19）。
  - 例外：F14「导出」与 F16「文本转 PDF」是**文字层**能力，必须在面板内明示「不还原版式/表格/图片」，不得宣称高保真 Office 转换；F16「打印与扩展」的语言包安装是唯一运行时联网点，必须在面板内明示（见 §11.4）。

## 7. Rust 后端模块划分

```
src-tauri/src/
├─ lib.rs            # tauri::generate_handler! + plugin 注册 + AppState
├─ error.rs          # AppError / 错误码 / From 映射
├─ license.rs        # F8 试用 + Ed25519 离线激活缓存 + ensure_export_allowed
├─ state.rs          # 取消注册表、进度事件发射、临时目录清理
├─ logging.rs        # 本地日志（默认关闭，仅操作类型与错误码）
├─ engine/
│  ├─ mod.rs
│  ├─ sidecar.rs     # 定位 qpdf/tesseract/pdfium/tessdata，三平台自检
│  ├─ qpdf.rs        # 合并/拆分/解密/加密(AES-256)/线性化 CLI 封装
│  ├─ render.rs      # PDFium 动态加载 → PNG/JPG 位图（暴露 load_pdfium 给 extras 复用）
│  ├─ pdf.rs         # lopdf：图片→PDF、压缩重编码、签名图章、表单填写/扁平化
│  ├─ ocr.rs         # tesseract CLI：可搜索 PDF / TXT
│  ├─ extras.rs      # F10–F14：改字/批注/页管理/装饰/导出 + 真实 PDF 文件级单测
│  ├─ objects.rs     # F15：AcroForm 文本域 / 真脱敏 / 多级书签
│  └─ more.rs        # F16：TXT/CSV/HTML→PDF、中文水印（系统 CJK 字体嵌入）
└─ commands/
   ├─ mod.rs
   ├─ jobs.rs        # F1–F7 + 9 种批量操作
   ├─ extras.rs      # F10–F14 五个命令（复用 jobs::execute；中文水印分支调 more）
   ├─ objects.rs     # F15 四个命令（导出类复用 execute；书签列表为只读）
   ├─ more.rs        # F16 四个命令（打印/邮件、语言包列表与安装）
   └─ app.rs         # get_app_info / license / logging / open_output_folder
```

`JobCtx.app` 为 `Option<&AppHandle>`：真机运行时由命令层注入 `Some(app)`，引擎级文件级单测可注入 `None` 并直接传入 `&Pdfium`，从而不依赖 Tauri runtime 即可验证真实 PDF 产物（`ctx.app()` 在离线模式下返回 E2004）。

## 8. 验收映射（PRD §7 / §14）

| 验收项 | 断言位置 |
|---|---|
| 合并 100 文件 ≤60s、页数之和 | `tests/acceptance.py` |
| 拆分 500 页 / 每 10 页 → 50 文件 | `tests/acceptance.py` |
| 压缩 20MB → ≤5MB、文字层仍在 | `tests/acceptance.py` |
| 300DPI 10 页 ≤30s、页序正确 | `tests/acceptance.py` |
| OCR 10 页 ≤60s、可搜索 PDF | `tests/acceptance.py` |
| 批量 200 文件、report.csv 错误码 | `tests/acceptance.py` |
| 断网可用、试用到期只限导出 | `tests/license_test.md` |
| 无上传、原文件不被修改 | `site/verify.html` + `docs/user-manual.md` |

## 9. 工程红线

1. 组件许可：只用 PDFium(BSD-3) / qpdf(Apache-2.0) / Tesseract(Apache-2.0) / lopdf / image(MIT|Apache-2.0)。禁止 MuPDF / Ghostscript。
2. 全程本地：**任何文件处理功能都不联网，文件与文件名永不上传**；运行时网络请求只有三类且可枚举、可抓包验证（见 §11.4）：一次性激活、可选更新检查、用户主动安装 OCR 语言包。不收集邮箱；不做遥测；日志默认关闭。**禁止再写「零网络请求」这类绝对表述**。
3. 三个平台目标：`x86_64-pc-windows-msvc`、`aarch64-apple-darwin`、`x86_64-apple-darwin`；macOS 合成 universal binary。
4. 失败不损坏原文件；任何写操作先写临时文件再原子替换。

---

## 10. F10–F14 补充契约（2026-11-16 范围变更，PRD §22）

### 10.1 请求体（Rust 字段 snake_case + `#[serde(rename_all = "camelCase")]`；前端全 camelCase）

```ts
export type ToolId =
  | "merge" | "split" | "compress" | "convert" | "ocr" | "sign" | "batch"
  | "edit" | "annotate" | "pages" | "decorate" | "export"
  | "objects" | "create" | "share";   // §11 补齐到 15 项

export interface EditTextRequest {
  jobId: string; file: string; password?: string; output: string;
  find: string; replace: string; matchCase: boolean; wholeWord: boolean; pages?: string;
}
export type AnnotationKind = "highlight" | "underline" | "strikeout" | "squiggly" | "note";
export interface AnnotateRequest {
  jobId: string; file: string; password?: string; output: string;
  find: string; kind: AnnotationKind; color: string; // #RRGGBB
  author: string; note: string; matchCase: boolean; pages?: string;
}
export type PageOpMode = "rotate" | "delete" | "extract" | "insert" | "crop";
export interface PageOpsRequest {
  jobId: string; file: string; password?: string; output: string;
  mode: PageOpMode; pages: string; degrees: number;
  insertAt: number; insertCount: number; margins: number[]; // [左,上,右,下] pt
}
export interface DecorateRequest {
  jobId: string; file: string; password?: string; output: string;
  watermark: string; opacity: number; angle: number; watermarkSize: number;
  header: string; footer: string; pageNumbers: boolean;
  numberPosition: "footerCenter" | "footerRight" | "headerRight";
  title: string; author: string; subject: string; keywords: string;
  userPassword: string; ownerPassword: string;
  allowPrint: boolean; allowCopy: boolean; allowModify: boolean;
}
export type ExportFormat = "txt" | "html" | "docx" | "xlsx";
export interface ExportRequest {
  jobId: string; file: string; password?: string; output: string;
  format: ExportFormat; pages?: string;
}
```

### 10.2 批量 `operation` 扩展

`BatchRequest.operation` 由 4 种扩展到 9 种：
`compress | ocr | toImages | merge | edit | annotate | export | decorate | pages`。
`report.csv` 表头与错误码行为完全不变；单文件失败仍只记录一行、不中断队列。

### 10.3 错误码使用（不新增码，复用 §2）

| 场景 | 码 |
|---|---|
| 输入不存在 / PDF 损坏 / 结构写入失败 | `E1001` |
| 源文件需要密码 / 密码错误 | `E1002` / `E1003` |
| 输出被占用 | `E1007` |
| PDFium / qpdf / tessdata 缺失 | `E2002` / `E2001` / `E1006` |
| 用户取消 | `E2003` |
| 试用到期 | `E2005` |
| 参数非法、页码越界、查找不到、颜色非法、字体缺字形、**页眉/页脚含中文**、系统 CJK 字体缺失、未知操作、语言代码不在白名单 | `E2004`（message 必须写明具体原因与可操作建议） |
| 语言包下载失败 / 下载内容过小 / 写用户目录失败 | `E2004`（不得改动已有语言包；OCR 现有功能不受影响） |

### 10.4 诚实边界（必须同步到 UI / FAQ / 用户手册）

1. F10 **不重排、不回流**；替换变长可能覆盖右侧内容；原字体缺新字形时**返回错误而非写乱码**；扫描件需先 OCR。
2. F11 只对**可选中文本**生效；写入标准 PDF 注解字典，非图像烫印。
3. F12 重建页树后嵌套书签可能不随页号更新；`crop` 只改 `/CropBox`，不删除内容。
4. F13 **水印文字支持中文**：含非拉丁字符时走 `more::cjk_watermark()`，用系统 CJK TrueType 字体嵌入并写进真实文本层（水平居中灰字；字号生效，角度/不透明度不生效；找不到系统 CJK 字体时明确报错，不写方框）。**页眉/页脚仍限 Helvetica/WinAnsi 拉丁字符**，输入中文明确报错。加密只设置用户提供的密码，不破解已有密码。
5. F14 **只导出文字层，不还原版式/表格/图片**；DOCX/XLSX 由本机自写 ZIP+OOXML 生成，不调用任何云服务；不得宣称「高保真转换」。

### 10.5 验证状态

第二轮已把验证从「编译通过」提升到 **真实 PDF 文件级**：`npm run build`（61 modules）/ `cargo check --lib` / `cargo test --lib -- --test-threads=1`（28 PASS / 0 FAIL，含 11 项真实 PDF + PDFium DLL + `qpdf --check` 端到端）/ UI 冒烟 20 PASS / 0 FAIL（15 入口，含中文水印与 42 种语言包文案断言）。
实证修好两个假成功：① F10 改字必须 `page.regenerate_content()` 否则对象级修改丢失，且 `FPDFText_SetText` 缺字形会静默写空格 → 增加写后读回校验 `verify_edit()`；② 随包 PDFium 在 `/Rotate != 0` 页面上文字提取返回空 → `with_unrotated()` 生成只把 `/Rotate` 置 0 的临时副本用于查找/导出，正式产物保留旋转。
**仍未做：真机 Tauri 端到端（GUI 内原生对话框/授权/命令链路）、注解在 Adobe Reader 显示、Word/Excel 打开 OOXML、AES-256 阅读器实测**，见 `docs/pdfgear-parity-matrix.md` §4。

---

## 11. F15 / F16 补充契约（2026-11-16 同日第二轮，PRD §22.7）

### 11.1 请求 / 响应体（Rust 字段 snake_case + `#[serde(rename_all = "camelCase")]`；前端全 camelCase）

```ts
// ---- F15：表单 / 脱敏 / 书签 ----
export interface FormFieldSpec {
  name: string; page: number; x: number; y: number;
  width: number; height: number; default: string;
  multiline: boolean; required: boolean;
}
export interface FormRequest {
  jobId: string; file: string; password?: string; output: string;
  fields: FormFieldSpec[];
}
export interface RedactRequest {
  jobId: string; file: string; password?: string; output: string;
  terms: string[]; matchCase: boolean; pages?: string;
}
export interface BookmarkEntry { level: number; title: string; page: number }
export interface BookmarkQuery { file: string; password?: string }
export interface BookmarkRequest {
  jobId: string; file: string; password?: string; output: string;
  action: "set" | "add" | "rename" | "delete";
  spec: string;      // 每行「页码 标题」，如 `3 第三章 交付验收`；标题可含空格
  target: string;    // rename/delete 时与现有书签标题完全一致
  newTitle: string;
}

// ---- F16：文本转 PDF / 打印邮件 / OCR 语言包 ----
export interface TextToPdfRequest {
  jobId: string; file: string; output: string;
  format: "txt" | "csv" | "html";
}
export interface ShareRequest {
  jobId: string; file: string; action: "print" | "email";
  subject: string; body: string;
}
export interface LanguageRequest { lang: string }
export interface OcrLanguageInfo { code: string; label: string; installed: boolean }
```

前端 `objects` 面板的 `fieldsSpec` 每行格式：`页码|名称|x|y|宽|高|默认值|多行(0/1)|必填(0/1)`（`App.tsx` 解析为 `FormFieldSpec[]`）；`terms` 按换行或逗号拆分。

### 11.2 命令行为差异（不得按任务类命令一刀切理解）

| 命令 | 走 `execute()` 外壳 | 输出 | 授权 / 事件 |
|---|---|---|---|
| `create_form_pdf` / `redact_pdf` / `manage_bookmarks` / `text_to_pdf` | 是 | 原子提交到 `output` | 校验导出授权、发进度/结束事件 |
| `share_file` | 是 | **无输出文件**（`outputs: []`），只返回提示文本 | 同上，但只调用系统默认程序 |
| `list_bookmarks` / `list_ocr_languages` | **否**（只读查询） | 直接返回数组 / 列表 | **不占导出授权、不发进度事件** |
| `install_ocr_language` | **否** | `String` 成功消息 | 不占导出授权；唯一运行时联网点 |

### 11.3 诚实边界（必须同步到 UI / FAQ / 用户手册 / 官网）

1. **F15 表单**：仅创建 AcroForm **文本域**（单行/多行/必填），写入 Widget 注解 + `/FT /Tx` + `/NeedAppearances true`（由阅读器生成外观）；**不覆盖既有表单字段**；无复选框/单选/下拉/计算脚本/签名域。
2. **F15 脱敏**：先真删除文字对象再盖黑框，**复制粘贴也取不到原文**；只处理可选中文本，扫描件需先 OCR；不是「只盖一层黑框」。
3. **F15 书签**：支持**多级书签**，spec 每行 `[>...]页码 标题`，行首 `>` 数量 = 层级（无 `>` 一级、`>` 二级、`>>` 三级），旧「页码 标题」格式仍是一级；层级跳跃（父级不存在）报错。`set/add/rename/delete` 用 parent 栈重建 `/Outlines`、**任何操作都不压平已有层级**；不支持界面拖放换父级；页面增删后书签**不自动跟随页号**；中文标题用 UTF-16BE + BOM 写入。
4. **F16 文本转 PDF**：TXT/CSV/HTML → A4 PDF，每页 45 行、11pt 自动分页；中文用**系统 CJK TrueType 字体嵌入**（Windows 优先 `C:/Windows/Fonts/simhei.ttf`）。HTML **只剥离标签，不执行脚本、不渲染 CSS**；找不到系统 CJK 字体时明确报错，不写方框/乱码。**纯文本排版，不还原 Office 版式、表格与图片**。
5. **F16 打印/邮件**：打印用 Windows `Start-Process -FilePath <path> -Verb Print` / macOS `lp`，**无应用内打印预览**；邮件为 `mailto:?subject=...&body=...` 预填主题与正文，**附件需在客户端手动添加**；两者都不上传文件。
6. **F16 中文水印**：见 §10.4 第 4 条（水平居中灰字，字号生效，角度/不透明度不生效）。
7. **F16 语言包**：42 种白名单语言；随包 `eng` + `chi_sim`，其余按需下载到用户目录。

### 11.4 唯一运行时联网点（隐私口径，禁止写成「零网络请求」）

`install_ocr_language({ lang })` 用包内 `ureq` 下载 `https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/<lang>.traineddata` 到 `app_data_dir/tessdata/`。三条硬约束：

1. **只在用户主动点击「安装语言包」时发生**；
2. URL 只含**语言代码**，不含文件名、路径、机器指纹或任何用户数据；请求头 UA 固定为 `PayOnce/1.0 (local OCR language pack installer)`；
3. 下载的是公开静态字典，**不调用云端处理或 AI 接口**；识别全程在本机。白名单校验语言代码、小于 50 KB 的响应丢弃、先写 `.tmp` 再替换（Windows 覆盖前先删除目标）。

**对外正确承诺**：「文件内容永不上传，文件与文件名都不发；所有网络请求可枚举且可抓包验证」。运行时请求共三类：一次性激活、可选更新检查、用户主动的语言包下载。**任何文件处理功能都不联网。** 与 PDFgear 的 AI 把 PDF 内容发往 Azure OpenAI 有本质区别。

### 11.5 验证状态

28 项 Rust 单测通过（含 11 项真实 PDF 文件级，覆盖改字/中文/缺字形/批注/旋转页/脱敏/四格式导出/中文文本转 PDF/中文水印/AcroForm 结构/书签往返）、`npm run build` 61 modules、UI 冒烟 20 PASS / 0 FAIL（15 入口；本轮新增「中文水印与页眉页脚边界已区分」「语言包清单 42 种」两项断言）。
**仍未做**：真机 Tauri GUI 端到端、Adobe Reader 显示注解与填写表单、Word/Excel 打开 OOXML、系统打印/邮件在真机唤起、`install_ocr_language` 真机联网下载与断网降级、macOS 分支（`lp` / `open`）。

