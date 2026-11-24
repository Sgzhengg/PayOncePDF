#!/usr/bin/env node
/**
 * license-server.mjs - PayOnce PDF 本地 mock 激活服务（Node 22 ESM，仅本地联调）
 *
 * 用途
 *   提供 POST http://127.0.0.1:8787/api/activate，接受 {key, fingerprint}，
 *   校验 PAYONCE-XXXX-XXXX-XXXX 格式，返回 Ed25519 签名的离线激活载荷，供 Rust 端
 *   license.rs 与前端 activate_license 命令联调。
 *
 * 用法
 *   node scripts/gen-license-keys.mjs        # 先生成开发密钥对
 *   node scripts/license-server.mjs          # 启动（默认 127.0.0.1:8787）
 *   PAYONCEPDF_MOCK_PORT=9000 node scripts/license-server.mjs
 *   curl -s -X POST http://127.0.0.1:8787/api/activate \
 *        -H 'Content-Type: application/json' \
 *        -d '{"key":"PAYONCE-ABCD-EFGH-IJKL","fingerprint":"machine-fp-hash"}'
 *
 * 响应
 *   200 { "payload": "<base64(JSON)>", "signature": "<base64(Ed25519 签名)>" }
 *   payload = base64(UTF-8(JSON))，JSON 字段：
 *     key         : 完整 license key（明文；Rust 端强制校验与缓存 key 一致）
 *     keyHash     : sha256(hex) of the license key（服务端索引，不替代 key）
 *     fingerprint : 请求中的机器指纹哈希（Rust 端强制校验）
 *     issuedAt    : ISO-8601 签发时间（必填）
 *     nonce       : 一次性随机数（生产服务据此防重放）
 *     expiresAt   : null（买断，永不过期；试用过期由 Rust 端本地 7 天逻辑控制）
 *   signature = base64(Ed25519 sign(payloadBytes, privateKey))
 *   校验方式（Rust 端）：base64 解码 payload 得到 payloadBytes，用 public.key 的
 *   Ed25519 公钥对 payloadBytes 验证 signature；再解析 JSON 使用字段。
 *
 * 错误
 *   400 { "code": "E2004", "message": "..." }  非法 key / 缺字段 / 非法 JSON
 *   404 / 405                                  路径或方法不匹配
 *
 * 生产环境必须替换（重要）
 *   本服务是**本地 mock**，只做格式校验、不查购买记录、不防重放、不维护吊销列表。
 *   上线必须替换为 Gumroad / 爱发电 的真实校验服务（校验订单、绑定机器指纹、
 *   防重放 nonce、吊销与退款同步），私钥不得随客户端分发。
 *   Rust 端 `DEFAULT_ACTIVATE_URL` 固定为 https://payoncepdf.com/api/activate
 *   （该端点尚未部署）。**本地联调必须显式设置**环境变量：
 *     PAYONCEPDF_ACTIVATE_URL=http://127.0.0.1:8787/api/activate
 *   未设置时普通用户会连到尚未上线的生产地址；本 mock 不是任何环境的默认目标。
 *
 * 工程红线
 *   - 仅监听 127.0.0.1（不暴露到网络）；不硬编码任何凭据；不生成 emoji。
 *   - 路径用 path.join / fileURLToPath，不写死盘符；Windows / macOS 均可运行。
 *   - 请求体限制 1MB，避免本地 mock 被滥用。
 */

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import process from 'node:process';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PRIVATE_KEY_PATH = path.join(__dirname, '.dev-license-private.key');
const HOST = '127.0.0.1';
const PORT = Number.parseInt(process.env.PAYONCEPDF_MOCK_PORT || '8787', 10);
const MAX_BODY = 1024 * 1024;
const KEY_RE = /^PAYONCE-[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}$/;
// 2026-11-20 拍板：一把钥匙最多 3 台设备（与 config/product.json 的 max_devices_per_key 一致）。
// 本 mock 用内存 Set 记录不同机器指纹；生产激活服务需要持久化台账，属人工待办。
const MAX_DEVICES = 3;
const deviceLedger = new Map();

function log(msg) { process.stdout.write(msg + '\n'); }

