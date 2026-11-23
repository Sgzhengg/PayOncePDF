/**
 * selfcheck-contract.test.mjs - F21 应用内自检的报告契约与纯展示逻辑单测。
 *
 * 直接 import `src/lib/selfcheck.ts`（Node 22 内置 TypeScript 类型剥离），
 * 不启动 Tauri、不加载 PDFium、不碰 DOM；验证的是前端对自检报告的判定与复制文案，
 * 不是源码字符串断言。运行：npm test / node --test tests/selfcheck-contract.test.mjs
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  caseMark,
  formatSelfCheckText,
  groupSelfCheckCases,
  selfCheckReportDir,
  summarizeSelfCheck,
  verdictText,
} from '../src/lib/selfcheck.ts';

const c = (name, group, ok, skipped = false, detail = 'ok') => ({
  case: name, group, feature: 'F1', ok, skipped, optional: skipped,
  ms: 12, detail,
});

const makeReport = (cases, overrides = {}) => ({
  schema: 'payonce.selftest/1',
  started_at: 1730000000000,
  elapsed_ms: 1234,
  env: { os: 'windows', arch: 'x86_64', app: '1.0.0' },
  summary: { total: 999, passed: 999, failed: 0, skipped: 0 }, // 故意写错，前端必须重算
  coverage: { self_checked: ['F1 合并', 'F10 就地改字'], needs_real_device: ['安装 / 卸载', '拖放'] },
  cases,
  path: 'C:\\Users\\me\\Downloads\\selftest-1730000000000.json',
  ...overrides,
});

test('汇总从 cases 重算，不信任后端 summary', () => {
  const report = makeReport([
    c('env.qpdf_ready', '环境依赖', true),
    c('engine.merge', '引擎端到端', true),
    c('engine.slice', '引擎端到端', false, false, 'E2001 qpdf 缺失'),
    c('ocr.optional_engine', '可选', true, true, 'tessdata 缺失'),
  ]);
  const s = summarizeSelfCheck(report);
  assert.deepEqual(s, { total: 4, passed: 2, failed: 1, skipped: 1, verdict: 'fail' });
});

test('全部通过 / 有跳过 / 有失败 三种结论', () => {
  assert.equal(summarizeSelfCheck(makeReport([c('a', 'g', true), c('b', 'g', true)])).verdict, 'pass');
  assert.equal(summarizeSelfCheck(makeReport([c('a', 'g', true), c('b', 'g', true, true)])).verdict, 'warn');
  assert.equal(summarizeSelfCheck(makeReport([c('a', 'g', false)])).verdict, 'fail');
  assert.equal(summarizeSelfCheck({ cases: undefined }).total, 0);
  assert.match(verdictText('fail'), /失败/);
  assert.match(verdictText('warn'), /跳过/);
  assert.match(verdictText('pass'), /通过/);
});

test('分组保持报告顺序，不丢用例', () => {
  const groups = groupSelfCheckCases([
    c('env.a', '环境依赖', true),
    c('logic.a', '纯逻辑', true),
    c('env.b', '环境依赖', false),
    c('engine.a', '引擎端到端', true),
  ]);
  assert.deepEqual(groups.map((g) => g.group), ['环境依赖', '纯逻辑', '引擎端到端']);
  assert.deepEqual(groups[0].cases.map((x) => x.case), ['env.a', 'env.b']);
  assert.equal(groups.flatMap((g) => g.cases).length, 4);
  assert.deepEqual(groupSelfCheckCases(undefined), []);
});

test('用例标记区分 通过 / 失败 / 跳过', () => {
  assert.equal(caseMark(c('a', 'g', true)), '✓');
  assert.equal(caseMark(c('a', 'g', false)), '✗');
  assert.equal(caseMark(c('a', 'g', true, true)), '—');
});

test('报告目录兼容 Windows / Unix 路径', () => {
  assert.equal(selfCheckReportDir('C:\\Users\\me\\Downloads\\selftest-1.json'), 'C:\\Users\\me\\Downloads');
  assert.equal(selfCheckReportDir('/Users/me/Downloads/selftest-1.json'), '/Users/me/Downloads');
  assert.equal(selfCheckReportDir(''), '');
  assert.equal(selfCheckReportDir('selftest-1.json'), 'selftest-1.json');
});

test('复制文本包含环境、逐条结果与真机剩余清单', () => {
  const report = makeReport([
    c('env.qpdf_ready', '环境依赖', true, false, '就绪 · bin/qpdf.exe'),
    c('engine.replace_text', '引擎端到端', false, false, 'E2002 渲染引擎未加载'),
  ]);
  const text = formatSelfCheckText(report);
  assert.match(text, /payonce\.selftest\/1/);
  assert.match(text, /app=1\.0\.0/);
  assert.match(text, /1\/2 通过，1 失败/);
  assert.match(text, /✓ env\.qpdf_ready/);
  assert.match(text, /✗ engine\.replace_text/);
  assert.match(text, /E2002 渲染引擎未加载/);
  assert.match(text, /已自检覆盖：F1 合并、F10 就地改字/);
  assert.match(text, /- 安装 \/ 卸载/);
  assert.match(text, /报告文件：C:\\Users\\me\\Downloads/);
});

test('报告字段缺失时复制文本不抛错', () => {
  const text = formatSelfCheckText({ schema: 'payonce.selftest/1' });
  assert.match(text, /0\/0 通过/);
  assert.match(text, /仍需真机验证：/);
  assert.match(text, /（无）/);
  assert.match(text, /报告文件：未写出/);
});
