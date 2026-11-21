#!/usr/bin/env node
/**
 * gen-license-keys.mjs - 生成 PayOnce PDF 开发用 Ed25519 授权密钥对（Node 22 ESM）
 *
 * 用途
 *   用 Node crypto.generateKeyPairSync('ed25519') 生成一对开发密钥：
 *     - 公钥：SPKI DER 的最后 32 字节（Ed25519 原始公钥），base64 写入
 *             src-tauri/keys/public.key（供 Rust 编译期嵌入，校验离线激活签名）
 *     - 私钥：PKCS#8 PEM 写入 scripts/.dev-license-private.key（仅本地 mock 服务使用）
 *   并输出公钥指纹（sha256），便于核对 Rust 端嵌入的公钥是否一致。
 *
 * 用法
 *   node scripts/gen-license-keys.mjs            # 生成 / 覆盖开发密钥对
 *   node scripts/gen-license-keys.mjs --check    # 只校验公钥存在且为 32 字节 base64
 *   node scripts/gen-license-keys.mjs --help
 *
 * 注意（重要）
 *   1. 私钥绝不入库：scripts/.dev-license-private.key 必须加入 .gitignore
 *      （本目录已提供 scripts/.gitignore，写入 .dev-license-private.key）。
 *   2. 公钥必须入库：根目录 .gitignore 当前有 `src-tauri/keys/*` 规则，会忽略
 *      public.key。请负责 .gitignore 的 Agent 增加 `!src-tauri/keys/public.key`
 *      白名单，或在 CI 构建前用本脚本重新生成公钥；否则 Rust 编译期拿不到公钥。
 *   3. 本脚本只产生**开发**密钥。生产密钥必须由离线安全流程生成，公钥替换流程见
 *      src-tauri/keys/README.md；激活服务须替换为 Gumroad / 爱发电 的真实校验服务。
 *   4. 覆盖已有密钥会使此前签发的所有离线激活签名失效，请谨慎。
 *
 * 退出码
 *   0 成功（含 --check 通过）
 *   1 校验失败 / 缺少依赖
 *   2 参数错误
 *
 * 工程红线
 *   - 路径用 path.join / fileURLToPath，不写死盘符；Windows / macOS 均可运行。
 *   - 不硬编码任何凭据；不联网；不生成 emoji。
 */

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import process from 'node:process';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '..');
const KEYS_DIR = path.join(REPO_ROOT, 'src-tauri', 'keys');
const PUBLIC_KEY_PATH = path.join(KEYS_DIR, 'public.key');
const PRIVATE_KEY_PATH = path.join(__dirname, '.dev-license-private.key');
const RAW_ED25519_PUBKEY_BYTES = 32;

function log(msg) { process.stdout.write(msg + '\n'); }
function fail(code, msg) { process.stderr.write(`[ERROR] ${msg}\n`); process.exit(code); }

function help() {
  log('gen-license-keys.mjs - 生成 PayOnce PDF 开发用 Ed25519 授权密钥对');
  log('');
  log('用法: node scripts/gen-license-keys.mjs [--check] [--help]');
  log('  (无参数)   生成 / 覆盖 src-tauri/keys/public.key 与 scripts/.dev-license-private.key');
  log('  --check    只校验 public.key 存在且为 32 字节原始公钥的 base64');
  log('  --help     显示本帮助');
  log('');
  log('提醒: 私钥绝不入库；生产环境须替换密钥与激活服务（见 src-tauri/keys/README.md）。');
}

function parseArgs(argv) {
  const args = { check: false, help: false };
  for (const a of argv) {
    if (a === '--check') args.check = true;
    else if (a === '--help' || a === '-h') args.help = true;
    else fail(2, `未知参数: ${a}（用 --help 查看用法）`);
  }
  return args;
}

