#!/usr/bin/env node
/**
 * provision-sidecars.mjs - PayOnce PDF sidecar 供给脚本（Node 22 ESM，仅用 Node 内置模块）
 *
 * 用途
 *   为 Tauri 2 打包准备 PRD 9.1 要求的三平台 sidecar：
 *     - qpdf / tesseract       -> Tauri externalBin 规范名（bin/<name>-<triple>[.exe]）
 *     - tessdata_fast 中英语言包 -> src-tauri/resources/tessdata/（bundle.resources: ["resources/*"]）
 *     - PDFium 动态库          -> src-tauri/resources/
 *   并生成 src-tauri/bin/sidecar-manifest.json（版本、来源、sha256、架构、是否签名），
 *   校验文件存在性与 CPU 架构；网络不可用或校验失败时以明确退出码失败，绝不静默成功。
 *
 * 用法
 *   node scripts/provision-sidecars.mjs --target <triple> [选项]
 *   node scripts/provision-sidecars.mjs --target x86_64-pc-windows-msvc --dry-run
 *   node scripts/provision-sidecars.mjs --target aarch64-apple-darwin --dry-run
 *   node scripts/provision-sidecars.mjs --target x86_64-apple-darwin --offline ./sidecar-cache
 *   node scripts/provision-sidecars.mjs --help
 *
 * 选项
 *   --target <triple>   必填（--help 除外）。支持：
 *                         x86_64-pc-windows-msvc / aarch64-apple-darwin / x86_64-apple-darwin
 *   --dry-run           只打印供给计划（来源 URL、许可、目标路径），不联网、不写文件。
 *   --offline <dir>     从本地目录拷贝兜底，不联网。目录内按文件名递归查找，支持
 *                         <dir>/<triple>/<file> 或 <dir>/<file> 两种布局：
 *                         qpdf[.exe]、tesseract[.exe]、eng.traineddata、
 *                         chi_sim.traineddata、pdfium.dll / libpdfium.dylib
 *   --help              显示帮助。
 *
 * 环境变量
 *   PAYONCEPDF_PROVISION_ROOT  覆盖目标根目录（默认仓库根），用于测试 / 预演，避免污染仓库。
 *
 * 退出码
 *   0 成功（含 --dry-run）
 *   2 参数错误（未知 target、缺 --target、--offline 目录不存在等）
 *   3 网络 / 下载错误（含上游 URL 未核实）
 *   4 解压 / 提取错误
 *   5 文件缺失 / 校验错误（存在性、空文件、sha256、externalBin 命名）
 *   6 架构不匹配（PE Machine / Mach-O cputype / lipo -archs）
 *
 * 重要：来源 URL 核实（TODO(verify-url)）
 *   下方 SOURCES 常量表里的直链均标注了 TODO(verify-url)：上游发布资源的
 *   版本号 / 文件名会随发布变化。启用 CI 在线下载前必须人工逐条核实；
 *   未核实或网络不可用时请使用 --offline <dir> 从本地缓存拷贝。
 *   许可红线（PRD 9.2）：qpdf(Apache-2.0)、Tesseract(Apache-2.0)、
 *   tessdata_fast(Apache-2.0)、PDFium(BSD-3-Clause) 允许；禁止 MuPDF / Ghostscript(AGPL)。
 *
 * 工程红线
 *   - 路径一律 path.join / path.resolve，不写死盘符；Windows / macOS 均可运行。
 *   - 不硬编码任何凭据；不生成 emoji；不执行需要真实证书/账号的操作。
 *   - macOS 侧 sip：brew 版 tesseract 依赖 brew 的 dylib，生产应改用静态链接构建或把依赖
 *     dylib 一并拷入 resources 并修正 rpath（见 SOURCES.tesseract.notes，TODO）。
 */

import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import zlib from 'node:zlib';
import crypto from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import process from 'node:process';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// 默认写到仓库根；可用 PAYONCEPDF_PROVISION_ROOT 覆盖（便于测试 / CI 预演，不污染仓库）。
const REPO_ROOT = process.env.PAYONCEPDF_PROVISION_ROOT
  ? path.resolve(process.env.PAYONCEPDF_PROVISION_ROOT)
  : path.resolve(__dirname, '..');
const BIN_DIR = path.join(REPO_ROOT, 'src-tauri', 'bin');
const RES_DIR = path.join(REPO_ROOT, 'src-tauri', 'resources');
const MANIFEST_PATH = path.join(BIN_DIR, 'sidecar-manifest.json');

// ---------------------------------------------------------------------------
// 集中来源常量表
// 每条直链都标记 TODO(verify-url)：请人工核实后再用于 CI 在线下载。
// ---------------------------------------------------------------------------
const GENERATOR_VERSION = 1;

