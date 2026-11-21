/**
 * defect-regression.test.mjs - 竞品对标审查后修复的功能缺陷回归（Node 22 node:test）。
 * 运行：npm test  /  node --test tests/defect-regression.test.mjs
 *
 * 只做静态契约断言：不启动 Tauri、不依赖 dist 或 Rust 编译产物。
 * 锁住的是「曾经出过错、以后不能再犯」的具体行为约束。
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

const app = () => read('src/App.tsx');
const pdfRs = () => read('src-tauri/src/engine/pdf.rs');
const sidecarRs = () => read('src-tauri/src/engine/sidecar.rs');

// ---- 缺陷 1：F19 链式状态残留，可能让新结果误开启框选 ----

test('换输入 / 移文件 / 清空 / 回原件 / 非链式任务都会清掉链式权限', () => {
  const s = app();
  assert.match(s, /const resetChainState = useCallback\(\(\) => \{[\s\S]*?setChainableResultPath\(null\);[\s\S]*?ownEditedRef\.current\.clear\(\);/, '缺少统一清理函数');
  // 五个入口都必须调用；少一个就会留下「看着 A 改了 B」的窗口
  for (const [name, re] of [
    ['addItems', /const addItems[\s\S]{0,300}?resetChainState\(\)/],
    ['removeFile', /const removeFile[\s\S]{0,200}?resetChainState\(\)/],
    ['moveFile', /const moveFile[\s\S]{0,200}?resetChainState\(\)/],
    ['clearFiles', /const clearFiles[\s\S]{0,200}?resetChainState\(\)/],
    ['applySelection', /const applySelection[\s\S]{0,300}?resetChainState\(\)/],
    ['onBackToInput', /onBackToInput=\{\(\) => \{[\s\S]{0,600}?resetChainState\(\)/],
  ]) {
    assert.match(s, re, `${name} 未清链式状态`);
  }
  // 连续编辑必须保留状态：只在非 keepPreview 分支清
  assert.match(s, /if \(!opts\?\.keepPreview\) \{[\s\S]{0,200}?resetChainState\(\);[\s\S]{0,80}?\}/);
  assert.match(s, /if \(opts\?\.chainableResult\) setChainableResultPath\(firstOutput\)/);
});

// ---- 缺陷 2：onUsePage 对无页码工具谎报「已填入」 ----

test('onUsePage 对没有 pages 参数的工具如实提示，不再假成功', () => {
  const s = app();
  const body = s.match(/onUsePage=\{\(n\) => \{[\s\S]*?\n            \}\}/)?.[0] ?? '';
  assert.ok(body, '找不到 onUsePage 回调');
  assert.match(body, /if \(!cur \|\| !\("pages" in cur\)\)/, '没有无页码工具的分支');
  assert.match(body, /不使用页码，无需填写/, '缺少如实提示');
  assert.match(body, /setStatusText\(`已把第 \$\{n\} 页填入右侧参数面板`\)/, '正常路径仍应提示成功');
});

// ---- 缺陷 3：表单扁平化中文静默变问号（数据损坏） ----

test('表单扁平化：Latin-1 正确编码，非 WinAnsi 明确拒绝且不再出现 ? 降级', () => {
  const s = pdfRs();
  assert.match(s, /fn winansi_bytes/, '缺少 WinAnsi 编码器');
  assert.match(s, /'\\u\{00A0\}'\.\.='\\u\{00FF\}'/, '未放行 Latin-1 范围');
  assert.match(s, /return Err\(other\)/, '未拒绝非 WinAnsi 字符');
  assert.match(s, /if flatten \{[\s\S]{0,400}?winansi_bytes\(value\)[\s\S]{0,300}?AppError::parse/, '写入前未校验字段值');
  assert.match(s, /无法扁平化的字符/, '缺少明确拒绝提示');
  // 旧实现把所有非 ASCII 压成 '?'——这个表达式必须彻底消失
  assert.doesNotMatch(s, /_ => out\.push\('\?'\),/);
  assert.doesNotMatch(s, /escape_pdf_string/);
});

// ---- 缺陷 4：OCR 语言包下载只检查大小，错误页会落盘 ----

test('OCR 语言包下载做结构校验，不信任文件名和大小', () => {
  const s = sidecarRs();
  assert.match(s, /fn validate_traineddata/, '缺少结构校验函数');
  assert.match(s, /bytes\.first\(\) != Some\(&0x18\)/, '未校验 protobuf 文件头');
  assert.match(s, /0xFF/, '未校验二进制填充特征');
  assert.match(s, /if let Err\(reason\) = validate_traineddata\(&bytes\)/, '下载后未调用校验');
  assert.match(s, /AppError::ocr_lang\(/, '校验失败应使用 OCR 专用错误码');
  // 旧的「只查大小」逻辑必须移除
  assert.doesNotMatch(s, /if bytes\.len\(\) < 50_000 \{\s*return Err\(AppError::internal/);
});

// ---- 缺陷 5：书签层级被压平（合并保住层级，管理一操作就全变顶层） ----

const objectsRs = () => read('src-tauri/src/engine/objects.rs');
const morePanels = () => read('src/panels/MorePanels.tsx');

test('书签管理：spec 支持 > 层级且跳跃报错，写回用 parent 栈保层级', () => {
  const s = objectsRs();
  // spec 解析层级：行首 > 计数 = 层级
  assert.match(s, /let level_raw = line\.bytes\(\)\.take_while\(\|&b\| b == b'>'\)\.count\(\) as u32 \+ 1/, 'spec 未按 > 解析层级');
  assert.match(s, /level_raw > prev_level \+ 1/, '未拒绝层级跳跃');
  assert.match(s, /BookmarkEntry \{\s*level: level_raw/, '解析结果未带层级');
  // 写回 parent 栈：旧实现 add_bookmark(..., None) 把层级压平，必须消失
  assert.match(s, /let mut stack: Vec<\(u32, u32\)> = Vec::new\(\)/, '写回缺少 parent 栈');
  assert.match(s, /let parent = stack\.last\(\)\.map\(\|\(_, id\)\| \*id\)/, '未取上级作为 parent');
  assert.match(s, /Bookmark::new\(e\.title\.clone\(\), \[0\.0, 0\.0, 0\.0\], 0, pid\),\s*parent,/, 'add_bookmark 未传 parent');
  assert.doesNotMatch(s, /Bookmark::new\(e\.title\.clone\(\), \[0\.0, 0\.0, 0\.0\], 0, pid\),\s*None,/, '旧压平写回仍残留');
  // 文件头注释不能再说「只做顶层」
  assert.doesNotMatch(s, /书签管理只做顶层/, '过时注释未更新');
});

test('书签面板：提示支持多级、列表按 level 缩进、不再谎称只有顶层', () => {
  const s = morePanels();
  assert.match(s, /支持多级书签（行首 > 表示子级）/, '面板未说明多级支持');
  assert.match(s, /行首加 > 表示子级、>> 表示孙级/, 'spec 输入未说明层级格式');
  assert.match(s, /style=\{\{ paddingLeft: \(b\.level - 1\) \* 16 \}\}/, '列表未按 level 缩进');
  assert.match(s, /Math\.max\(\.\.\.items\.map\(\(b\) => b\.level\)\)/, '读取提示未展示最深层级');
  assert.doesNotMatch(s, /只做顶层书签/, '过时提示未更新');
});