if (!Number.isInteger(PORT) || PORT < 1 || PORT > 65535) {
  process.stderr.write(`[ERROR] PAYONCEPDF_MOCK_PORT 非法: ${process.env.PAYONCEPDF_MOCK_PORT}\n`);
  process.exit(1);
}
if (!fs.existsSync(PRIVATE_KEY_PATH)) {
  process.stderr.write(
    `[ERROR] 缺少开发私钥: ${PRIVATE_KEY_PATH}\n` +
    `        请先运行: node scripts/gen-license-keys.mjs\n`);
  process.exit(1);
}

const privateKey = crypto.createPrivateKey({
  key: fs.readFileSync(PRIVATE_KEY_PATH, 'utf8'),
  format: 'pem',
  type: 'pkcs8',
});

function sendJson(res, status, obj) {
  const body = Buffer.from(JSON.stringify(obj), 'utf8');
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Content-Length': body.length,
    'Cache-Control': 'no-store',
  });
  res.end(body);
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (c) => {
      size += c.length;
      if (size > MAX_BODY) {
        reject(new Error('请求体超过 1MB 限制'));
        req.destroy();
        return;
      }
      chunks.push(c);
    });
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
    req.on('error', reject);
  });
}

function activate(payloadObj) {
  const { key, fingerprint } = payloadObj;
  if (typeof key !== 'string' || !KEY_RE.test(key)) {
    return { status: 400, body: { code: 'E2004', message: 'invalid license key: expected PAYONCE-XXXX-XXXX-XXXX' } };
  }
  if (typeof fingerprint !== 'string' || fingerprint.length === 0) {
    return { status: 400, body: { code: 'E2004', message: 'fingerprint 必须是非空字符串' } };
  }
  const used = deviceLedger.get(key) ?? new Set();
  if (!used.has(fingerprint) && used.size >= MAX_DEVICES) {
    return {
      status: 403,
      body: { code: 'E2004', message: `该密钥已激活 ${MAX_DEVICES} 台设备，达到上限` },
    };
  }
  used.add(fingerprint);
  deviceLedger.set(key, used);
  // 字段契约见 src-tauri/keys/README.md：key / fingerprint / issuedAt / nonce 必填，
  // 缺任一字段 Rust 端会拒绝激活，避免旧 mock 令牌被缓存成「看起来已激活」。
  const payload = {
    key,
    keyHash: crypto.createHash('sha256').update(key, 'utf8').digest('hex'),
    fingerprint,
    issuedAt: new Date().toISOString(),
    nonce: crypto.randomUUID(),
    expiresAt: null,
  };
  const payloadBytes = Buffer.from(JSON.stringify(payload), 'utf8');
  const signature = crypto.sign(null, payloadBytes, privateKey);
  return {
    status: 200,
    body: {
      payload: payloadBytes.toString('base64'),
      signature: signature.toString('base64'),
    },
  };
}

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, `http://${HOST}:${PORT}`);
    if (req.method === 'GET' && url.pathname === '/health') {
      return sendJson(res, 200, { status: 'ok', mode: 'local-mock' });
    }
    if (url.pathname !== '/api/activate') {
      return sendJson(res, 404, { code: 'E2004', message: 'not found' });
    }
    if (req.method !== 'POST') {
      res.setHeader('Allow', 'POST');
      return sendJson(res, 405, { code: 'E2004', message: 'method not allowed; use POST' });
    }
    let parsed;
    try {
      parsed = JSON.parse(await readBody(req));
    } catch (e) {
      return sendJson(res, 400, { code: 'E2004', message: `invalid JSON / body: ${e.message}` });
    }
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return sendJson(res, 400, { code: 'E2004', message: 'body 必须是 JSON 对象' });
    }
    const out = activate(parsed);
    return sendJson(res, out.status, out.body);
  } catch (err) {
    return sendJson(res, 500, { code: 'E2004', message: `internal mock error: ${err.message}` });
  }
});

server.listen(PORT, HOST, () => {
  log(`[license-server] 本地 mock 激活服务: http://${HOST}:${PORT}/api/activate`);
  log('[license-server] 私钥: ' + PRIVATE_KEY_PATH + '（仅开发用，绝不入库）');
  log('[license-server] 生产须替换为 Gumroad / 爱发电 真实校验服务；Rust 端用 PAYONCEPDF_ACTIVATE_URL 覆盖地址。');
  log('[license-server] Ctrl+C 停止。');
});
