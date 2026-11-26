//! KnuPdf: printing a sheet as pixels, a band at a time. The print-ready PDF is drawn by the
//! application's own renderer at the printer's resolution, in bands small enough that a page
//! never needs more than a few tens of megabytes, and each band goes to the printer through GDI
//! (`windows.rs`). Blank parts of a band are not sent. Everything here is plain arithmetic on
//! pixels, so it is tested on every platform.

/// The most rendered pixels per inch. A printer that asks for more (1200 dpi) gets 600 stretched
/// by its driver: past 600 a page takes four times as long to draw for no difference on paper.
pub const MAX_DPI: f64 = 600.0;
/// The least: below 300 dpi text prints with soft edges.
pub const MIN_DPI: f64 = 300.0;
/// A band's most rows and columns (the renderer draws at most 4096 pixels a side per tile).
pub const BAND_ROWS: u32 = 1024;
pub const BAND_COLUMNS: u32 = 4096;
/// The most pixels a sheet may have at the chosen resolution (a poster sheet is drawn at a
/// lower one): 600 dpi on A0.
const MAX_SHEET_PIXELS: f64 = 700.0e6;

/// What the printer's device context says about the paper, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    pub dpi_x: f64,
    pub dpi_y: f64,
    /// The paper's size.
    pub width: f64,
    pub height: f64,
    /// Where the printable area starts on the paper (the device context's origin).
    pub offset_x: f64,
    pub offset_y: f64,
}

/// How one sheet is drawn: at `scale` rendered pixels per point, `width` × `height` pixels,
/// turned a quarter clockwise when the paper lies the other way round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub scale: f64,
    pub width: u32,
    pub height: u32,
    pub rotate: bool,
    /// Device pixels per rendered pixel, across and down the paper.
    fx: f64,
    fy: f64,
    offset_x: f64,
    offset_y: f64,
}

/// A rectangle of rendered pixels, or of device pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Where pixels land on the paper, in device pixels relative to the printable area's origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// The pixel size of `points` at `scale`: whole pixels, at least 1.
fn pixels(points: f64, scale: f64) -> u32 {
    let px = (points * scale - 0.01).ceil();
    if px.is_finite() && px >= 1.0 { px.min(f64::from(u32::MAX)) as u32 } else { 1 }
}

/// How a `sheet` (width, height in points) is drawn on `device`. `None` when the device or the
/// sheet makes no sense (no resolution, an empty sheet).
pub fn place(sheet: (f64, f64), device: Device) -> Option<Placement> {
    let (sw, sh) = sheet;
    let usable = |v: f64| v.is_finite() && v > 0.0;
    if !(usable(sw) && usable(sh) && usable(device.dpi_x) && usable(device.dpi_y) && usable(device.width) && usable(device.height)) {
        return None;
    }
    let mut dpi = device.dpi_x.max(device.dpi_y).clamp(MIN_DPI, MAX_DPI);
    // A huge sheet is drawn at a lower resolution rather than for ever.
    let at = |dpi: f64| (sw * dpi / 72.0) * (sh * dpi / 72.0);
    if at(dpi) > MAX_SHEET_PIXELS {
        dpi = (dpi * (MAX_SHEET_PIXELS / at(dpi)).sqrt()).max(1.0);
    }
    let scale = dpi / 72.0;
    let sheet_landscape = sw > sh + crate::sheets::SAME;
    let sheet_portrait = sh > sw + crate::sheets::SAME;
    let paper_landscape = device.width > device.height;
    let rotate = (sheet_landscape && !paper_landscape) || (sheet_portrait && paper_landscape);
    let offset = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
    Some(Placement {
        scale,
        width: pixels(sw, scale),
        height: pixels(sh, scale),
        rotate,
        fx: device.dpi_x / dpi,
        fy: device.dpi_y / dpi,
        offset_x: offset(device.offset_x),
        offset_y: offset(device.offset_y),
    })
}

impl Placement {
    /// The bands a sheet is drawn in, top to bottom, left to right.
    pub fn bands(&self) -> Vec<Rect> {
        let mut out = Vec::new();
        let mut y = 0u32;
        while y < self.height {
            let h = BAND_ROWS.min(self.height - y);
            let mut x = 0u32;
            while x < self.width {
                let w = BAND_COLUMNS.min(self.width - x);
                out.push(Rect { x, y, w, h });
                x += w;
            }
            y += h;
        }
        out
    }