const SOURCES = {
  qpdf: {
    license: 'Apache-2.0',
    homepage: 'https://github.com/qpdf/qpdf/releases',
    version: '11.9.1',
    notes: [
      'qpdf Windows mingw64 包的 qpdf.exe 依赖同目录 qpdf29.dll / libgcc_s_seh-1.dll /',
      'libstdc++-6.dll / libwinpthread-1.dll；单独部署 qpdf.exe 在干净机器上会因缺 DLL 失败。',
      'TODO: 需与负责 tauri.conf.json 的 Agent 协调，把这些 DLL 作为 resources 随包并放入 DLL 搜索路径，',
      '或改用静态链接构建；当前 bundle.externalBin 只声明了 bin/qpdf 与 bin/tesseract 两项。',
    ],
    byTarget: {
      'x86_64-pc-windows-msvc': {
        // 2026-11-16 实测有效：13MB zip，内含 bin/qpdf.exe 与 4 个依赖 DLL。
        url: 'https://github.com/qpdf/qpdf/releases/download/v11.9.1/qpdf-11.9.1-mingw64.zip',
        archive: 'zip',
        binPathInArchive: 'qpdf-11.9.1-mingw64/bin/qpdf.exe',
        todoVerifyUrl: false,
      },
      'aarch64-apple-darwin': { brew: 'qpdf', brewBin: 'bin/qpdf', todoVerifyUrl: false },
      'x86_64-apple-darwin': { brew: 'qpdf', brewBin: 'bin/qpdf', todoVerifyUrl: false },
    },
  },
  tesseract: {
    license: 'Apache-2.0',
    homepage: 'https://github.com/tesseract-ocr/tesseract',
    winHomepage: 'https://github.com/UB-Mannheim/tesseract/wiki',
    notes: [
      'macOS 用 brew 安装后拷贝；brew 版 tesseract 动态链接 libtesseract/liblept 等 dylib，',
      '直接拷单个可执行文件在干净机器上会因缺 dylib 失败（TODO: 生产改用静态构建，或把依赖',
      'dylib 一并拷入 resources 并用 install_name_tool 修正 rpath）。',
      'Windows 推荐 UB Mannheim 便携版 zip；安装包(.exe/NSIS)无法通用解压，见 --offline 兜底。',
    ],
    byTarget: {
      'x86_64-pc-windows-msvc': {
        // UB Mannheim 只发 NSIS 安装包（无法通用解压），故不伪造便携 zip URL。
        // 在线模式优先复用本机已安装的 Tesseract-OCR；CI 可先 `choco install tesseract -y`。
        url: null,
        localPaths: [
          'C:\\Program Files\\Tesseract-OCR\\tesseract.exe',
          'C:\\Program Files (x86)\\Tesseract-OCR\\tesseract.exe',
        ],
        localEnv: 'TESSERACT_HOME',
        archive: 'zip',
        binPathInArchive: 'tesseract.exe',
        todoVerifyUrl: true,
      },
      'aarch64-apple-darwin': { brew: 'tesseract', brewBin: 'bin/tesseract', todoVerifyUrl: false },
      'x86_64-apple-darwin': { brew: 'tesseract', brewBin: 'bin/tesseract', todoVerifyUrl: false },
    },
  },
  tessdata: {
    license: 'Apache-2.0',
    homepage: 'https://github.com/tesseract-ocr/tessdata_fast',
    files: {
      // 2026-11-16 实测 HTTP 200（tessdata_fast main 分支）。
      'eng.traineddata': {
        url: 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/eng.traineddata',
        todoVerifyUrl: false,
      },
      'chi_sim.traineddata': {
        url: 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/chi_sim.traineddata',
        todoVerifyUrl: false,
      },
    },
  },
  pdfium: {
    license: 'BSD-3-Clause',
    homepage: 'https://github.com/bblanchon/pdfium-binaries/releases',
    byTarget: {
      // 2026-11-16 实测有效：release tag `chromium/8152`，资源为 .tgz，
      // Windows 内 bin/pdfium.dll，macOS 内 lib/libpdfium.dylib。
      'x86_64-pc-windows-msvc': {
        url: 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8152/pdfium-win-x64.tgz',
        archive: 'tgz',
        libInArchive: 'bin/pdfium.dll',
        todoVerifyUrl: false,
      },
      'aarch64-apple-darwin': {
        url: 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8152/pdfium-mac-arm64.tgz',
        archive: 'tgz',
        libInArchive: 'lib/libpdfium.dylib',
        todoVerifyUrl: false,
      },
      'x86_64-apple-darwin': {
        url: 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8152/pdfium-mac-x64.tgz',
        archive: 'tgz',
        libInArchive: 'lib/libpdfium.dylib',
        todoVerifyUrl: false,
      },
    },
  },
};

