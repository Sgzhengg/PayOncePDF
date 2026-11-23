/**
 * trial-paywall.test.mjs - 试用 → 缴费 → 买断 分叉的纯逻辑单测。
 *
 * 直接 import `src/lib/commerce.ts`（Node 22 内置 TypeScript 类型剥离），
 * 不启动 Tauri、不打开外部链接。运行：npm test
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { shouldShowPaywall, licenseHeadline, PAYMENT_URL } from '../src/lib/commerce.ts';

const trial = { status: 'trial', trialDaysLeft: 7, canExport: true };
const active = { status: 'active', trialDaysLeft: 0, activatedAt: 1700000000000, canExport: true };
const expired = { status: 'expired', trialDaysLeft: 0, canExport: false };

test('下载入口：试用期内不弹缴费窗口', () => {
  assert.equal(shouldShowPaywall(null), false);
  assert.equal(shouldShowPaywall(trial), false);
  assert.match(licenseHeadline(trial), /剩余 7 天/);
});

test('下载入口：试用到期弹出缴费窗口，文件仍在', () => {
  assert.equal(shouldShowPaywall(expired), true);
  assert.match(licenseHeadline(expired), /文件仍在/);
});

test('购买入口：已激活用户到期直接永久，永不弹缴费窗口', () => {
  assert.equal(shouldShowPaywall(active), false);
  assert.match(licenseHeadline(active), /永久使用/);
});

test('缴费地址真实可达，不伪造 Gumroad 无效直链', () => {
  assert.match(PAYMENT_URL, /^https:\/\//);
  assert.ok(!PAYMENT_URL.includes('gumroad.com/l/'));
  assert.match(PAYMENT_URL, /payoncepdf\.com\/pricing$/);
});
