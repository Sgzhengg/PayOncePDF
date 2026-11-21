import type { ToolId } from "./types";

// 中间文件列表项。path 在 Tauri 下为真实路径；预览模式下仅为文件名（没有真实路径）。
export interface FileItem {
  id: string;
  name: string;
  path: string;
  size: number;
  status: "ready" | "done" | "error" | "skipped";
  pages?: number;
  note?: string;
}

// 拖放 / 选择加入的输入项。预览模式能拿到浏览器 File 的真实大小；
// Tauri 对话框只返回路径，size 为 0（列表显示 "-"）。
export interface InputItem {
  path: string;
  size: number;
}

export interface ToolMeta {
  id: ToolId;
  label: string;
  hint: string;
}

export const ALL_TOOLS: ToolMeta[] = [
  { id: "merge", label: "合并", hint: "多个 PDF 合并为一个" },
  { id: "split", label: "拆分", hint: "按页数 / 范围 / 书签拆分" },
  { id: "compress", label: "压缩", hint: "压缩到指定大小" },
  { id: "convert", label: "转换", hint: "PDF 与图片互转" },
  { id: "ocr", label: "OCR", hint: "本地文字识别" },
  { id: "sign", label: "签名", hint: "签名图章与表单填写" },
  { id: "batch", label: "批量", hint: "整个文件夹批量处理" },
  { id: "edit", label: "改文字", hint: "就地替换文字，不转 Word" },
  { id: "annotate", label: "批注", hint: "高亮 / 下划线 / 便签" },
  { id: "pages", label: "页面", hint: "旋转 / 删除 / 抽取 / 裁剪" },
  { id: "decorate", label: "水印", hint: "水印 / 页码 / 属性 / 加密" },
  { id: "export", label: "导出", hint: "TXT / HTML / Word / Excel" },
  { id: "objects", label: "文档对象", hint: "表单 / 脱敏 / 书签" },
  { id: "compare", label: "文档比对", hint: "两版 PDF 文字层差异（新增 / 删除 / 修改）" },
  { id: "create", label: "文本转 PDF", hint: "TXT / CSV / HTML 含中文" },
  { id: "share", label: "打印与扩展", hint: "系统打印 / 邮件 / OCR 语言包" },
];

export interface MergeParams {
  output: string;
  password: string;
  keepBookmarks: boolean;
}

export interface SplitParams {
  password: string;
  outputDir: string;
  mode: "everyN" | "ranges" | "bookmarks";
  n: number;
  ranges: string;
  nameTemplate: string;
}

export interface CompressParams {
  password: string;
  output: string;
  targetMB: number;
  quality: "auto" | "high" | "balanced" | "small";
}

export interface ConvertParams {
  direction: "toImages" | "toPdf";
  password: string;
  outputDir: string;
  format: "png" | "jpg";
  dpi: 72 | 150 | 300;
  pages: string;
  output: string;
  pageSize: "auto" | "a4" | "letter";
  margin: number;
}

export interface OcrParams {
  password: string;
  output: string;
  outputFormat: "pdf" | "txt";
  languages: string[];
  pages: string;
}

