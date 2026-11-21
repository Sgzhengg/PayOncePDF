//! 旋转页坐标变换（F18 框选 / F19 按位置替换）。
//!
//! 实测（2026-11-21，随包 PDFium 0.9.5）：
//! - `page.width()/height()` 与 `render_page_png()` 都是**显示尺寸**：/Rotate 90/270 时
//!   内容流 595×842 的页显示为 842×595；/Rotate 0/180 显示尺寸=内容流尺寸。
//! - `PdfTextChar::tight_bounds()/loose_bounds()` 永远是**内容流坐标**（左下原点），
//!   不随 /Rotate 变化（四个角度实测字符框完全一致）。
//!
//! 因此：F18 回传给前端的字符框必须变换到显示坐标系，才能与「屏幕→显示 pt」的
//! 选区求交；F19 收到前端的显示坐标选区后必须逆变换回内容流坐标，才能命中
//! PDFium 的内容流字符框。两个方向都在这里定义，并做 round-trip 单测。
//!
//! 矩形统一表示为 (left, bottom, right, top)，左下原点，单位 pt。

/// PDF 页面 /Rotate（顺时针角度，0/90/180/270）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl PageRotation {
    pub fn from_degrees(d: u32) -> Self {
        match d % 360 {
            90 => PageRotation::Deg90,
            180 => PageRotation::Deg180,
            270 => PageRotation::Deg270,
            _ => PageRotation::Deg0,
        }
    }

    pub fn degrees(self) -> u32 {
        match self {
            PageRotation::Deg0 => 0,
            PageRotation::Deg90 => 90,
            PageRotation::Deg180 => 180,
            PageRotation::Deg270 => 270,
        }
    }

    /// 90/270 交换宽高，0/180 不变。
    pub fn swaps_axes(self) -> bool {
        matches!(self, PageRotation::Deg90 | PageRotation::Deg270)
    }
}

pub type Rect = (f32, f32, f32, f32);

/// 内容流尺寸 → 显示尺寸（90/270 交换宽高）。
pub fn display_size(rot: PageRotation, content_w: f32, content_h: f32) -> (f32, f32) {
    if rot.swaps_axes() {
        (content_h, content_w)
    } else {
        (content_w, content_h)
    }
}

/// 内容流矩形 → 显示矩形。
///
/// `content_w/h` 是内容流（MediaBox）尺寸。`display_w/h` 由 `display_size()` 得来。
/// 变换是绕原点的刚体旋转 + 平移，逆时针数学坐标、左下原点：
/// - 0°：恒等
/// - 90°（页面顺时针）：(x,y) → (y, content_w - x)
/// - 180°：(x,y) → (content_w - x, content_h - y)
/// - 270°：(x,y) → (content_h - y, x)
pub fn content_to_display(rot: PageRotation, r: Rect, content_w: f32, content_h: f32) -> Rect {
    let (l, b, rgt, t) = r;
    match rot {
        PageRotation::Deg0 => r,
        PageRotation::Deg90 => (b, content_w - rgt, t, content_w - l),
        PageRotation::Deg180 => (content_w - rgt, content_h - t, content_w - l, content_h - b),
        PageRotation::Deg270 => (content_h - t, l, content_h - b, rgt),
    }
}

/// 显示矩形 → 内容流矩形（`content_to_display` 的逆）。
///
/// 用显示尺寸做逆变换：90° 逆为 (x,y)→(display_h - y, x)；270° 逆为 (y, display_w - x)。
pub fn display_to_content(rot: PageRotation, r: Rect, display_w: f32, display_h: f32) -> Rect {
    let (l, b, rgt, t) = r;
    match rot {
        PageRotation::Deg0 => r,
        PageRotation::Deg90 => (display_h - t, l, display_h - b, rgt),
        PageRotation::Deg180 => (display_w - rgt, display_h - t, display_w - l, display_h - b),
        PageRotation::Deg270 => (b, display_w - rgt, t, display_w - l),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 0.01, "{a} != {b}");
    }

    fn approx_rect(a: Rect, b: Rect) {
        approx(a.0, b.0);
        approx(a.1, b.1);
        approx(a.2, b.2);
        approx(a.3, b.3);
    }

    /// 实测锚点：内容流 595×842，"H" 字符框 (73.4,700)-(83.6,712.9)。
    /// 90° 后显示 842×595，字符应落在显示右上区域附近。
    #[test]
    fn ninety_degree_content_to_display_matches_real_pdfium() {
        let r = (73.4, 700.0, 83.6, 712.9);
        let d = content_to_display(PageRotation::Deg90, r, 595.0, 842.0);
        // (x,y)→(y,595-x)：left=bottom=700, bottom=595-right=511.4,
        // right=top=712.9, top=595-left=521.6
        approx_rect(d, (700.0, 511.4, 712.9, 521.6));
        // 显示尺寸 842×595，字符框落在其内（右侧偏上）。
        assert!(d.0 >= 0.0 && d.2 <= 842.0 && d.1 >= 0.0 && d.3 <= 595.0);
    }

    /// 四个角度双向 round-trip 必须还原内容流矩形（F19 正确性基石）。
    #[test]
    fn all_rotations_round_trip() {
        let (cw, ch) = (595.0, 842.0);
        let r = (72.0, 640.0, 228.0, 664.0); // 内容流里一段文字
        for rot in [
            PageRotation::Deg0,
            PageRotation::Deg90,
            PageRotation::Deg180,
            PageRotation::Deg270,
        ] {
            let (dw, dh) = display_size(rot, cw, ch);
            let d = content_to_display(rot, r, cw, ch);
            assert!(d.0 >= 0.0 && d.2 <= dw && d.1 >= 0.0 && d.3 <= dh, "{rot:?}: {d:?}");
            let back = display_to_content(rot, d, dw, dh);
            approx_rect(back, r);
        }
    }

    #[test]
    fn display_size_swaps_only_for_odd_quarters() {
        assert_eq!(display_size(PageRotation::Deg0, 595.0, 842.0), (595.0, 842.0));
        assert_eq!(display_size(PageRotation::Deg90, 595.0, 842.0), (842.0, 595.0));
        assert_eq!(display_size(PageRotation::Deg180, 595.0, 842.0), (595.0, 842.0));
        assert_eq!(display_size(PageRotation::Deg270, 595.0, 842.0), (842.0, 595.0));
    }

    /// 逆变换用显示尺寸：把用户拖出来的显示矩形还原成内容流矩形。
    #[test]
    fn inverse_uses_display_size() {
        // 90°：用户框住显示矩形 (700,511.4)-(712.9,521.6)，显示尺寸 842×595
        let back = display_to_content(PageRotation::Deg90, (700.0, 511.4, 712.9, 521.6), 842.0, 595.0);
        approx_rect(back, (73.4, 700.0, 83.6, 712.9));
    }
}
