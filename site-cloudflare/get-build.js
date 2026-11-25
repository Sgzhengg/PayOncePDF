/**
 * get-build.js — 「立即开始 7 天试用」的落地交互（纯本地，无任何网络请求）
 *
 * 背景：下载通道开放前，试用入口只能靠邮件登记。但直接放 `mailto:` 链接有个坑：
 * 系统没配置默认邮件客户端时，点击**完全没有任何反应**（2026-11-25 实测复现），
 * 用户会以为按钮坏了、进不了下载页。
 *
 * 所以改成：点击 → 就地展开获取方式 + 自动复制整封邮件 → 给出明确的复制结果，
 * mailto 只作为「已配置邮件程序」时的可选路径。
 *
 * 约定：
 * - 触发器按钮：`[data-getbuild-open]`，用 `data-widget` 指到下面的容器 id；
 * - 容器：`<div class="getbuild" id="...">`，内含 `.getbuild-panel` 与
 *   `[data-copy="email"]` / `[data-copy="mail"]` 两个复制按钮，文案从 data-* 读取；
 * - 状态提示写进 `.getbuild-status`（aria-live=polite）。
 * - 本文件不得发起任何网络请求（站点 CSP 为 script-src 'self'，且校验脚本契约会检查）。
 */
(function () {
  'use strict';

  // 中英共用一个脚本，提示语按 <html lang> 切换，避免英文页冒出中文
  var MSG = /^en/i.test(document.documentElement.lang || '')
    ? {
        autoOk: 'The full email is on your clipboard \u2014 paste it into any mailbox and send.',
        autoFail: 'Options are open below: use \u201cCopy email\u201d or \u201cCopy full email\u201d, then paste into any mailbox.',
        copiedEmail: 'Email address copied: ',
        copiedMail: 'Full email copied \u2014 paste it into any mailbox and send.',
        copyFail: 'Copy failed: select the text and copy it manually, or write to ',
      }
    : {
        autoOk: '已把整封邮件复制到剪贴板：粘贴到任意邮箱发送即可。',
        autoFail: '方式已展开：点下面的「复制邮箱」或「复制完整邮件」，再粘贴到任意邮箱发送。',
        copiedEmail: '邮箱地址已复制：',
        copiedMail: '整封邮件已复制，粘贴到任意邮箱发送即可。',
        copyFail: '复制失败：请手动选中文本复制，或直接发到 ',
      };

  function text(el, name) {
    var v = el.getAttribute(name);
    return v == null ? '' : v;
  }

  function setStatus(box, msg, bad) {
    var s = box.querySelector('.getbuild-status');
    if (!s) return;
    s.textContent = msg;
    s.classList.toggle('bad', Boolean(bad));
  }

  function legacyCopy(value) {
    var ta = document.createElement('textarea');
    ta.value = value;
    ta.setAttribute('readonly', '');
    ta.style.position = 'fixed';
    ta.style.top = '-2000px';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    var ok = false;
    try {
      ta.select();
      ta.setSelectionRange(0, value.length);
      ok = document.execCommand('copy');
    } catch (e) {
      ok = false;
    }
    document.body.removeChild(ta);
    return ok;
  }

  function copy(value) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(value).then(
        function () { return true; },
        function () { return legacyCopy(value); }
      );
    }
    return Promise.resolve(legacyCopy(value));
  }

  function mailBody(box) {
    var body = text(box, 'data-mail');
    // data-* 里的换行写成字面 \n，这里还原成真换行，方便粘贴进邮件客户端
    return body.replace(/\\n/g, '\n');
  }

  function open(box, trigger) {
    var panel = box.querySelector('.getbuild-panel');
    if (!panel) return;
    var wasHidden = panel.hasAttribute('hidden');
    panel.removeAttribute('hidden');
    box.classList.add('is-open');
    if (trigger) trigger.setAttribute('aria-expanded', 'true');
    if (wasHidden) {
      panel.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      var first = panel.querySelector('button, a');
      if (first) {
        try { first.focus({ preventScroll: true }); } catch (e) { first.focus(); }
      }
    }
    // 点按钮即用户手势，此时复制成功率高；失败也不阻断，下面还有显式复制按钮。
    // 自动复制失败时不报错（用户并没有要求复制），只给中性指引，避免一上来就是红字。
    copy(mailBody(box)).then(function (ok) {
      setStatus(box, ok ? MSG.autoOk : MSG.autoFail, false);
    });
  }

  function wireCopy(box) {
    var pairs = [
      ['email', 'data-email', MSG.copiedEmail],
      ['mail', 'data-mail', MSG.copiedMail],
    ];
    pairs.forEach(function (p) {
      var btn = box.querySelector('[data-copy="' + p[0] + '"]');
      if (!btn) return;
      btn.addEventListener('click', function () {
        var value = p[0] === 'mail' ? mailBody(box) : text(box, p[1]);
        copy(value).then(function (ok) {
          if (!ok) {
            setStatus(box, MSG.copyFail + text(box, 'data-email') + '.', true);
            return;
          }
          setStatus(box, p[2] + (p[0] === 'email' ? text(box, p[1]) : ''), false);
        });
      });
    });
  }

  function init() {
    var boxes = document.querySelectorAll('.getbuild');
    Array.prototype.forEach.call(boxes, function (box) {
      wireCopy(box);
    });
    var triggers = document.querySelectorAll('[data-getbuild-open]');
    Array.prototype.forEach.call(triggers, function (t) {
      var id = t.getAttribute('data-widget');
      var box = id ? document.getElementById(id) : t.closest('.getbuild');
      if (!box) return;
      t.setAttribute('aria-expanded', 'false');
      t.setAttribute('aria-controls', box.id || '');
      t.addEventListener('click', function (ev) {
        ev.preventDefault();
        open(box, t);
      });
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
