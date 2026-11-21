# 最终版：UIA 打开对话框 → UIA ValuePattern 填文件名（同步 IFileDialog 内部状态）
# → 焦点移到「打开」按钮（收起自动完成下拉）→ InvokePattern 确认。
# 顺序回退：UIA Invoke → WM_COMMAND(IDOK) → BM_CLICK。
import ctypes
import os
import subprocess
import sys
import time

import comtypes.client

comtypes.client.GetModule("UIAutomationCore.dll")
from comtypes.gen import UIAutomationClient as UIA  # noqa: E402
from PIL import ImageGrab  # noqa: E402

ROOT = r"E:\PayOncePDF"
EXE = os.path.join(ROOT, r"src-tauri\target\release\payoncepdf.exe")
PDF = os.path.join(ROOT, r"tests\fixtures\text_cjk_special.pdf")
OUT_DIR = os.path.join(ROOT, "tests", "out")

u = ctypes.windll.user32
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)
except Exception:
    u.SetProcessDPIAware()

u.FindWindowW.restype = ctypes.c_void_p
u.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
u.GetWindowRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_long * 4)]
u.SendMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_void_p, ctypes.c_void_p]
u.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]


class RECT(ctypes.Structure):
    _fields_ = [
        ("left", ctypes.c_long), ("top", ctypes.c_long),
        ("right", ctypes.c_long), ("bottom", ctypes.c_long),
    ]


u.GetWindowRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(RECT)]




class _MI2(ctypes.Structure):
    _fields_ = [("dx", ctypes.c_long), ("dy", ctypes.c_long), ("mouseData", ctypes.c_ulong),
                ("dwFlags", ctypes.c_ulong), ("time", ctypes.c_ulong),
                ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong))]


class _U2(ctypes.Union):
    _fields_ = [("mi", _MI2), ("pad", ctypes.c_byte * 24)]


class _IN2(ctypes.Structure):
    _fields_ = [("type", ctypes.c_ulong), ("u", _U2)]


def _send_mouse(flags, nx=0, ny=0):
    a = (_IN2 * 1)()
    a[0].type = 0
    a[0].u.mi = _MI2(nx, ny, 0, flags, 0, None)
    return u.SendInput(1, a, ctypes.sizeof(_IN2))


def drag(x1, y1, x2, y2, steps=14):
    sw, sh = 2559, 1599
    def nx(x): return int(x * 65535 / sw)
    def ny(y): return int(y * 65535 / sh)
    _send_mouse(0x0001 | 0x8000, nx(x1), ny(y1))
    time.sleep(0.2)
    _send_mouse(0x0002 | 0x8000, nx(x1), ny(y1))
    time.sleep(0.15)
    for i in range(1, steps + 1):
        _send_mouse(0x0001 | 0x8000, nx(x1 + (x2 - x1) * i / steps), ny(y1 + (y2 - y1) * i / steps))
        time.sleep(0.04)
    _send_mouse(0x0004 | 0x8000, nx(x2), ny(y2))
    time.sleep(0.4)


def grab(hwnd, name):
    r = RECT()
    u.GetWindowRect(hwnd, ctypes.byref(r))
    img = ImageGrab.grab(bbox=(r.left, r.top, r.right, r.bottom), all_screens=True)
    p = os.path.join(OUT_DIR, name)
    img.save(p)
    print(f"SHOT {p} {img.size} {os.path.getsize(p)}B", flush=True)
    return p