/** 从 SPKI DER 中取出 32 字节 Ed25519 原始公钥。 */
function rawPublicKey(publicKey) {
  const spki = publicKey.export({ format: 'der', type: 'spki' });
  if (spki.length < RAW_ED25519_PUBKEY_BYTES) {
    fail(1, `SPKI DER 过短 (${spki.length} bytes)`);
  }
  const raw = spki.subarray(spki.length - RAW_ED25519_PUBKEY_BYTES);
  // Ed25519 SPKI 固定前缀: 302a300506032b6570032100
  const prefix = Buffer.from('302a300506032b6570032100', 'hex');
  if (spki.length !== 44 || !spki.subarray(0, 12).equals(prefix)) {
    fail(1, 'SPKI DER 不是预期的 Ed25519 结构（预期 44 字节）');
  }
  return raw;
}

function fingerprint(raw) {
  return crypto.createHash('sha256').update(raw).digest('hex');
}

function prettyFingerprint(hex) {
  return (hex.match(/.{1,4}/g) || []).join(':');
}

function checkMode() {
  if (!fs.existsSync(PUBLIC_KEY_PATH)) {
    fail(1, `缺少公钥: ${PUBLIC_KEY_PATH}（先运行 node scripts/gen-license-keys.mjs）`);
  }
  const text = fs.readFileSync(PUBLIC_KEY_PATH, 'utf8').trim();
  let raw;
  try { raw = Buffer.from(text, 'base64'); }
  catch { fail(1, 'public.key 不是合法 base64'); }
  if (raw.length !== RAW_ED25519_PUBKEY_BYTES || raw.toString('base64') !== text) {
    fail(1, `public.key 不是 32 字节原始公钥的 base64（解码后 ${raw.length} 字节）`);
  }
  const fp = fingerprint(raw);
  log(`[gen-license-keys] --check OK`);
  log(`  public.key : ${PUBLIC_KEY_PATH}`);
  log(`  base64     : ${text}`);
  log(`  指纹 sha256: ${fp}`);
  log(`  指纹(分组) : ${prettyFingerprint(fp)}`);
}

function generateMode() {
  if (!fs.existsSync(KEYS_DIR)) fs.mkdirSync(KEYS_DIR, { recursive: true });
  if (fs.existsSync(PUBLIC_KEY_PATH) || fs.existsSync(PRIVATE_KEY_PATH)) {
    log('[WARN] 检测到已有开发密钥，将被覆盖；此前签发的离线激活签名将失效。');
  }
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const raw = rawPublicKey(publicKey);
  const b64 = raw.toString('base64');
  const pem = String(privateKey.export({ format: 'pem', type: 'pkcs8' }));

  fs.writeFileSync(PUBLIC_KEY_PATH, b64, 'utf8');            // 恰好 base64 文本，无换行
  fs.writeFileSync(PRIVATE_KEY_PATH, pem, 'utf8');
  // 私钥权限收紧（POSIX 有效；Windows 忽略）
  try { fs.chmodSync(PRIVATE_KEY_PATH, 0o600); } catch { /* ignore */ }

  const fp = fingerprint(raw);
  log('[gen-license-keys] 已生成开发用 Ed25519 密钥对');
  log(`  公钥(base64 原始 32 字节): ${PUBLIC_KEY_PATH}`);
  log(`    ${b64}`);
  log(`  私钥(PKCS#8 PEM)        : ${PRIVATE_KEY_PATH}`);
  log(`  指纹 sha256             : ${fp}`);
  log(`  指纹(分组)              : ${prettyFingerprint(fp)}`);
  log('');
  log('[提醒] 私钥绝不入库：scripts/.dev-license-private.key 已在 scripts/.gitignore 中忽略；');
  log('        如改动 .gitignore，请确保该规则保留，切勿把私钥提交到版本库。');
  log('[注意] 公钥必须入库：根目录 .gitignore 的 `src-tauri/keys/*` 会忽略 public.key；');
  log('        请负责 .gitignore 的 Agent 增加 `!src-tauri/keys/public.key` 白名单，');
  log('        否则 Rust 编译期无法嵌入公钥，离线激活签名无法校验。');
  log('[生产] 本密钥仅用于开发联调；生产密钥替换流程见 src-tauri/keys/README.md，');
  log('        激活服务须替换为 Gumroad / 爱发电 的真实校验服务（非 scripts/license-server.mjs mock）。');
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) { help(); return; }
  if (args.check) checkMode();
  else generateMode();
}

main();
