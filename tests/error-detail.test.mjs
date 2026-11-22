/**
 * error-detail.test.mjs - P0-2 错误详情展示的纯函数单测（2026-11-22）。
 *
 * 直接 import `src/lib/errors.ts`（Node 22 内置 TypeScript 类型剥离），
 * 不启动 Tauri、不碰 DOM。验证后端详细原因被展示、技术噪音被甄别回退。
 * 运行：npm test / node --test tests/error-detail.test.mjs
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { describeErrorDetail, describeError, hintForCode } from '../src/lib/errors.ts';

test('后端中文业务原因完整展示（不再只显示通用码）', () => {
  const err = { code: 'E2004', message: '选区里有大小不同的字（最大 16.7pt / 最小 9.9pt，多半是上下标），已拒绝且未写出文件' };
  const shown = describeErrorDetail(err);
  assert.match(shown, /选区里有大小不同的字/);
  assert.match(shown, /未写出文件/);
  assert.notEqual(shown, hintForCode('E2004'));
});

test('中文原因未自带「未改动」语义时补一句安全保证', () => {
  const shown = describeErrorDetail({ code: 'E2004', message: '框选文字与预览不一致（预览「30」/ 引擎「60」）' });
  assert.match(shown, /框选文字与预览不一致/);
  assert.match(shown, /原文件未改动/);
});

test('自带「已拒绝 / 未改动」的原因不重复补句', () => {
  const a = describeErrorDetail({ code: 'E2004', message: '跨行替换已拒绝，原文件未改动' });
  assert.equal(a, '跨行替换已拒绝，原文件未改动');
});

test('纯英文技术异常回退到稳定 code 文案，不泄漏内部错误串', () => {
  assert.equal(describeErrorDetail({ code: 'E2004', message: 'IO error: No such file or directory (os error 2)' }), hintForCode('E2004'));
  assert.equal(describeErrorDetail({ code: 'E1001', message: 'serde_json error at line 3' }), hintForCode('E1001'));
});

test('多行堆栈 / Rust panic / JS 栈帧 / 超长 dump 一律回退', () => {
  const base = { code: 'E2004' };
  assert.equal(describeErrorDetail({ ...base, message: '中文错误\n  at foo (bar.rs:12:3)' }), hintForCode('E2004'));
  assert.equal(describeErrorDetail({ ...base, message: 'panicked at src/engine/x.rs:9: thread main' }), hintForCode('E2004'));
  assert.equal(describeErrorDetail({ ...base, message: '中文错误 at foo (/path/file.js:10:2)' }), hintForCode('E2004'));
  assert.equal(describeErrorDetail({ ...base, message: '错误'.repeat(200) }), hintForCode('E2004'));
});

test('空 message 与非对象异常回退；各 code 的兜底文案不变', () => {
  assert.equal(describeErrorDetail({ code: 'E2005', message: '   ' }), hintForCode('E2005'));
  assert.equal(describeErrorDetail('boom'), hintForCode('E2004'));
  assert.equal(describeErrorDetail(null), hintForCode('E2004'));
});

test('describeError（许可激活场景）行为保持不变：只返回 code 文案', () => {
  const err = { code: 'E2004', message: '选区字号不一致，已拒绝' };
  assert.equal(describeError(err), hintForCode('E2004'));
  assert.notEqual(describeError(err), describeErrorDetail(err));
});
