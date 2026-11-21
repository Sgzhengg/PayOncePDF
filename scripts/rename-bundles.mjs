#!/usr/bin/env node
/**
 * rename-bundles.mjs - 把 Tauri 默认产物名统一为发行命名。
 *
 *   Windows: PayOnce-PDF-Setup-<version>.exe (+ .sig)
 *            PayOnce-PDF-Setup-<version>.msi (+ .sig)
 *   macOS  : PayOnce-PDF-<version>.dmg
 *            .app 保持 "PayOnce PDF.app"（应用显示名，不改名）
 *
 * 用法：node scripts/rename-bundles.mjs [--require] [--windows-only]
 *   --require      找不到任何本次产物时以非零码退出（CI 使用）
 *   --windows-only 只处理 nsis / msi，跳过 dmg（CI 的 macOS dmg 由后续步骤自行命名）
 * 幂等：目标已存在且源即目标时不做任何事；已改成发行名的产物不会被重复处理。
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const CONF = path.join(ROOT, 'src-tauri', 'tauri.conf.json');
const TARGET_ROOT = path.join(ROOT, 'src-tauri', 'target');
const version = JSON.parse(fs.readFileSync(CONF, 'utf8')).version;
if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`[rename-bundles] tauri.conf.json version 非法: ${version}`);
  process.exit(1);
}

const requireArtifacts = process.argv.includes('--require');
const windowsOnly = process.argv.includes('--windows-only');
const results = [];

// 自动发现 bundle 目录：本地直构 target/release/bundle；带 --target 时 target/<triple>/release/bundle。
const bundleDirs = new Set();
const direct = path.join(TARGET_ROOT, 'release', 'bundle');
if (fs.existsSync(direct)) bundleDirs.add(direct);
if (fs.existsSync(TARGET_ROOT)) {
  for (const entry of fs.readdirSync(TARGET_ROOT, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const candidate = path.join(TARGET_ROOT, entry.name, 'release', 'bundle');
    if (fs.existsSync(candidate)) bundleDirs.add(candidate);
  }
}

function safeRename(src, dest) {
  if (src === dest) return 'unchanged';
  if (!fs.existsSync(src)) return 'missing';
  if (fs.existsSync(dest)) fs.rmSync(dest, { force: true });
  fs.renameSync(src, dest);
  return 'renamed';
}

function renameMatching(dir, predicate, nameFn) {
  if (!fs.existsSync(dir)) return;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (!entry.isFile() || !predicate(entry.name)) continue;
    const src = path.join(dir, entry.name);
    const dest = path.join(dir, nameFn(entry.name));
    const status = safeRename(src, dest);
    if (status !== 'missing') results.push({ status, from: entry.name, to: path.basename(dest), dir });
  }
}

// 只处理当前 productName 生成的默认文件名，历史产物留在原处作为对照。
const isCurrent = (n) => n.startsWith('PayOnce PDF');

for (const bundle of bundleDirs) {
  renameMatching(
    path.join(bundle, 'nsis'),
    (n) => isCurrent(n) && (n.endsWith('.exe') || n.endsWith('.exe.sig')),
    (n) => (n.endsWith('.exe.sig') ? `PayOnce-PDF-Setup-${version}.exe.sig` : `PayOnce-PDF-Setup-${version}.exe`),
  );
  renameMatching(
    path.join(bundle, 'msi'),
    (n) => isCurrent(n) && (n.endsWith('.msi') || n.endsWith('.msi.sig')),
    (n) => (n.endsWith('.msi.sig') ? `PayOnce-PDF-Setup-${version}.msi.sig` : `PayOnce-PDF-Setup-${version}.msi`),
  );
  if (!windowsOnly) {
    renameMatching(
      path.join(bundle, 'dmg'),
      (n) => isCurrent(n) && n.endsWith('.dmg'),
      () => `PayOnce-PDF-${version}.dmg`,
    );
  }
}

console.log(`[rename-bundles] version=${version} bundleDirs=${bundleDirs.size} windowsOnly=${windowsOnly}`);
for (const r of results) console.log(`  ${r.status.padEnd(9)} ${r.from} -> ${r.to}`);

// 幂等检查：即使本轮没有重命名，只要目标发行名已存在也视为成功。
const targets = [`PayOnce-PDF-Setup-${version}.exe`, `PayOnce-PDF-Setup-${version}.msi`];
if (!windowsOnly) targets.push(`PayOnce-PDF-${version}.dmg`);
const alreadyPresent = [...bundleDirs].some((bundle) =>
  targets.some((t) =>
    ['nsis', 'msi', 'dmg'].some((sub) => fs.existsSync(path.join(bundle, sub, t))),
  ),
);
if (results.length === 0) {
  console.log(`  (本轮无需重命名；目标发行名已存在=${alreadyPresent})`);
  if (requireArtifacts && !alreadyPresent) {
    console.error('[rename-bundles] --require 已指定但没有找到任何本次产物');
    process.exit(1);
  }
} else {
  console.log(`[rename-bundles] 完成，共 ${results.length} 个文件`);
}
