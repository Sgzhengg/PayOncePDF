// 安装包体积回归：resources 里不得再混入未被依赖的 DLL。
//
// 背景（2026-11-25）：`scripts/provision-sidecars.mjs` 原先把 qpdf.exe 同目录的
// 所有 .dll 全量拷进 resources，而 MSYS2 发行目录里还带着整条 GTK/ICU 栈：
// libicudt75.dll 29.3 MB、libicuin/icuuc 4.1 MB、pango/cairo/glib/harfbuzz/
// freetype/fontconfig 约 10 MB，合计 43.6 MB 死重量。
// 后果：安装包 31,719,938 B，国内单流实测 117 KB/s，下载要 4 分多钟。
// 删掉这些文件后应用内自检 32/32 全过，包体降到约一半。

import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const RES = path.join(ROOT, 'src-tauri', 'resources');

/** 已确认未被 payoncepdf.exe / qpdf.exe / tesseract.exe 依赖的 GTK/ICU 链。 */
const DEAD_DLLS = [
  'libicudt75.dll',
  'libicuin75.dll',
  'libicuuc75.dll',
  'libgio-2.0-0.dll',
  'libglib-2.0-0.dll',
  'libgobject-2.0-0.dll',
  'libgmodule-2.0-0.dll',
  'libharfbuzz-0.dll',
  'libcairo-2.dll',
  'libfreetype-6.dll',
  'libpixman-1-0.dll',
  'libfontconfig-1.dll',
  'libpango-1.0-0.dll',
  'libpangowin32-1.0-0.dll',
  'libpangoft2-1.0-0.dll',
  'libpangocairo-1.0-0.dll',
  'libpcre2-8-0.dll',
  'libfribidi-0.dll',
  'libgraphite2.dll',
  'libthai-0.dll',
  'libdatrie-1.dll',
  'libffi-8.dll',
];

function listDlls() {
  if (!fs.existsSync(RES)) return [];
  return fs.readdirSync(RES)
    .filter((n) => n.toLowerCase().endsWith('.dll'))
    .map((n) => ({ name: n, size: fs.statSync(path.join(RES, n)).size }));
}

test('resources 不得再包含未被依赖的 GTK/ICU DLL', () => {
  const present = new Set(listDlls().map((d) => d.name.toLowerCase()));
  const leaked = DEAD_DLLS.filter((n) => present.has(n.toLowerCase()));
  assert.deepEqual(leaked, [],
    `这些 DLL 未被任何二进制 import，只会凭空增大安装包：${leaked.join(', ')}`);
});

test('resources 体积守住上限，没有异常大文件', () => {
  const dlls = listDlls();
  assert.ok(dlls.length > 0, 'resources 目录不存在或为空');
  const total = dlls.reduce((a, d) => a + d.size, 0);
  assert.ok(total < 46 * 1024 * 1024,
    `DLL 合计 ${(total / 1048576).toFixed(1)} MB，超过 46 MB 上限（应是 36 个 / 约 37 MB）`);
  // 单文件超过 8 MB 只可能是被误拷回来的 ICU 数据（29 MB）或重复大库
  const huge = dlls.filter((d) => d.size > 8 * 1024 * 1024);
  assert.deepEqual(huge.map((d) => d.name), [],
    `出现异常大的 DLL：${huge.map((d) => `${d.name}(${(d.size / 1048576).toFixed(1)}MB)`).join(', ')}`);
});

test('qpdf 真正依赖的 libstdc++ 必须保留（曾因文件名带 + 被扫描漏掉）', () => {
  const names = new Set(listDlls().map((d) => d.name.toLowerCase()));
  for (const n of ['libstdc++-6.dll', 'libgcc_s_seh-1.dll', 'libwinpthread-1.dll', 'qpdf29.dll']) {
    assert.ok(names.has(n), `缺少 qpdf 运行时依赖 ${n}，子进程会以 E2004 失败`);
  }
});

test('provision 脚本按依赖闭包复制，而不是全量拷贝同目录 DLL', () => {
  const src = fs.readFileSync(path.join(ROOT, 'scripts', 'provision-sidecars.mjs'), 'utf8');
  assert.match(src, /function referencedDlls/, '必须有依赖扫描函数');
  assert.match(src, /referencedDlls\(cur\)/, '必须沿导入图做闭包遍历');
  assert.doesNotMatch(src, /names = fs\.readdirSync\(dir\)\.filter\(\(n\) => n\.toLowerCase\(\)\.endsWith\('\.dll'\)\);\s*\n\s*\} catch \{ return \[\]; \}\s*\n\s*const copied = \[\];\s*\n\s*for \(const n of names\) \{/,
    '不得退回「同目录 DLL 全量拷贝」的旧实现');
});
