/**
 * opener-contract.test.mjs - Tauri opener 权限最小化的静态契约。
 *
 * 背景（上线前 CodeReview P2-2）：`opener:default` 会同时授予
 * `allow-open-url` / `allow-reveal-item-in-dir` / `allow-default-urls`，
 * 其中 `allow-default-urls` 允许 webview 打开任意 http/https/mailto/tel。
 * 本应用实际只用 `openUrl(PAYMENT_URL)`，因此收紧为显式 URL allow-list。
 *
 * 这些断言会进入 `npm test`（CI 的 quality job），防止后来者为了图方便
 * 把权限退回 `opener:default` 或加上 `https://*` 之类的过宽通配。
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const CAP = path.join(ROOT, 'src-tauri', 'capabilities', 'default.json');
const COMMERCE = path.join(ROOT, 'src', 'lib', 'commerce.ts');

const cap = JSON.parse(fs.readFileSync(CAP, 'utf8'));
const commerce = fs.readFileSync(COMMERCE, 'utf8');

const entries = cap.permissions.map((p) => (typeof p === 'string' ? { identifier: p } : p));
const opener = entries.find((e) => e.identifier.startsWith('opener:'));
const allowUrls = (opener?.allow ?? []).map((e) => e.url).filter(Boolean);

test('不再使用 opener:default 或允许任意 URL 的权限', () => {
  assert.ok(opener, '必须显式声明 opener 权限');
  assert.equal(opener.identifier, 'opener:allow-open-url');
  const ids = entries.map((e) => e.identifier);
  for (const banned of [
    'opener:default',
    'opener:allow-default-urls',
    'opener:allow-open-path',
    'opener:allow-reveal-item-in-dir',
  ]) {
    assert.ok(!ids.includes(banned), `不得授予 ${banned}`);
  }
});

test('URL allow-list 精确到本站与 Gumroad，无过宽通配', () => {
  assert.ok(allowUrls.length >= 2, 'allow 列表不能为空');
  for (const u of allowUrls) {
    assert.match(u, /^https:\/\//, `只允许 https：${u}`);
    assert.ok(u.endsWith('/*'), `必须以路径通配结尾：${u}`);
    const host = u.slice('https://'.length).replace(/\/\*$/, '');
    assert.ok(host !== '*' && host !== '', `不得出现任意域通配：${u}`);
    assert.ok(
      host === 'payoncepdf.com' ||
        host === 'www.payoncepdf.com' ||
        host === 'gumroad.com' ||
        host === '*.gumroad.com' ||
        /^[a-z0-9-]+\.gumroad\.com$/.test(host),
      `只允许 payoncepdf.com / gumroad.com 域：${u}`,
    );
  }
  assert.ok(
    allowUrls.some((u) => u === 'https://payoncepdf.com/*'),
    '必须允许 payoncepdf.com',
  );
  assert.ok(
    allowUrls.some((u) => u.includes('gumroad.com')),
    '必须预留 Gumroad 结账域名',
  );
});

test('当前缴费地址确实落在 allow-list 内', () => {
  const m = commerce.match(/export const PAYMENT_URL\s*=\s*"([^"]+)"/);
  assert.ok(m, '必须在 commerce.ts 中声明 PAYMENT_URL 常量');
  const url = m[1];
  assert.match(url, /^https:\/\//, '缴费地址必须是 https');
  assert.ok(
    allowUrls.some((u) => {
      const prefix = u.slice(0, -1); // 去掉 '*'，保留末尾 '/'
      return url.startsWith(prefix);
    }),
    `PAYMENT_URL ${url} 未被任何 allow 条目覆盖`,
  );
  assert.ok(
    !/gumroad\.com\/l\//.test(url),
    'Gumroad 商品未创建，PAYMENT_URL 不得伪造结账直链',
  );
});

test('前端只用 openUrl，没有引入 reveal/openPath 等更宽能力', () => {
  const src = fs.readFileSync(path.join(ROOT, 'src', 'lib', 'commerce.ts'), 'utf8');
  assert.match(src, /openUrl\(/, '缴费入口应使用 openUrl');
  assert.ok(!/revealItemInDir|openPath\(/.test(src), '不得调用 revealItemInDir / openPath');
});