// 所有需要人工核实的 URL 汇总（--dry-run 会打印，最终报告也会列出）
const TODO_URLS = Object.freeze([
  'SOURCES.tesseract.byTarget["x86_64-pc-windows-msvc"].url (UB Mannheim 只提供 NSIS 安装包，故意置 null；改用本机安装 / --offline)',
  'release tag chromium/8152 与 qpdf 11.9.1 会随上游发布变化，升级版本前需重新核实',
  'macOS brew 版 tesseract/qpdf 依赖 dylib，生产需静态构建或随包并修正 rpath',
]);

// ---------------------------------------------------------------------------
// 目标平台定义
// ---------------------------------------------------------------------------
const TARGETS = Object.freeze({
  'x86_64-pc-windows-msvc': {
    os: 'windows', arch: 'x86_64', exeSuffix: '.exe',
    pdfiumName: 'pdfium.dll', machoExpected: null, peMachine: 0x8664,
  },
  'aarch64-apple-darwin': {
    os: 'macos', arch: 'arm64', exeSuffix: '',
    pdfiumName: 'libpdfium.dylib', machoExpected: 'arm64', peMachine: null,
  },
  'x86_64-apple-darwin': {
    os: 'macos', arch: 'x86_64', exeSuffix: '',
    pdfiumName: 'libpdfium.dylib', machoExpected: 'x86_64', peMachine: null,
  },
});

// CPU 架构常量（Mach-O cputype）
const CPUTYPE = Object.freeze({
  0x01000007: 'x86_64',
  0x0100000c: 'arm64',
  0x00000007: 'i386',
});

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------
function log(msg) { process.stdout.write(msg + '\n'); }
function warn(msg) { process.stdout.write('[WARN] ' + msg + '\n'); }
function fail(code, msg) {
  process.stderr.write(`[ERROR][exit=${code}] ${msg}\n`);
  process.exit(code);
}
function sha256(buf) { return crypto.createHash('sha256').update(buf).digest('hex'); }
function rel(p) { return path.relative(REPO_ROOT, p).split(path.sep).join('/'); }

function parseArgs(argv) {
  const args = { target: null, dryRun: false, offline: null, help: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--help' || a === '-h') args.help = true;
    else if (a === '--dry-run') args.dryRun = true;
    else if (a === '--target') {
      if (i + 1 >= argv.length) fail(2, '--target 缺少参数');
      args.target = argv[++i];
    } else if (a === '--offline') {
      if (i + 1 >= argv.length) fail(2, '--offline 缺少目录参数');
      args.offline = argv[++i];
    } else {
      fail(2, `未知参数: ${a}（用 --help 查看用法）`);
    }
  }
  return args;
}

function help() {
  log(`provision-sidecars.mjs - PayOnce PDF sidecar 供给 (Node 22 ESM)`);
  log('');
  log('用法: node scripts/provision-sidecars.mjs --target <triple> [选项]');
  log('');
  log('  --target <triple>  x86_64-pc-windows-msvc | aarch64-apple-darwin | x86_64-apple-darwin');
  log('  --dry-run          只打印计划，不联网、不写文件');
  log('  --offline <dir>    从本地目录拷贝兜底，不联网');
  log('  --help             显示本帮助');
  log('');
  log('退出码: 0 成功 / 2 参数 / 3 网络 / 4 解压 / 5 校验 / 6 架构不匹配');
  log('');
  log('TODO(verify-url) 需人工核实的来源 URL:');
  for (const t of TODO_URLS) log('  - ' + t);
  log('');
  log('许可: qpdf/Tesseract/tessdata_fast = Apache-2.0; PDFium = BSD-3-Clause (PRD 9.2)。');
}

// ---------------------------------------------------------------------------
// 计划（units）
// ---------------------------------------------------------------------------
function buildPlan(target, t) {
  const units = [];
  const qpdfSrc = SOURCES.qpdf.byTarget[target];
  const tessSrc = SOURCES.tesseract.byTarget[target];
  const pdfiumSrc = SOURCES.pdfium.byTarget[target];

  units.push({
    name: 'qpdf', kind: 'externalBin', archCheck: true,
    dest: path.join(BIN_DIR, `qpdf-${target}${t.exeSuffix}`),
    offlineNames: [`qpdf${t.exeSuffix}`, 'qpdf'],
    source: { ...qpdfSrc, component: 'qpdf', license: SOURCES.qpdf.license,
              homepage: SOURCES.qpdf.homepage, version: SOURCES.qpdf.version },
  });
  units.push({
    name: 'tesseract', kind: 'externalBin', archCheck: true,
    dest: path.join(BIN_DIR, `tesseract-${target}${t.exeSuffix}`),
    offlineNames: [`tesseract${t.exeSuffix}`, 'tesseract'],
    source: { ...tessSrc, component: 'tesseract', license: SOURCES.tesseract.license,
              homepage: SOURCES.tesseract.homepage },
  });
  for (const [fname, meta] of Object.entries(SOURCES.tessdata.files)) {
    units.push({
      name: fname, kind: 'resource', archCheck: false,
      dest: path.join(RES_DIR, 'tessdata', fname),
      offlineNames: [fname],
      source: { ...meta, component: `tessdata/${fname}`,
                license: SOURCES.tessdata.license, homepage: SOURCES.tessdata.homepage },
    });
  }
  units.push({
    name: 'pdfium', kind: 'resource', archCheck: true,
    dest: path.join(RES_DIR, t.pdfiumName),
    offlineNames: [t.pdfiumName],
    source: { ...pdfiumSrc, component: 'pdfium', license: SOURCES.pdfium.license,
              homepage: SOURCES.pdfium.homepage },
  });
  return units;
}

