#!/usr/bin/env python3
"""PayOnce PDF 官网本地预览服务器（Python 标准库，零依赖）。

模拟 Cloudflare Pages 原生的 clean URL 行为，让线上使用的"干净 URL"在本地也能打开：

    python scripts/serve-cloudflare-site.py             # http://127.0.0.1:8000/
    python scripts/serve-cloudflare-site.py 8899        # 指定端口

页面内链接已全部直连 `.html`，因此直接用 `python -m http.server` 也能正常浏览；
本脚本只是额外补上 /download、/pricing、/help、/en/… 这类无扩展名路径，
方便部署前完整验证 `_redirects` 的行为，避免本地点导航出现 404。

本脚本放在 `scripts/` 而不是 `site-cloudflare/`：Cloudflare Pages 的
`pages_build_output_dir = "."` 会把部署目录内所有文件当作静态资源发布，
本地预览工具与 .pyc 不应出现在公网。
"""

from __future__ import annotations

import functools
import http.server
import pathlib
import sys
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parent.parent / "site-cloudflare"

# 模拟 Pages 原生 clean URL：无扩展名路径内部映射到 .html 并返回 200。
# 注意：线上 _redirects **不能**写这些 200 rewrite，否则会与 Pages 的
# .html → 无扩展名 308 互相来回，形成无限重定向循环（2026-11-24 线上实测）。
# 本脚本只负责让本地普通静态服务器补上这个映射。
REWRITES = {
    "/": "/index.html",
    "/download": "/download.html",
    "/pricing": "/pricing.html",
    "/help": "/help.html",
    "/en": "/en/index.html",
    "/en/": "/en/index.html",
    "/en/download": "/en/download.html",
    "/en/pricing": "/en/pricing.html",
    "/en/help": "/en/help.html",
}

# 与 _redirects 中的 301 保持一致（旧链接兼容）
REDIRECTS = {
    "/index.en.html": "/en/",
    "/zh/": "/",
}


class PagesHandler(http.server.SimpleHTTPRequestHandler):
    def _prepare(self) -> bool:
        """应用重定向/重写；返回 False 表示响应已发出。"""
        parts = urllib.parse.urlsplit(self.path)
        if parts.path in REDIRECTS:
            self.send_response(301)
            self.send_header("Location", REDIRECTS[parts.path])
            self.end_headers()
            return False
        if parts.path in REWRITES:
            query = "?" + parts.query if parts.query else ""
            self.path = REWRITES[parts.path] + query
        return True

    def do_GET(self):
        if self._prepare():
            super().do_GET()

    def do_HEAD(self):
        if self._prepare():
            super().do_HEAD()

    def end_headers(self):
        # 本地改完刷新即可看到，不做缓存
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    handler = functools.partial(PagesHandler, directory=str(ROOT))
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as srv:
        print(f"PayOnce PDF preview: http://127.0.0.1:{port}/  (Ctrl+C to stop)", flush=True)
        try:
            srv.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