    /// Where the rendered pixels `r` land on the paper. Neighbouring rectangles meet exactly:
    /// both edges are rounded from the same position.
    pub fn target(&self, r: Rect) -> Target {
        // Turned a quarter clockwise, the sheet's pixel (u, v) lands at (height - v, u).
        let (x0, y0, x1, y1) = if self.rotate {
            let (h, x, y) = (f64::from(self.height), f64::from(r.x), f64::from(r.y));
            (h - y - f64::from(r.h), x, h - y, x + f64::from(r.w))
        } else {
            (f64::from(r.x), f64::from(r.y), f64::from(r.x) + f64::from(r.w), f64::from(r.y) + f64::from(r.h))
        };
        let px = |v: f64, f: f64, off: f64| (v * f - off).round() as i64;
        let (left, top) = (px(x0, self.fx, self.offset_x), px(y0, self.fy, self.offset_y));
        let (right, bottom) = (px(x1, self.fx, self.offset_x), px(y1, self.fy, self.offset_y));
        Target { x: left, y: top, w: (right - left).max(1), h: (bottom - top).max(1) }
    }
}

/// A premultiplied RGBA pixel over white paper, as (r, g, b).
fn on_white(p: &[u8]) -> (u8, u8, u8) {
    let (r, g, b, a) = (p[0], p[1], p[2], p[3]);
    let paper = 255 - a;
    (r.saturating_add(paper), g.saturating_add(paper), b.saturating_add(paper))
}

