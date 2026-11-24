/**
 * site-contract.test.mjs - 官网部署物与一键部署脚本的静态契约（Node 22 内置 node:test）。
 * 并入 npm test，让 CI 的 quality job 在构建前守住官网结构，不依赖网络或 Cloudflare 账号。
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkLocalFiles, PAGES } from '../scripts/verify-cloudflare-site.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

test('官网部署目录通过全部本地验收门禁', () => {
  const gate = checkLocalFiles();
  assert.deepEqual(gate.blockers, [], `官网存在阻断项：${gate.blockers.join(' | ')}`);
  assert.equal(gate.stats.pages, 8);
  assert.ok(gate.stats.links > 100, '站内链接总数应符合 8 页规模');
});

test('一键部署脚本默认只检查，显式 --deploy 才部署', () => {
  const src = read('scripts/deploy-cloudflare-site.mjs');
  assert.match(src, /if \(!opts\.deploy\)/, '必须存在「非 --deploy 就返回」的分支');
  assert.match(src, /wrangler login/, '未登录时必须给出 login 指引');
  assert.doesNotMatch(src, /spawnSync\([^)]*['"]login['"]/, '不得自动执行 wrangler login');
  assert.match(src, /pages/, '部署命令必须是 pages deploy');
  assert.match(src, /--project-name/, '部署命令必须指定项目名');
});

test('部署脚本复用验收门禁，且只从 site-cloudflare/ 部署', () => {
  const src = read('scripts/deploy-cloudflare-site.mjs');
  assert.match(src, /checkLocalFiles\(\)/, '部署前必须跑本地验收');
  assert.match(src, /if \(gate\.blockers\.length\)[\s\S]*?process\.exitCode = 1/, '门禁失败必须中止');
  assert.match(src, /cwd: SITE/, 'wrangler 的工作目录必须是 site-cloudflare/');
  assert.match(read('site-cloudflare/wrangler.toml'), /pages_build_output_dir = "\."/);
});

test('npm 提供 site:check / site:deploy / site:verify 三个入口', () => {
  const pkg = JSON.parse(read('package.json'));
  assert.equal(pkg.scripts['site:check'], 'node scripts/deploy-cloudflare-site.mjs --check');
  assert.equal(pkg.scripts['site:deploy'], 'node scripts/deploy-cloudflare-site.mjs --deploy');
  assert.equal(pkg.scripts['site:verify'], 'node scripts/verify-cloudflare-site.mjs');
  assert.match(pkg.scripts.test, /site-contract\.test\.mjs/, '官网契约必须进入 CI 门禁');
});

test('8 页中英镜像齐全，Gumroad 保持占位且不造假直链', () => {
  assert.equal(PAGES.length, 8);
  const all = PAGES.map((p) => read(`site-cloudflare/${p}`)).join('\n');
  assert.doesNotMatch(all, /href="https:\/\/gumroad\.com\/l\//i);
  assert.equal((all.match(/aria-disabled="true"/g) || []).length, 2);
  assert.doesNotMatch(all, /\$29|¥149/, '部署物不得残留早鸟价');
});