export interface SignParams {
  password: string;
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

export interface BatchParams {
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
  options: Record<string, string>;
}

// ---- F10–F14 参数 ----

export interface EditParams {
  password: string;
  output: string;
  find: string;
  replace: string;
  matchCase: boolean;
  wholeWord: boolean;
  pages: string;
}

export interface AnnotateParams {
  password: string;
  output: string;
  find: string;
  kind: "highlight" | "underline" | "strikeout" | "squiggly" | "note";
  color: string;
  author: string;
  note: string;
  matchCase: boolean;
  pages: string;
}

export interface PagesParams {
  password: string;
  output: string;
  mode: "rotate" | "delete" | "extract" | "insert" | "crop";
  pages: string;
  degrees: number;
  insertAt: number;
  insertCount: number;
  marginLeft: number;
  marginTop: number;
  marginRight: number;
  marginBottom: number;
}

export interface DecorateParams {
  password: string;
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

export interface ExportParams {
  password: string;
  output: string;
  format: "txt" | "html" | "docx" | "xlsx";
  pages: string;
}

// ---- F15 / F16 参数 ----

export interface ObjectsParams {
  password: string;
  output: string;
  mode: "form" | "redact" | "bookmarks";
  /** 每行 `页码|名称|x|y|宽|高|默认值|多行(0/1)|必填(0/1)`，pt 坐标从左下原点算 */
  fieldsSpec: string;
  terms: string;
  matchCase: boolean;
  pages: string;
  action: "set" | "add" | "rename" | "delete";
  spec: string;
  target: string;
  newTitle: string;
}

// ---- F20：文档比对参数 ----
export interface CompareParams {
  /** 旧文件路径；留空则用文件列表第 1 个 */
  oldFile: string;
  /** 新文件路径；留空则用文件列表第 2 个 */
  newFile: string;
  oldPassword: string;
  newPassword: string;
  pages: string;
  ignoreCase: boolean;
  ignoreWhitespace: boolean;
  ignorePunctuation: boolean;
  output: string;
}

export interface CreateParams {
  output: string;
  format: "txt" | "csv" | "html";
}

export interface ShareParams {
  action: "print" | "email";
  subject: string;
  body: string;
  langCode: string;
}

export interface ParamsState {
  merge: MergeParams;
  split: SplitParams;
  compress: CompressParams;
  convert: ConvertParams;
  ocr: OcrParams;
  sign: SignParams;
  batch: BatchParams;
  edit: EditParams;
  annotate: AnnotateParams;
  pages: PagesParams;
  decorate: DecorateParams;
  export: ExportParams;
  objects: ObjectsParams;
  compare: CompareParams;
  create: CreateParams;
  share: ShareParams;
}

// 输出/目录给出可用的占位默认值：Tauri 下用户可改用对话框选择绝对路径，
// 预览模式下也可直接演示进度（不会真正写文件）。
export function createDefaultParams(): ParamsState {
  return {
    merge: { output: "merged.pdf", password: "", keepBookmarks: true },
    split: {
      password: "",
      outputDir: "output",
      mode: "everyN",
      n: 10,
      ranges: "1-5,8",
      nameTemplate: "{name}_{index}.pdf",
    },
    compress: { password: "", output: "compressed.pdf", targetMB: 5, quality: "balanced" },
    convert: {
      direction: "toImages",
      password: "",
      outputDir: "output",
      format: "png",
      dpi: 150,
      pages: "",
      output: "images.pdf",
      pageSize: "a4",
      margin: 10,
    },
    ocr: { password: "", output: "ocr.pdf", outputFormat: "pdf", languages: ["eng"], pages: "" },
    sign: {
      password: "",
      output: "signed.pdf",
      signaturePath: "",
      page: 1,
      x: 50,
      y: 50,
      width: 120,
      height: 40,
      fields: {},
      flattenFields: false,
    },
    batch: {
      inputDir: "input",
      outputDir: "output",
      operation: "compress",
      options: defaultBatchOptions("compress"),
    },
    edit: {
      password: "",
      output: "edited.pdf",
      find: "",
      replace: "",
      matchCase: false,
      wholeWord: false,
      pages: "",
    },
    annotate: {
      password: "",
      output: "annotated.pdf",
      find: "",
      kind: "highlight",
      color: "#FFD54F",
      author: "",
      note: "",
      matchCase: false,
      pages: "",
    },
    pages: {
      password: "",
      output: "pages.pdf",
      mode: "rotate",
      pages: "1",
      degrees: 90,
      insertAt: 1,
      insertCount: 1,
      marginLeft: 20,
      marginTop: 20,
      marginRight: 20,
      marginBottom: 20,
    },
    decorate: {
      password: "",
      output: "decorated.pdf",
      watermark: "",
      opacity: 0.25,
      angle: 45,
      watermarkSize: 48,
      header: "",
      footer: "",
      pageNumbers: true,
      numberPosition: "footerCenter",
      title: "",
      author: "",
      subject: "",
      keywords: "",
      userPassword: "",
      ownerPassword: "",
      allowPrint: true,
      allowCopy: true,
      allowModify: true,
    },
    export: { password: "", output: "export.docx", format: "docx", pages: "" },
    objects: {
      password: "",
      output: "objects.pdf",
      mode: "redact",
      fieldsSpec: "1|buyer|72|700|200|24|ACME|0|1",
      terms: "1234 5678",
      matchCase: false,
      pages: "",
      action: "set",
      spec: "1 封面\n2 第二章 交付\n3 附录",
      target: "",
      newTitle: "",
    },
    compare: {
      oldFile: "",
      newFile: "",
      oldPassword: "",
      newPassword: "",
      pages: "",
      ignoreCase: false,
      ignoreWhitespace: true,
      ignorePunctuation: false,
      output: "compare-report.txt",
    },
    create: { output: "created.pdf", format: "txt" },
    share: {
      action: "print",
      subject: "请查收 PDF",
      body: "附件由 PayOnce PDF 在本机生成。",
      langCode: "jpn",
    },
  };
}

// 批量选项随操作类型给出一组默认键值，用户可自行增删。
export function defaultBatchOptions(
  operation: BatchParams["operation"],
): Record<string, string> {
  switch (operation) {
    case "compress":
      return { targetMB: "5", quality: "balanced", password: "" };
    case "ocr":
      return { languages: "eng", outputFormat: "pdf", pages: "" };
    case "toImages":
      return { format: "png", dpi: "150", pages: "" };
    case "merge":
      return { keepBookmarks: "true", password: "" };
    case "edit":
      return { find: "", replace: "", matchCase: "false", pages: "" };
    case "annotate":
      return { find: "", kind: "highlight", color: "#FFD54F", pages: "" };
    case "export":
      return { format: "txt", pages: "" };
    case "decorate":
      return { watermark: "DRAFT", opacity: "0.25", pageNumbers: "true" };
    case "pages":
      return { mode: "rotate", pages: "1", degrees: "90" };
    default:
      return {};
  }
}
