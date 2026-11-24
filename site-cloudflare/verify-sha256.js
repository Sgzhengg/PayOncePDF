/*
 * PayOnce PDF — 浏览器内 SHA-256 校验（参考 zhuazhua.video/download 的做法）。
 *
 * 约束（有契约测试守护）：
 *  1. 文件只在浏览器本机读取，绝不上传，全程无任何网络请求；
 *  2. 通过外部脚本加载，兼容站点 CSP 的 script-src 'self'（不使用内联脚本 / eval）；
 *  3. 期望值来自页面上的 data-hash 属性，与 README / 测试报告中的发行哈希一致；
 *  4. 结果只用 textContent 写入，不拼接 HTML。
 */
(function () {
  'use strict';

  var input = document.getElementById('pkg-file');
  var out = document.getElementById('pkg-result');
  var expectedEl = document.getElementById('pkg-sha256');
  if (!input || !out || !expectedEl) return;

  var expected = (expectedEl.getAttribute('data-hash') || '').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(expected)) {
    out.textContent = '校验值配置异常，请邮件联系我们，不要使用该文件。';
    return;
  }
  if (!window.crypto || !window.crypto.subtle || typeof File === 'undefined') {
    out.textContent = '当前浏览器不支持 WebCrypto，请用下方命令行方式核对。';
    return;
  }

  input.addEventListener('change', function () {
    var file = input.files && input.files[0];
    if (!file) return;
    out.textContent = '正在本机计算 SHA-256…（文件不会上传）';

    file.arrayBuffer()
      .then(function (buffer) {
        return window.crypto.subtle.digest('SHA-256', buffer);
      })
      .then(function (digest) {
        var bytes = new Uint8Array(digest);
        var hex = Array.prototype.map.call(bytes, function (b) {
          return ('0' + b.toString(16)).slice(-2);
        }).join('');
        if (hex === expected) {
          out.textContent = '✓ 校验值一致：文件完整、未被篡改。\n' + hex;
        } else {
          out.textContent = '✗ 校验值不一致：请删除该文件并重新获取安装包。\n本机计算：' + hex
            + '\n官方校验值：' + expected;
        }
      })
      .catch(function (err) {
        out.textContent = '无法读取该文件：' + (err && err.message ? err.message : String(err));
      });
  });
})();