function printPlan(target, t, units, args) {
  log('=== provision-sidecars 计划（--dry-run，不联网 / 不写文件）===');
  log(`target            : ${target}  (os=${t.os} arch=${t.arch})`);
  log(`repo root         : ${REPO_ROOT}`);
  log(`bin dir           : ${rel(BIN_DIR)}`);
  log(`resources dir     : ${rel(RES_DIR)}`);
  log(`manifest          : ${rel(MANIFEST_PATH)}`);
  log(`mode              : ${args.offline ? 'offline (' + args.offline + ')' : 'online (fetch)'}`);
  log('bundle.externalBin: bin/qpdf, bin/tesseract');
  log('bundle.resources  : resources/*');
  log('');
  for (const u of units) {
    log(`- [${u.kind}] ${u.name}`);
    log(`    dest          : ${rel(u.dest)}`);
    log(`    license       : ${u.source.license}`);
    log(`    homepage      : ${u.source.homepage}`);
    if (u.source.brew) {
      log(`    source        : brew install ${u.source.brew} -> ${u.source.brewBin}`);
    } else if (u.source.url === null) {
      log(`    source        : URL 未核实 (null) -> 在线模式会报错；请用 --offline 或核实后填入`);
    } else {
      log(`    source        : ${u.source.url}`);
    }
    if (u.source.archive) log(`    archive       : ${u.source.archive} (${u.source.binPathInArchive || u.source.libInArchive})`);
    if (u.source.todoVerifyUrl) log(`    TODO(verify-url): 上游 URL 未核实，启用在线下载前必须人工确认`);
    log(`    offline names : ${u.offlineNames.join(', ')}`);
    log(`    archCheck     : ${u.archCheck ? 'yes (' + (t.peMachine ? 'PE Machine' : 'Mach-O / lipo') + ')' : 'no (data file)'}`);
  }
  log('');
  log('TODO(verify-url) 汇总:');
  for (const t2 of TODO_URLS) log('  - ' + t2);
  log('');
  if (args.offline) {
    log(`--offline 目录检查: ${args.offline}`);
    for (const u of units) {
      const found = findOfflineFile(args.offline, u.offlineNames, target);
      log(`  ${found ? '[FOUND]  ' : '[MISSING]'} ${u.name}: ${found || '(未找到)'}`);
    }
  }
  log('');
  log('注: 在线下载失败 / URL 未核实 -> 退出码 3；架构不符 -> 6；绝不静默成功。');
}

// ---------------------------------------------------------------------------
// --offline 兜底：递归按文件名查找
// ---------------------------------------------------------------------------
function walk(dir, cb, depth = 0) {
  if (depth > 8) return;
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); }
  catch { return; }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) walk(full, cb, depth + 1);
    else if (e.isFile()) cb(full);
  }
}

function findOfflineFile(root, names, target) {
  const wanted = new Set(names.map((n) => n.toLowerCase()));
  const matches = [];
  walk(root, (full) => {
    if (wanted.has(path.basename(full).toLowerCase())) matches.push(full);
  });
  if (matches.length === 0) return null;
  // 优先命中 <triple> 子目录，其次路径最短者
  matches.sort((a, b) => {
    const at = a.split(path.sep).join('/').includes(target) ? 0 : 1;
    const bt = b.split(path.sep).join('/').includes(target) ? 0 : 1;
    if (at !== bt) return at - bt;
    return a.length - b.length;
  });
  return matches[0];
}