/// The part of a `w` × `h` premultiplied RGBA band that is not white paper, relative to the band;
/// `None` when it is all white (or the buffer is short).
pub fn ink(rgba: &[u8], w: u32, h: u32) -> Option<Rect> {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || rgba.len() < w.checked_mul(h)?.checked_mul(4)? {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for (y, row) in rgba.chunks_exact(w * 4).take(h).enumerate() {
        let mut first = None;
        let mut last = 0;
        for (x, p) in row.as_chunks::<4>().0.iter().enumerate() {
            if on_white(p) != (255, 255, 255) {
                first.get_or_insert(x);
                last = x;
            }
        }
        if let Some(first) = first {
            x0 = x0.min(first);
            x1 = x1.max(last);
            y0 = y0.min(y);
            y1 = y;
        }
    }
    (x0 != usize::MAX).then(|| Rect { x: x0 as u32, y: y0 as u32, w: (x1 - x0 + 1) as u32, h: (y1 - y0 + 1) as u32 })
}

/// A device-independent bitmap's pixels for `part` of a `w`-wide premultiplied RGBA band: rows
/// top to bottom, blue-green-red, each row padded to four bytes, over white. Turned a quarter
/// clockwise when `rotate`. Returns the bitmap's width, height and bytes; `None` when `part` is
/// outside the band.
pub fn dib(rgba: &[u8], w: u32, part: Rect, rotate: bool) -> Option<(u32, u32, Vec<u8>)> {
    let bw = w as usize;
    let (px, py, pw, ph) = (part.x as usize, part.y as usize, part.w as usize, part.h as usize);
    if pw == 0 || ph == 0 || px.checked_add(pw)? > bw {
        return None;
    }
    let rows = rgba.len() / bw.checked_mul(4)?;
    if py.checked_add(ph)? > rows {
        return None;
    }
    let pixel = |x: usize, y: usize| -> (u8, u8, u8) {
        let i = ((py + y) * bw + px + x) * 4;
        rgba.get(i..i + 4).map_or((255, 255, 255), on_white)
    };
    let (ow, oh) = if rotate { (ph, pw) } else { (pw, ph) };
    let stride = (ow.checked_mul(3)?.checked_add(3)?) & !3;
    let mut out = vec![255u8; stride.checked_mul(oh)?];
    for oy in 0..oh {
        let row = &mut out[oy * stride..oy * stride + ow * 3];
        for (ox, dst) in row.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            // Turned clockwise: the output's (ox, oy) is the source's (oy, ph - 1 - ox).
            let (r, g, b) = if rotate { pixel(oy, ph - 1 - ox) } else { pixel(ox, oy) };
            dst.copy_from_slice(&[b, g, r]);
        }
    }
    Some((ow as u32, oh as u32, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: (f64, f64) = (595.28, 841.89);

    fn device(dpi: f64, landscape: bool) -> Device {
        let (w, h) = (8.27 * dpi, 11.69 * dpi);
        let (width, height) = if landscape { (h, w) } else { (w, h) };
        Device { dpi_x: dpi, dpi_y: dpi, width, height, offset_x: 0.17 * dpi, offset_y: 0.17 * dpi }
    }

    #[test]
    fn the_printers_resolution_is_used_within_300_to_600() {
        assert_eq!(place(A4, device(600.0, false)).map(|p| p.width), Some(4961));
        assert_eq!(place(A4, device(1200.0, false)).map(|p| p.scale * 72.0), Some(600.0));
        assert_eq!(place(A4, device(150.0, false)).map(|p| p.scale * 72.0), Some(300.0));
        assert_eq!(place(A4, Device { dpi_x: 0.0, ..device(600.0, false) }), None);
        assert_eq!(place((0.0, 10.0), device(600.0, false)), None);
    }

    #[test]
    fn a_huge_sheet_is_drawn_at_a_lower_resolution() {
        let p = place((10_000.0, 10_000.0), device(600.0, false)).unwrap();
        assert!(f64::from(p.width) * f64::from(p.height) <= MAX_SHEET_PIXELS * 1.01);
    }

    #[test]
    fn bands_cover_the_sheet_once() {
        let p = place(A4, device(600.0, false)).unwrap();
        let bands = p.bands();
        let area: u64 = bands.iter().map(|b| u64::from(b.w) * u64::from(b.h)).sum();
        assert_eq!(area, u64::from(p.width) * u64::from(p.height));
        assert!(bands.iter().all(|b| b.w <= BAND_COLUMNS && b.h <= BAND_ROWS));
        assert_eq!(bands.len(), 14, "7 rows of 2 columns at 600 dpi");
    }

    #[test]
    fn bands_land_side_by_side_on_the_paper() {
        let p = place(A4, Device { dpi_x: 600.0, dpi_y: 300.0, ..device(600.0, false) }).unwrap();
        let bands = p.bands();
        let (a, b) = (p.target(bands[0]), p.target(bands[1]));
        assert_eq!(a.x + a.w, b.x, "columns meet");
        let below = p.target(bands[2]);
        assert_eq!(a.y + a.h, below.y, "rows meet");
        assert_eq!(a.h, 512, "300 dpi down the paper from 600 dpi rendered");
        assert_eq!(a.x, -102, "the printable area's offset is taken off");
    }

    #[test]
    fn a_landscape_sheet_on_portrait_paper_is_turned() {
        let p = place((A4.1, A4.0), device(600.0, false)).unwrap();
        assert!(p.rotate);
        let t = p.target(Rect { x: 0, y: 0, w: 10, h: 4 });
        // The sheet's top-left corner goes to the paper's top-right.
        assert_eq!((t.x, t.y, t.w, t.h), (i64::from(p.height) - 4 - 102, -102, 4, 10));
        assert!(!place((A4.1, A4.0), device(600.0, true)).unwrap().rotate);
        assert!(!place(A4, device(600.0, false)).unwrap().rotate);
    }

    fn band(w: u32, h: u32, dots: &[(u32, u32, [u8; 4])]) -> Vec<u8> {
        let mut v = vec![0u8; (w * h * 4) as usize];
        for &(x, y, p) in dots {
            let i = ((y * w + x) * 4) as usize;
            v[i..i + 4].copy_from_slice(&p);
        }
        v
    }

    #[test]
    fn blank_paper_is_not_sent() {
        let white = [255, 255, 255, 255];
        assert_eq!(ink(&band(8, 4, &[(1, 1, white)]), 8, 4), None, "transparent and white are paper");
        assert_eq!(ink(&band(8, 4, &[(2, 1, [0, 0, 0, 255]), (5, 3, [0, 0, 0, 128])]), 8, 4), Some(Rect { x: 2, y: 1, w: 4, h: 3 }));
        assert_eq!(ink(&[0; 4], 8, 4), None, "a short buffer");
    }

    #[test]
    fn the_bitmap_is_bgr_over_white_with_padded_rows() {
        let rgba = band(3, 2, &[(0, 0, [200, 0, 0, 255]), (2, 1, [0, 0, 100, 128])]);
        let (w, h, bytes) = dib(&rgba, 3, Rect { x: 0, y: 0, w: 3, h: 2 }, false).unwrap();
        assert_eq!((w, h, bytes.len()), (3, 2, 24), "9 bytes a row padded to 12");
        assert_eq!(&bytes[0..3], &[0, 0, 200]);
        assert_eq!(&bytes[3..6], &[255, 255, 255]);
        assert_eq!(&bytes[12 + 6..12 + 9], &[227, 127, 127], "half-covered blue over white");
        assert_eq!(dib(&rgba, 3, Rect { x: 2, y: 0, w: 2, h: 1 }, false), None, "outside the band");
    }

    #[test]
    fn a_turned_bitmap_goes_clockwise() {
        // A 3 × 2 band whose top-left pixel is black: turned clockwise it is 2 × 3, black top right.
        let rgba = band(3, 2, &[(0, 0, [0, 0, 0, 255])]);
        let (w, h, bytes) = dib(&rgba, 3, Rect { x: 0, y: 0, w: 3, h: 2 }, true).unwrap();
        assert_eq!((w, h), (2, 3));
        let stride = 8;
        assert_eq!(&bytes[3..6], &[0, 0, 0], "top right");
        assert_eq!(&bytes[0..3], &[255, 255, 255]);
        assert_eq!(&bytes[stride..stride + 6], &[255; 6]);
    }
}
