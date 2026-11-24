/**
 * rename-contract.test.mjs - 改名后的前端/配置契约单元测试（Node 22 内置 node:test）。
 * 运行：npm test  /  node --test tests/rename-contract.test.mjs
 * 只做静态契约断言，不启动 Tauri、不依赖 dist 或 Rust 编译产物。
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');
const json = (p) => JSON.parse(read(p));

test('Tauri 配置：应用名、窗口标题、identifier、更新 feed', () => {
  const c = json('src-tauri/tauri.conf.json');
  assert.equal(c.productName, 'PayOnce PDF');
  assert.equal(c.app.windows[0].title, 'PayOnce PDF');
  assert.equal(c.identifier, 'app.payoncepdf.desktop');
  assert.ok(c.plugins.updater.endpoints[0].startsWith('https://payoncepdf.com/api/update/'));
  assert.ok(c.plugins.updater.endpoints[0].includes('{{current_version}}'));
});

test('包名：npm / Cargo / lib crate / main.rs 引用一致', () => {
  const pkg = json('package.json');
  assert.equal(pkg.name, 'payoncepdf');
  const lock = json('package-lock.json');
  assert.equal(lock.name, 'payoncepdf');
  assert.equal(lock.packages[''].name, 'payoncepdf');
  const cargo = read('src-tauri/Cargo.toml');
  assert.match(cargo, /name = "payoncepdf"/);
  assert.match(cargo, /name = "payoncepdf_lib"/);
  assert.match(read('src-tauri/src/main.rs'), /payoncepdf_lib::run\(\)/);
  assert.match(read('src-tauri/Cargo.lock'), /name = "payoncepdf"/);
});

test('许可证：前缀、长度、正则、示例与环境变量全部为 PAYONCE', () => {
  const rust = read('src-tauri/src/license.rs');
  assert.match(rust, /key\.len\(\) != 22/);
  assert.match(rust, /&key\[\.\.8\] != "PAYONCE-"/);
  assert.match(rust, /PAYONCEPDF_ACTIVATE_URL/);
  assert.match(rust, /https:\/\/payoncepdf\.com\/api\/activate/);
  assert.match(rust, /legacy_prefix_is_rejected/);

  const ts = read('src/lib/backend.ts');
  assert.match(ts, /\/\^PAYONCE\(-\[A-Za-z0-9\]\{4\}\)\{3\}\$\/i/);
  assert.match(ts, /PAYONCE-\*\*\*\*-\*\*\*\*-/);
  assert.doesNotMatch(ts, /\/\^ONCE\(/);

  const srv = read('scripts/license-server.mjs');
  assert.match(srv, /const KEY_RE = \/\^PAYONCE-\[A-Z0-9\]\{4\}-/);
  assert.match(srv, /PAYONCEPDF_MOCK_PORT/);
  assert.match(srv, /PAYONCEPDF_ACTIVATE_URL/);

  assert.match(read('src/components/LicensePanel.tsx'), /PAYONCE-XXXX-XXXX-XXXX/);
  assert.match(read('tests/license_test.md'), /PAYONCE-ABCD-EFGH-IJKL/);
  assert.match(read('docs/requirements/PRD.md'), /PAYONCE-XXXX-XXXX-XXXX/);
});

test('许可证正则行为：新前缀接受、旧前缀拒绝（不兼容）', () => {
  const re = /^PAYONCE(-[A-Za-z0-9]{4}){3}$/i;
  assert.ok(re.test('PAYONCE-ABCD-EFGH-IJKL'));
  const legacy = ['ON', 'CE-ABCD-EFGH-IJKL'].join('');
  assert.equal(re.test(legacy), false, '旧前缀必须被拒绝');
  assert.equal(re.test('PAYONCE-ABCD-EFGH'), false);
});

test('事件名：Rust 常量与 TS 监听器使用同一 payoncepdf:// 命名空间', () => {
  const state = read('src-tauri/src/state.rs');
  assert.match(state, /PROGRESS_EVENT: &str = "payoncepdf:\/\/progress"/);
  assert.match(state, /FINISHED_EVENT: &str = "payoncepdf:\/\/finished"/);
  const ts = read('src/lib/backend.ts');
  assert.match(ts, /event\.listen<ProgressEvent>\("payoncepdf:\/\/progress"/);
  assert.match(ts, /event\.listen<JobResult>\("payoncepdf:\/\/finished"/);
});

test('用户可见文案：侧栏、关于页、邮件正文、站点页脚', () => {
  assert.match(read('src/components/Sidebar.tsx'), /PayOnce PDF/);
  assert.match(read('src/components/AboutDialog.tsx'), /关于 PayOnce PDF/);
  assert.match(read('src/lib/params.ts'), /附件由 PayOnce PDF 在本机生成/);
  assert.match(read('index.html'), /<title>PayOnce PDF/);
  assert.match(read('site/index.html'), /Support: hello@payoncepdf\.com/);
  assert.match(read('site/index.zh.html'), /支持邮箱：hello@payoncepdf\.com/);
  assert.match(read('docs/support-template.md'), /hello@payoncepdf\.com/);
});

test('价格冲突已裁决：config/product.json 为单一来源', () => {
  const c = json('config/product.json');
  for (const k of ['price_usd', 'price_cny', 'early_bird_usd', 'early_bird_cny']) {
    assert.equal(typeof c[k], 'number', `${k} 必须是数字`);
  }
  assert.equal(c.price_usd, 39, '2026-11-20 对标 UPDF 后重新定价');
  assert.equal(c.price_cny, 199);
  assert.equal(c.early_bird_usd, 29);
  assert.equal(c.early_bird_cny, 149);
  assert.equal(c.max_devices_per_key, 3, '一把钥匙最多 3 台设备');
  assert.equal(c.pending_confirmation, false, '价格冲突已裁决且新定价已由用户授权拍板');
  assert.ok(c.pricing_v2 && c.pricing_v2.reference.updf_perpetual_cny === 238);
  assert.ok(c.previous_resolution && c.previous_resolution.changed_files.length === 5);
  // 2026-11-24：_in_sync 收敛为「仓库内可核验」的现行价格文件；
  // E:/PainRadar/docs/pdf-launch/* 在本仓库外，已移除，不再冒充已同步。
  assert.ok(c._in_sync.length >= 8, '_in_sync 至少覆盖 8 个仓库内现行价格文件');
  for (const f of c._in_sync) {
    assert.ok(!/^[A-Za-z]:[\\/]/.test(f), `${f} 不能是仓库外的绝对路径`);
    assert.ok(fs.existsSync(path.join(ROOT, f)), `${f} 必须真实存在`);
  }
  assert.equal(c.early_bird_status, 'cancelled_2026-11-24', '早鸟已取消，字段仅作历史记录');
  assert.match(
    c._sync_status_2026_11_24.canonical_source,
    /cancelled_2026-11-24/,
    '事实源必须写明早鸟已取消',
  );
});

test('参考资料正式价已与 config/product.json 一致（仅在该目录存在时检查）', () => {
  const REF = 'E:/PainRadar/docs/pdf-launch';
  if (!fs.existsSync(REF)) return; // 外部参考资料目录：可移植性兜底
  const files = ['landing.html', 'landing.zh.html', 'copy.md', 'posts.md', 'preorder.md'];
  for (const f of files) {
    const s = fs.readFileSync(path.join(REF, f), 'utf8');
    // ¥149 / $29 现在是合法早鸟价；PRD 等文档里的 ¥169 属历史论证，不能一刀切
    assert.match(s, /\$39|¥199/, `${f} 应含 2026-11-20 新价`);
    assert.doesNotMatch(s, /\$29 \/ ¥169/, `${f} 不应再把 $29 / ¥169 写成当前正式价`);
    assert.doesNotMatch(s, /\$19 once/, `${f} 不应再把 $19 写成正式价`);
    assert.doesNotMatch(s, /final price \$29/, `${f} 不应把早鸟价 $29 写成最终价`);
  }
  assert.match(fs.readFileSync(path.join(REF, 'landing.html'), 'utf8'), /final price \$39/);
  assert.match(fs.readFileSync(path.join(REF, 'landing.zh.html'), 'utf8'), /正式价 ¥199/);
});

test('F17 内置阅读视图命令已在前后端注册', () => {
  assert.match(read('src-tauri/src/lib.rs'), /commands::viewer::get_pdf_info/);
  assert.match(read('src-tauri/src/lib.rs'), /commands::viewer::render_page_preview/);
  assert.match(read('src/lib/backend.ts'), /getPdfInfo: "get_pdf_info"/);
  assert.match(read('src/lib/backend.ts'), /renderPagePreview: "render_page_preview"/);
  assert.ok(fs.existsSync(path.join(ROOT, 'src/components/PdfPreview.tsx')));
  assert.match(read('src/App.tsx'), /<PdfPreview/);
});

test('UPDF 对标决策与 OCR 打包缺陷修复已落盘', () => {
  assert.match(read('src-tauri/tauri.conf.json'), /"resources\/tessdata\/\*"/);
  assert.ok(fs.existsSync(path.join(ROOT, 'docs/updf-parity-decision.md')));
  assert.ok(fs.existsSync(path.join(ROOT, 'docs/updf-gap-analysis.md')));
});

test('产物命名脚本与迁移说明存在', () => {
  assert.ok(fs.existsSync(path.join(ROOT, 'scripts/rename-bundles.mjs')));
  assert.match(read('scripts/rename-bundles.mjs'), /PayOnce-PDF-Setup-/);
  assert.match(read('scripts/rename-bundles.mjs'), /PayOnce-PDF-\$\{version\}\.dmg/);
  assert.ok(fs.existsSync(path.join(ROOT, 'docs/RENAME-MIGRATION.md')));
  assert.match(
    read('package.json'),
    /"test": "node --test tests\/rename-contract\.test\.mjs tests\/selection-geometry\.test\.mjs(?: tests\/[a-z-]+\.test\.mjs)*"/,
  );
});

test('F18 画布框选联动：后端命令、坐标口径、前端联动与唯一性守卫', () => {
  const lib = read('src-tauri/src/lib.rs');
  assert.match(lib, /commands::viewer::get_page_text_layout/);
  assert.match(read('src-tauri/src/commands/viewer.rs'), /pub fn get_page_text_layout/);
  assert.match(read('src-tauri/src/engine/render.rs'), /pub fn page_text_layout/);
  assert.match(read('src-tauri/src/engine/render.rs'), /pub const MAX_LAYOUT_CHARS: usize = 60_000/);
  // 2026-11-21 起旋转页可框选：字符框变换到显示坐标后再回传（不禁用）
  const render = read('src-tauri/src/engine/render.rs');
  assert.match(render, /geometry::content_to_display/);
  assert.match(render, /geometry::display_size/);
  assert.match(read('src-tauri/src/engine/geometry.rs'), /pub fn content_to_display/);
  assert.match(read('src-tauri/src/engine/geometry.rs'), /pub fn display_to_content/);
  assert.match(render, /unsupported_reason/);
  assert.match(read('src/lib/backend.ts'), /getPageTextLayout: "get_page_text_layout"/);
  assert.match(read('src/lib/types.ts'), /hasTextLayer: boolean/);
  assert.match(read('src/lib/types.ts'), /unsupportedReason\?: string/);
  assert.ok(fs.existsSync(path.join(ROOT, 'src/lib/selection.ts')));
  assert.match(read('src/components/PdfPreview.tsx'), /onApplySelection/);
  assert.match(read('src/components/PdfPreview.tsx'), /canApplySelection/);
  assert.match(read('src/components/PdfPreview.tsx'), /viewer-selection-bar/);
  // 结果预览：改完立刻看到输出页，且结果态不允许框选（避免看着结果改了原文件）
  assert.match(read('src/App.tsx'), /setResultFile\(/);
  assert.match(read('src/App.tsx'), /const applySelection/);
  assert.match(read('src/App.tsx'), /mode: "redact"/);
  assert.match(read('src/styles.css'), /\.viewer-sel-warn/);
  assert.match(read('package.json'), /selection-geometry\.test\.mjs/);
});

test('F19 按矩形精确替换：命令注册、坐标闸门、前端联动与失败即拒绝', () => {
  const lib = read('src-tauri/src/lib.rs');
  assert.match(lib, /commands::extras::edit_text_at_rect/);
  assert.match(read('src-tauri/src/commands/extras.rs'), /pub fn edit_text_at_rect/);
  assert.match(read('src-tauri/src/commands/mod.rs'), /pub struct RectEditRequest/);

  const engine = read('src-tauri/src/engine/extras.rs');
  assert.match(engine, /pub fn replace_text_at_rect/);
  // 坐标口径：PDF pt、左下原点，与 F18 同一套字符框
  assert.match(engine, /fn pick_chars_by_rect/);
  // 旋转页：选区逆变换回内容流坐标 + 去旋转副本定位 + 写回 /Rotate
  assert.match(engine, /geometry::display_to_content/);
  assert.match(engine, /with_unrotated\(path, out_tmp/);
  assert.match(engine, /fn restore_rotations/);
  // 字符层顺序必须与内容流文本对象一致，否则拒绝（不猜对象）
  assert.match(engine, /run_chars != page_chars/);
  // 跨行必须拒绝，不能把两行挤成一行
  assert.match(engine, /框选跨了多行/);
  // 写后读回：原文出现次数必须恰好减一
  assert.match(engine, /before_count/);
  assert.match(engine, /fn verify_rect_edit/);

  assert.match(read('src/lib/backend.ts'), /editTextAtRect: "edit_text_at_rect"/);
  assert.match(read('src/lib/types.ts'), /interface RectEditRequest/);
  assert.match(read('src/lib/selection.ts'), /export function unionRect/);
  assert.match(read('src/components/PdfPreview.tsx'), /onRectEdit/);
  assert.match(read('src/components/PdfPreview.tsx'), /直接改这一处/);
  assert.match(read('src/App.tsx'), /editTextAtRect/);
  assert.match(read('src/App.tsx'), /editedOutputPath/);
  // 复用统一外壳：授权、取消、进度、原子提交都不另开路径
  assert.match(read('src/App.tsx'), /const startJob/);
});

test('P0-2 错误详情透传：任务失败展示后端中文原因，技术噪音甄别回退', () => {
  const errors = read('src/lib/errors.ts');
  assert.match(errors, /export function describeErrorDetail/);
  assert.match(errors, /纯英文\/技术细节：不向用户暴露/);
  assert.match(errors, /原文件未改动/);
  // 甄别规则：多行 / 超长 / panic / 栈帧都回退
  assert.match(errors, /panicked at/i);

  const app = read('src/App.tsx');
  assert.match(app, /describeErrorDetail, normalizeError/);
  // catch 分支必须用新函数，不得回退到只看 code 的 hintForCode
  const catchBlock = app.slice(app.indexOf('const appErr = normalizeError(e)'));
  assert.match(catchBlock.slice(0, 300), /setStatusText\(describeErrorDetail\(appErr\)\)/);
  assert.doesNotMatch(app, /setStatusText\(hintForCode\(appErr\.code\)\)/);
});

test('F19 宽度溢出预警：纯函数估算 + 用户确认闸门 + 预警不清空不绕过', () => {
  const selection = read('src/lib/selection.ts');
  assert.match(selection, /export function estimateRectEditWidth/);
  assert.match(selection, /export interface RectEditWidthEstimate/);
  assert.match(selection, /likelyOverflow/);
  // 诚实边界写进代码注释：是估算不是真实 advance、不阻断执行
  assert.match(selection, /这是估算，不是引擎的真实 advance 测量/);

  const preview = read('src/components/PdfPreview.tsx');
  assert.match(preview, /estimateRectEditWidth/);
  assert.match(preview, /widthEstimate/);
  // 闸门：溢出一个变量控制放行，且必须有用户确认态
  assert.match(preview, /rectEditBaseReady/);
  assert.match(preview, /const rectEditReady = rectEditBaseReady && \(!mayOverflow \|\| overflowAck\)/);
  assert.match(preview, /setOverflowAck\(false\)/); // 选区/文字变化即撤销确认
  assert.match(preview, /已知风险，仍要改这一处/);
  assert.match(preview, /宽度预警：估算新文字/);
  assert.match(read('src/styles.css'), /\.viewer-sel-overflow/);
});

test('F20 文档比对：命令注册、只读查询、文字层 diff、诚实边界与前端联动', () => {
  const lib = read('src-tauri/src/lib.rs');
  assert.match(lib, /commands::compare::compare_documents/);
  assert.match(lib, /commands::compare::export_compare_report/);
  const cmd = read('src-tauri/src/commands/compare.rs');
  assert.match(cmd, /pub fn compare_documents/);
  assert.match(cmd, /pub fn export_compare_report/);
  const engine = read('src-tauri/src/engine/compare.rs');
  // 纯逻辑 diff + 逐页提取 + 扫描件检测（只读，不写文件）
  assert.match(engine, /fn tokenize/);
  assert.match(engine, /fn lcs_diff/);
  assert.match(engine, /pub fn diff_text/);
  assert.match(engine, /pub fn compare_with_pdfium/);
  assert.match(engine, /render::PdfDoc::open/);
  assert.match(engine, /has_text_layer/);
  assert.match(engine, /请先走 OCR 再比对/);
  // 超限失败而非 OOM
  assert.match(engine, /TOKEN_LIMIT/);
  assert.match(engine, /CHAR_LIMIT/);
  // 边界写进报告与引擎注释
  assert.match(engine, /不做\*\*图片、版式/);
  assert.match(engine, /本报告仅比对文字层文本/);
  assert.match(read('src-tauri/src/engine/mod.rs'), /pub mod compare;/);
  // 前端：工具入口 + 命令名 + 面板 + 只读查询封装 + 结果着色样式
  assert.match(read('src/lib/types.ts'), /\| "compare"/);
  assert.match(read('src/lib/types.ts'), /interface CompareReport/);
  assert.match(read('src/lib/params.ts'), /id: "compare"/);
  assert.match(read('src/lib/backend.ts'), /compareDocuments: "compare_documents"/);
  assert.match(read('src/lib/backend.ts'), /export async function compareDocuments/);
  assert.match(read('src/components/ParamPanel.tsx'), /case "compare"/);
  assert.ok(fs.existsSync(path.join(ROOT, 'src/panels/ComparePanel.tsx')));
  assert.match(read('src/panels/ComparePanel.tsx'), /不识别图片、版式、页序/);
  assert.match(read('src/styles.css'), /\.compare-diff-list/);
});