def main():
    print("spawn", EXE, flush=True)
    proc = subprocess.Popen([EXE], cwd=os.path.dirname(EXE))
    try:
        hwnd = 0
        for _ in range(90):
            hwnd = u.FindWindowW(None, "PayOnce PDF")
            if hwnd:
                break
            time.sleep(0.25)
        if not hwnd:
            print("NO_WINDOW")
            return 2
        time.sleep(5.5)
        print("hwnd", hwnd, flush=True)

        uia = comtypes.client.CreateObject(UIA.CUIAutomation, interface=UIA.IUIAutomation)
        root = uia.GetRootElement()

        btn = None
        end = time.time() + 20
        while time.time() < end and btn is None:
            cond = uia.CreatePropertyCondition(
                UIA.UIA_ControlTypePropertyId, UIA.UIA_ButtonControlTypeId
            )
            arr = root.FindAll(UIA.TreeScope_Descendants, cond)
            for i in range(arr.Length):
                try:
                    e = arr.GetElement(i)
                    if "添加文件" in (e.CurrentName or ""):
                        btn = e
                        break
                except Exception:
                    pass
            if btn is None:
                time.sleep(0.6)
        if btn is None:
            print("NO_ADD_BUTTON")
            return 3
        btn.GetCurrentPattern(UIA.UIA_InvokePatternId).QueryInterface(
            UIA.IUIAutomationInvokePattern
        ).Invoke()
        print("dialog requested", flush=True)

        dlg = 0
        for _ in range(30):
            time.sleep(0.3)
            dlg = u.FindWindowW(None, "添加文件")
            if dlg:
                break
        if not dlg:
            print("NO_DIALOG_WINDOW")
            return 4
        time.sleep(1.2)
        print("dialog hwnd", dlg, flush=True)

        dlg_el = uia.ElementFromHandle(dlg)
        if not bool(dlg_el):
            print("NO_DIALOG_ELEMENT")
            return 5

        # 文件名编辑框
        econd = uia.CreatePropertyCondition(UIA.UIA_ControlTypePropertyId, UIA.UIA_EditControlTypeId)
        earr = dlg_el.FindAll(UIA.TreeScope_Descendants, econd)
        edit = None
        for i in range(earr.Length):
            try:
                e = earr.GetElement(i)
                if "文件名" in (e.CurrentName or ""):
                    edit = e
                    break
            except Exception:
                pass
        if edit is None:
            print("NO_FILENAME_EDIT")
            return 6
        edit.SetFocus()
        time.sleep(0.2)
        edit.GetCurrentPattern(UIA.UIA_ValuePatternId).QueryInterface(
            UIA.IUIAutomationValuePattern
        ).SetValue(PDF)
        print("value set:", edit.CurrentValue if hasattr(edit, "CurrentValue") else "", flush=True)
        time.sleep(0.8)

        # 「打开」按钮：先 SetFocus 收掉下拉，再 Invoke
        bcond = uia.CreatePropertyCondition(UIA.UIA_ControlTypePropertyId, UIA.UIA_ButtonControlTypeId)
        barr = dlg_el.FindAll(UIA.TreeScope_Descendants, bcond)
        open_btn = None
        for i in range(barr.Length):
            try:
                e = barr.GetElement(i)
                if (e.CurrentName or "").startswith("打开"):
                    open_btn = e
                    break
            except Exception:
                pass
        if open_btn is None:
            print("NO_OPEN_BUTTON")
            return 7
        print("open button:", open_btn.CurrentName, flush=True)
        open_btn.SetFocus()
        time.sleep(0.5)
        try:
            open_btn.GetCurrentPattern(UIA.UIA_InvokePatternId).QueryInterface(
                UIA.IUIAutomationInvokePattern
            ).Invoke()
            print("invoked open", flush=True)
        except Exception as e:
            print("invoke failed:", e, flush=True)
        time.sleep(2.0)

        if u.FindWindowW(None, "添加文件"):
            print("still open -> WM_COMMAND IDOK", flush=True)
            u.SendMessageW(dlg, 0x0111, ctypes.c_void_p(1), None)
            time.sleep(2.0)
        if u.FindWindowW(None, "添加文件"):
            print("still open -> BM_CLICK fallback", flush=True)
            if open_btn is not None:
                try:
                    for i in range(barr.Length):
                        e = barr.GetElement(i)
                        if (e.CurrentName or "").startswith("打开"):
                            nh = e.CurrentNativeWindowHandle
                            if nh:
                                u.SendMessageW(ctypes.c_void_p(nh), 0x00F5, None, None)
                            break
                except Exception as e:
                    print("bmclick err", e, flush=True)
            time.sleep(2.5)

        time.sleep(3.0)
        grab(hwnd, "real-window-f17-loaded.png")
        print("dialog still open:", bool(u.FindWindowW(None, "添加文件")), flush=True)

        # F18：在渲染页的文字行上拖选（图片坐标 300,178 -> 560,178）
        try:
            r = RECT()
            u.GetWindowRect(hwnd, ctypes.byref(r))
            drag(r.left + 300, r.top + 178, r.left + 560, r.top + 178)
            time.sleep(1.0)
            grab(hwnd, "real-window-f18-selected.png")
            print("drag attempted", flush=True)
        except Exception as e:
            print("drag err:", e, flush=True)
        print("DONE", flush=True)
        return 0
    finally:
        time.sleep(0.4)
        try:
            proc.terminate()
        except Exception:
            pass


if __name__ == "__main__":
    sys.exit(main())