// ---------------------------------------------------------------------------
// 在线下载 / 解压
// ---------------------------------------------------------------------------
async function fetchBytes(url) {
  const attempts = 3;
  let lastErr;
  for (let i = 1; i <= attempts; i++) {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 60000);
    try {
      const res = await fetch(url, {
        signal: ctrl.signal,
        redirect: 'follow',
        headers: { 'User-Agent': 'payoncepdf-provision/1.0' },
      });
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`);
      const buf = Buffer.from(await res.arrayBuffer());
      if (buf.length === 0) throw new Error('空响应体');
      return buf;
    } catch (err) {
      lastErr = err;
      warn(`下载失败 (第 ${i}/${attempts} 次) ${url}: ${err.message}`);
    } finally {
      clearTimeout(timer);
    }
    if (i < attempts) await new Promise((r) => setTimeout(r, 500 * i));
  }
  fail(3, `下载失败（已重试 ${attempts} 次）: ${url} -> ${lastErr && lastErr.message}`);
}

/** 纯 Node 标准库 ZIP 解压（method 0 store / 8 deflate），带 zip-slip 防护。 */
function extractZip(buf, destDir) {
  let eocd = -1;
  const min = Math.max(0, buf.length - 65558);
  for (let i = buf.length - 22; i >= min; i--) {
    if (buf.readUInt32LE(i) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) fail(4, '不是合法 ZIP（未找到 EOCD）');
  const total = buf.readUInt16LE(eocd + 10);
  const cdOffset = buf.readUInt32LE(eocd + 16);
  const root = path.resolve(destDir);
  fs.mkdirSync(root, { recursive: true });
  let p = cdOffset;
  for (let n = 0; n < total; n++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) fail(4, `ZIP 中央目录损坏 @${p}`);
    const method = buf.readUInt16LE(p + 10);
    const compSize = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extraLen = buf.readUInt16LE(p + 30);
    const commentLen = buf.readUInt16LE(p + 32);
    const localOffset = buf.readUInt32LE(p + 42);
    const name = buf.toString('utf8', p + 46, p + 46 + nameLen);
    const target = path.resolve(root, name);
    if (target !== root && !target.startsWith(root + path.sep)) {
      fail(4, `ZIP 路径越界 (zip slip): ${name}`);
    }
    if (buf.readUInt32LE(localOffset) !== 0x04034b50) fail(4, `ZIP 本地头损坏: ${name}`);
    const lNameLen = buf.readUInt16LE(localOffset + 26);
    const lExtraLen = buf.readUInt16LE(localOffset + 28);
    const start = localOffset + 30 + lNameLen + lExtraLen;
    const comp = buf.subarray(start, start + compSize);
    if (name.endsWith('/')) {
      fs.mkdirSync(target, { recursive: true });
    } else {
      fs.mkdirSync(path.dirname(target), { recursive: true });
      let data;
      if (method === 0) data = comp;
      else if (method === 8) data = zlib.inflateRawSync(comp);
      else fail(4, `不支持的 ZIP 压缩方法 ${method}: ${name}`);
      fs.writeFileSync(target, data);
    }
    p += 46 + nameLen + extraLen + commentLen;
  }
}

/**
 * 纯 Node 标准库 TGZ 解压（gzip + ustar），带 zip-slip 防护。
 * Windows 的 MSYS/Git Bash tar 会把 `C:\...` 里的冒号解释为远程主机，
 * 因此不使用外部 tar 命令，保证三平台行为一致。
 */
function extractTgz(buf, destDir) {
  const tar = zlib.gunzipSync(buf);
  const root = path.resolve(destDir);
  fs.mkdirSync(root, { recursive: true });
  let offset = 0;
  let longName = null;
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    offset += 512;
    if (header.every((b) => b === 0)) break;
    let name = header.toString('utf8', 0, 100).replace(/\0.*$/, '').trim();
    const prefix = header.toString('utf8', 345, 500).replace(/\0.*$/, '').trim();
    if (prefix) name = `${prefix}/${name}`;
    const sizeRaw = header.toString('ascii', 124, 136).replace(/\0.*$/, '').trim();
    const size = parseInt(sizeRaw, 8);
    if (!Number.isFinite(size) || size < 0) fail(4, `TGZ 头中的 size 非法: ${sizeRaw}`);
    const type = String.fromCharCode(header[156] || 48);
    const data = tar.subarray(offset, offset + size);
    offset += Math.ceil(size / 512) * 512;

    if (type === 'L') {
      longName = data.toString('utf8').replace(/\0.*$/, '').trim();
      continue;
    }
    if (longName) { name = longName; longName = null; }
    if (!name) continue;
    // 拒绝绝对路径、盘符与 .. 逃逸
    if (/^[A-Za-z]:/.test(name) || name.startsWith('/') || name.includes('..')) {
      fail(4, `TGZ 路径越界: ${name}`);
    }
    const target = path.resolve(root, name);
    if (target !== root && !target.startsWith(root + path.sep)) {
      fail(4, `TGZ 路径越界 (zip slip): ${name}`);
    }
    if (type === '5') {
      fs.mkdirSync(target, { recursive: true });
    } else if (type === '0' || type === '\0' || type === '7') {
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, data);
    }
    // 其它类型（符号链接 / PAX 扩展头等）跳过，PDFium 包不需要。
  }
}

/** 在解压目录里按 basename 递归定位目标文件（压缩包内路径可能与声明略有差异）。 */
function locateInDir(dir, declaredPath) {
  const declared = path.basename(declaredPath);
  const hits = [];
  walk(dir, (full) => {
    if (path.basename(full).toLowerCase() === declared.toLowerCase()) hits.push(full);
  });
  // 优先完全匹配声明的相对路径
  const exact = path.join(dir, declaredPath);
  if (fs.existsSync(exact) && fs.statSync(exact).isFile()) return exact;
  if (hits.length > 0) return hits.sort((a, b) => a.length - b.length)[0];
  return null;
}

function brewCopy(formula, brewBin) {
  if (process.platform !== 'darwin') {
    fail(3, `brew 仅 macOS 可用（当前 ${process.platform}）；请在 macOS runner 运行或使用 --offline`);
  }
  let prefix;
  try {
    prefix = execFileSync('brew', ['--prefix', formula], { encoding: 'utf8' }).trim();
  } catch {
    warn(`${formula} 未安装，尝试 brew install（需网络，可能较慢）`);
    try {
      execFileSync('brew', ['install', formula], { stdio: 'inherit' });
    } catch (e) {
      fail(3, `brew install ${formula} 失败: ${e.message}`);
    }
    prefix = execFileSync('brew', ['--prefix', formula], { encoding: 'utf8' }).trim();
  }
  const src = path.join(prefix, brewBin);
  if (!fs.existsSync(src)) fail(5, `brew 前缀下找不到 ${src}`);
  return src;
}

/** 取得某个 unit 的源文件路径（写到临时工作目录），返回 { srcPath, provenance }。 */
async function obtainSource(unit, target, args, workDir) {
  const s = unit.source;
  if (args.offline) {
    const found = findOfflineFile(args.offline, unit.offlineNames, target);
    if (!found) fail(5, `--offline 目录中找不到 ${unit.name}（候选名: ${unit.offlineNames.join(', ')}）`);
    return { srcPath: found, provenance: { mode: 'offline', path: found } };
  }
  if (s.brew) {
    const src = brewCopy(s.brew, s.brewBin);
    return { srcPath: src, provenance: { mode: 'brew', formula: s.brew, path: src } };
  }
  if (s.url === null) {
    // 没有可直接解压的上游包时，优先复用本机已安装的同版本工具
    // （CI 里可先 `choco install tesseract -y`，或用 TESSERACT_HOME 指定路径）。
    const candidates = [];
    if (s.localEnv && process.env[s.localEnv]) {
      candidates.push(
        path.join(process.env[s.localEnv], 'tesseract.exe'),
        process.env[s.localEnv],
      );
    }
    if (s.localPaths) candidates.push(...s.localPaths);
    const found = candidates.find((p) => {
      try { return fs.existsSync(p) && fs.statSync(p).isFile(); } catch { return false; }
    });
    if (!found) {
      fail(3, `${unit.name} 无可用的上游便携包，且本机未找到已安装的 tesseract.exe。`
        + `请先安装（Windows: choco install tesseract -y）或设置 ${s.localEnv || 'TESSERACT_HOME'}，`
        + `或从 ${s.homepage} 核实后填入 SOURCES，或使用 --offline <dir>`);
    }
    log(`[provision]    复用本机安装: ${found}`);
    return { srcPath: found, provenance: { mode: 'local-install', path: found } };
  }
  if (s.todoVerifyUrl) {
    warn(`TODO(verify-url): ${unit.name} 的 URL 未人工核实，仍尝试下载: ${s.url}`);
  }
  const buf = await fetchBytes(s.url);
  const tmpName = `${unit.name}-${path.basename(new URL(s.url).pathname) || 'download'}`;
  const tmpPath = path.join(workDir, tmpName.replace(/[^\w.-]/g, '_'));
  fs.writeFileSync(tmpPath, buf);

  if (!s.archive) return { srcPath: tmpPath, provenance: { mode: 'http', url: s.url } };

  const extractDir = path.join(workDir, `extract-${unit.name}`);
  fs.mkdirSync(extractDir, { recursive: true });
  if (s.archive === 'zip') {
    extractZip(buf, extractDir);
  } else if (s.archive === 'tgz' || s.archive === 'tar.gz') {
    try {
      extractTgz(buf, extractDir);
    } catch (e) {
      fail(4, `tgz 解压失败: ${e.message}`);
    }
  } else if (s.archive === 'nsis') {
    fail(4, `${unit.name} 是 NSIS 安装包，无法通用解压；请改用便携版 zip 或 --offline`);
  } else {
    fail(4, `未知 archive 类型: ${s.archive}`);
  }
  const declared = s.binPathInArchive || s.libInArchive;
  const located = declared ? locateInDir(extractDir, declared) : null;
  if (!located) fail(4, `在压缩包中找不到 ${declared || '目标文件'}（解压目录: ${extractDir}）`);
  return {
    srcPath: located,
    provenance: { mode: 'http', url: s.url, archive: s.archive, entry: declared },
  };
}

// ---------------------------------------------------------------------------
// 架构校验
// ---------------------------------------------------------------------------
function verifyPE(buf, expectedMachine) {
  if (buf.length < 0x40 || buf.toString('ascii', 0, 2) !== 'MZ') {
    return { ok: false, arch: null, detail: '缺少 MZ (DOS) 头' };
  }
  const peOff = buf.readUInt32LE(0x3c);
  if (peOff + 24 > buf.length || buf.toString('ascii', peOff, peOff + 4) !== 'PE\0\0') {
    return { ok: false, arch: null, detail: '缺少 PE\\0\\0 签名' };
  }
  const machine = buf.readUInt16LE(peOff + 4);
  const names = { 0x8664: 'x86_64', 0x014c: 'i386', 0xaa64: 'arm64' };
  return {
    ok: machine === expectedMachine,
    arch: names[machine] || `0x${machine.toString(16)}`,
    detail: `PE Machine=0x${machine.toString(16)}`,
  };
}

function verifyMachO(buf, expected) {
  const fatMagics = new Set([0xcafebabe, 0xbebafeca, 0xcafebabf, 0xbfbafeca]);
  const beMagic = buf.readUInt32BE(0);
  const leMagic = buf.readUInt32LE(0);
  const archs = new Set();
  const addCpu = (cpu) => { const n = CPUTYPE[cpu]; if (n) archs.add(n); };
  if (fatMagics.has(beMagic) || fatMagics.has(leMagic)) {
    const nfat = buf.readUInt32BE(4); // fat 头为大端
    for (let i = 0; i < Math.min(nfat, 16); i++) {
      const off = 8 + i * 20;
      if (off + 8 > buf.length) break;
      addCpu(buf.readUInt32BE(off)); // cputype
    }
  } else {
    // 单一 Mach-O：由 magic 判断字节序后读 cputype (offset 4)
    const cpu = (beMagic === 0xfeedfacf || beMagic === 0xfeedface)
      ? buf.readInt32BE(4)
      : buf.readInt32LE(4);
    addCpu(cpu >>> 0);
  }
  return {
    ok: archs.has(expected),
    arch: archs.size ? [...archs].join('+') : 'unknown',
    detail: `Mach-O archs=[${[...archs].join(',') || '?'}] expected=${expected}`,
  };
}

function verifyArch(buf, t) {
  if (t.peMachine !== null) return verifyPE(buf, t.peMachine);
  return verifyMachO(buf, t.machoExpected);
}

/** macOS 上额外用 lipo -archs 复核（可用时）。 */
function lipoCheck(file, expected) {
  if (process.platform !== 'darwin') return null;
  const r = spawnSync('lipo', ['-archs', file], { encoding: 'utf8' });
  if (r.status !== 0) return { ok: false, detail: `lipo -archs 失败: ${(r.stderr || '').trim()}` };
  const out = (r.stdout || '').trim();
  return { ok: out.split(/\s+/).includes(expected), detail: `lipo -archs: ${out}` };
}

function checkSigned(file, t) {
  if (t.os !== 'macos') {
    return { signed: false, note: 'Windows Authenticode 在 CI 签名步骤处理' };
  }
  if (process.platform !== 'darwin') {
    return { signed: false, note: '当前非 macOS 主机，跳过 codesign -dv 检查' };
  }
  const r = spawnSync('codesign', ['-dv', '--verbose=0', file], { encoding: 'utf8' });
  return r.status === 0
    ? { signed: true, note: 'codesign -dv 通过' }
    : { signed: false, note: 'codesign -dv 未通过（通常在 CI 打包阶段签名）' };
}

/**
 * Windows 版 qpdf / tesseract 依赖同目录 DLL。externalBin 只打包可执行文件本身，
 * 因此把同目录的 DLL 一并放进 bundle.resources，由 Rust 端 apply_child_env()
 * 把 resource_dir 前置到子进程 PATH，保证干净机器上也能启动。
 */
function copySiblingDlls(srcPath, resDir) {
  const dir = path.dirname(srcPath);
  let names = [];
  try {
    names = fs.readdirSync(dir).filter((n) => n.toLowerCase().endsWith('.dll'));
  } catch { return []; }
  const copied = [];
  for (const n of names) {
    const from = path.join(dir, n);
    const to = path.join(resDir, n);
    fs.copyFileSync(from, to);
    copied.push(n);
  }
  if (copied.length) {
    log(`[provision]    随包 DLL (${copied.length} 个, resources/): ${copied.slice(0, 6).join(', ')}${copied.length > 6 ? ' ...' : ''}`);
  }
  return copied;
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------
async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) { help(); return; }
  if (!args.target) fail(2, '缺少 --target <triple>（用 --help 查看用法）');
  const t = TARGETS[args.target];
  if (!t) fail(2, `未知 target: ${args.target}；支持: ${Object.keys(TARGETS).join(' / ')}`);
  if (args.offline && !fs.existsSync(args.offline)) {
    fail(2, `--offline 目录不存在: ${args.offline}`);
  }
  const units = buildPlan(args.target, t);

  if (args.dryRun) {
    printPlan(args.target, t, units, args);
    return;
  }

  log(`[provision] target=${args.target} mode=${args.offline ? 'offline' : 'online'}`);
  log(`[provision] TODO(verify-url) 共 ${TODO_URLS.length} 处需人工核实`);
  fs.mkdirSync(BIN_DIR, { recursive: true });
  fs.mkdirSync(path.join(RES_DIR, 'tessdata'), { recursive: true });

  const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'payoncepdf-sidecars-'));
  const entries = [];
  try {
    for (const u of units) {
      log(`[provision] -> ${u.name} (${u.kind})`);
      const { srcPath, provenance } = await obtainSource(u, args.target, args, workDir);
      const buf = fs.readFileSync(srcPath);
      if (buf.length === 0) fail(5, `${u.name} 源文件为空: ${srcPath}`);

      if (u.archCheck) {
        const v = verifyArch(buf, t);
        if (!v.ok) fail(6, `${u.name} 架构不匹配 -> ${v.detail} (${rel(u.dest)})`);
        const lipo = t.os === 'macos' ? lipoCheck(srcPath, t.machoExpected) : null;
        if (lipo && !lipo.ok) fail(6, `${u.name} lipo 复核失败 -> ${lipo.detail}`);
        log(`[provision]    arch ok: ${v.detail}${lipo ? ' | ' + lipo.detail : ''}`);
      }

      fs.mkdirSync(path.dirname(u.dest), { recursive: true });
      fs.copyFileSync(srcPath, u.dest);
      if (t.os === 'macos' && u.kind === 'externalBin') {
        try { fs.chmodSync(u.dest, 0o755); } catch { /* 非 POSIX 文件系统忽略 */ }
      }
      if (!fs.existsSync(u.dest)) fail(5, `写入后文件不存在: ${rel(u.dest)}`);
      if (t.os === 'windows' && u.kind === 'externalBin') {
        copySiblingDlls(srcPath, RES_DIR);
      }
      const written = fs.readFileSync(u.dest);
      if (sha256(written) !== sha256(buf)) fail(5, `${u.name} 写入后 sha256 不一致`);

      const sig = checkSigned(u.dest, t);
      entries.push({
        name: u.name,
        kind: u.kind,
        dest: rel(u.dest),
        component: u.source.component,
        version: u.source.version || null,
        source: provenance,
        license: u.source.license,
        homepage: u.source.homepage,
        todoVerifyUrl: !!u.source.todoVerifyUrl,
        bytes: written.length,
        sha256: sha256(written),
        arch: t.arch,
        archVerified: u.archCheck,
        signed: sig.signed,
        signedCheck: sig.note,
      });
      log(`[provision]    ${rel(u.dest)} (${written.length} bytes, sha256=${sha256(written).slice(0, 12)}...)`);
    }

    // externalBin 命名规范最终校验
    for (const nm of ['qpdf', 'tesseract']) {
      const expect = path.join(BIN_DIR, `${nm}-${args.target}${t.exeSuffix}`);
      if (!fs.existsSync(expect)) fail(5, `externalBin 命名校验失败，缺少 ${rel(expect)}`);
    }

    const manifest = {
      generator: 'scripts/provision-sidecars.mjs',
      version: GENERATOR_VERSION,
      target: args.target,
      platform: { os: t.os, arch: t.arch },
      generatedAt: new Date().toISOString(),
      bundle: {
        externalBin: ['bin/qpdf', 'bin/tesseract'],
        resources: ['resources/*'],
        note: 'externalBin 实际文件名为 bin/<name>-<target-triple>[.exe]，由 Tauri 打包时按 triple 选取。',
      },
      licenses: {
        qpdf: 'Apache-2.0', tesseract: 'Apache-2.0',
        tessdata_fast: 'Apache-2.0', pdfium: 'BSD-3-Clause',
      },
      todoVerifyUrls: TODO_URLS,
      notes: [
        'signed 字段反映供给阶段状态；CI 打包签名发生在 tauri build 之后，故通常为 false。',
        'macOS 需对 resources 内的 dylib 与 externalBin 逐文件 codesign --options runtime，再签 .app。',
        ...SOURCES.qpdf.notes,
        ...SOURCES.tesseract.notes,
      ],
      entries,
    };
    fs.writeFileSync(MANIFEST_PATH, JSON.stringify(manifest, null, 2) + '\n', 'utf8');
    log(`[provision] manifest: ${rel(MANIFEST_PATH)}`);
    log(`[provision] 完成：${entries.length} 个文件，全部通过存在性 / sha256 / 架构校验。`);
  } finally {
    try { fs.rmSync(workDir, { recursive: true, force: true }); } catch { /* ignore */ }
  }
}

main().catch((err) => {
  // 兜底：任何未捕获异常都不能静默成功
  process.stderr.write(`[ERROR][unhandled] ${err && err.stack ? err.stack : err}\n`);
  process.exit(3);
});
