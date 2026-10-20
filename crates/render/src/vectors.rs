//! KnuPdf: the straight lines and filled rectangles a page draws — table rules, underlines,
//! boxes and bars — in displayed page points (origin top left, y down), so an export can draw
//! them again where the PDF did. Curves, patterns, images and text are left out.

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt,
    interpret_page,
};
use kurbo::{Affine, BezPath, PathEl, Point, Rect};

/// Most shapes kept per page: a hostile page cannot make the export unbounded.
const MAX_SHAPES: usize = 20_000;
/// Fills and strokes fainter than this (alpha) draw nothing worth keeping.
const MIN_ALPHA: f32 = 0.05;
/// Coordinates beyond this are not on any page.
const MAX_COORDINATE: f64 = 1e6;

/// One straight line or filled rectangle.
#[derive(Clone, Debug, PartialEq)]
pub enum VectorShape {
    /// A stroked straight segment, `width` points wide.
    Line { from: [f64; 2], to: [f64; 2], width: f64, color: [f64; 3] },
    /// A filled axis-aligned rectangle [x0, y0, x1, y1].
    Fill { rect: [f64; 4], color: [f64; 3] },
}

#[derive(Default)]
struct Shapes {
    shapes: Vec<VectorShape>,
}

/// The paint as sRGB in 0..=1, or `None` for patterns and nearly transparent colours.
fn rgb(paint: &Paint<'_>) -> Option<[f64; 3]> {
    let Paint::Color(c) = paint else { return None };
    let [r, g, b, a] = c.to_rgba().components();
    (a >= MIN_ALPHA).then(|| [r, g, b].map(|v| f64::from(v).clamp(0.0, 1.0)))
}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x.abs() <= MAX_COORDINATE && p.y.abs() <= MAX_COORDINATE
}

/// The axis-aligned rectangle a closed four-sided path traces, if it is one.
fn as_rect(path: &BezPath, t: Affine) -> Option<Rect> {
    let mut pts: Vec<Point> = Vec::with_capacity(5);
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) if pts.is_empty() => pts.push(t * p),
            PathEl::LineTo(p) => pts.push(t * p),
            PathEl::ClosePath => {}
            _ => return None,
        }
        if pts.len() > 5 {
            return None;
        }
    }
    // A closing line back to the start may or may not be written.
    if pts.len() == 5 && (pts[4] - pts[0]).hypot() < 0.01 {
        pts.pop();
    }
    if pts.len() != 4 || !pts.iter().all(|p| finite(*p)) {
        return None;
    }
    // Each side is horizontal or vertical.
    let axis = |a: Point, b: Point| (a.x - b.x).abs() < 0.01 || (a.y - b.y).abs() < 0.01;
    (0..4).all(|i| axis(pts[i], pts[(i + 1) % 4])).then(|| {
        let (xs, ys) = (pts.iter().map(|p| p.x), pts.iter().map(|p| p.y));
        Rect::new(
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        )
    })
}

impl Shapes {
    fn push(&mut self, shape: VectorShape) {
        if self.shapes.len() < MAX_SHAPES {
            self.shapes.push(shape);
        }
    }

    fn stroke(&mut self, path: &BezPath, t: Affine, width: f64, color: [f64; 3]) {
        // The stroke width as drawn: the line width scaled by the transform's average scale.
        let scale = t.determinant().abs().sqrt();
        let width = (width * scale).clamp(0.1, 100.0);
        let (mut start, mut at) = (None::<Point>, None::<Point>);
        for el in path.elements() {
            match *el {
                PathEl::MoveTo(p) => {
                    let p = t * p;
                    (start, at) = (Some(p), Some(p));
                }
                PathEl::LineTo(p) => {
                    let p = t * p;
                    if let Some(a) = at
                        && finite(a)
                        && finite(p)
                    {
                        self.push(VectorShape::Line { from: [a.x, a.y], to: [p.x, p.y], width, color });
                    }
                    at = Some(p);
                }
                PathEl::ClosePath => {
                    if let (Some(a), Some(s)) = (at, start)
                        && finite(a)
                        && finite(s)
                        && (a - s).hypot() > 0.01
                    {
                        self.push(VectorShape::Line { from: [a.x, a.y], to: [s.x, s.y], width, color });
                    }
                    at = start;
                }
                // Curves are not rules or boxes.
                PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => at = Some(t * p),
            }
        }
    }
}

impl<'a> Device<'a> for Shapes {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, path: &BezPath, transform: Affine, paint: &Paint<'a>, mode: &PathDrawMode) {
        let Some(color) = rgb(paint) else { return };
        match mode {
            PathDrawMode::Stroke(props) => self.stroke(path, transform, f64::from(props.line_width), color),
            PathDrawMode::Fill(_) => {
                if let Some(r) = as_rect(path, transform)
                    && r.width() > 0.0
                    && r.height() > 0.0
                {
                    self.push(VectorShape::Fill { rect: [r.x0, r.y0, r.x1, r.y1], color });
                }
            }
        }
    }
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph(&mut self, _: &Glyph<'a>, _: Affine, _: Affine, _: &Paint<'a>, _: &GlyphDrawMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// The lines and filled rectangles of one page. Panics inside the interpreter are the caller's
/// to catch.
pub(crate) fn page_shapes(pdf: &Pdf, page: usize, settings: &InterpreterSettings) -> Result<Vec<VectorShape>, String> {
    let pages = pdf.pages();
    let p = pages.get(page).ok_or("the page could not be read")?;
    let (w, h) = p.render_dimensions();
    let cache = InterpreterCache::new();
    let initial = p.initial_transform(true).to_kurbo();
    let mut ctx = Context::new(initial, Rect::new(0.0, 0.0, f64::from(w), f64::from(h)), &cache, p.xref(), settings.clone());
    let mut dev = Shapes::default();
    interpret_page(p, &mut ctx, &mut dev);
    Ok(dev.shapes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PageRenderer, RenderConfig};
    use std::sync::Arc;

    fn pdf(content: &str) -> Vec<u8> {
        format!(
            "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >> endobj
4 0 obj << /Length {} >> stream
{content}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF",
            content.len()
        )
        .into_bytes()
    }

    #[test]
    fn rules_and_filled_boxes_are_found_in_page_points() {
        // A 2 pt red rule at y 90 (user space, y up), a grey box, a stroked box and a curve.
        let content = "2 w 1 0 0 RG 10 90 m 90 90 l S 0.5 g 10 10 30 20 re f 0 G 1 w 50 10 40 20 re S 10 50 m 30 70 50 70 70 50 c S";
        let mut r = PageRenderer::new(Arc::new(pdf(content)), RenderConfig::default());
        let shapes = r.page_shapes(0).expect("shapes");
        // Displayed points, origin top left: y 90 up is 10 down.
        assert!(shapes.contains(&VectorShape::Line { from: [10.0, 10.0], to: [90.0, 10.0], width: 2.0, color: [1.0, 0.0, 0.0] }), "{shapes:?}");
        assert!(
            shapes
                .iter()
                .any(|s| matches!(s, VectorShape::Fill { rect, color } if *rect == [10.0, 70.0, 40.0, 90.0] && (color[0] - 0.5).abs() < 0.01)),
            "{shapes:?}"
        );
        // The stroked box is four sides; the curve adds nothing.
        let lines = shapes.iter().filter(|s| matches!(s, VectorShape::Line { color, .. } if *color == [0.0; 3])).count();
        assert_eq!(lines, 4, "{shapes:?}");
    }

    #[test]
    fn hostile_geometry_stays_bounded_and_finite() {
        let mut content = String::from("1e300 0 0 1e300 0 0 cm 0 0 m 1 1 l S Q q ");
        for i in 0..30_000 {
            content.push_str(&format!("{} 1 m {} 2 l S ", i % 90, i % 90));
        }
        let mut r = PageRenderer::new(Arc::new(pdf(&content)), RenderConfig::default());
        let shapes = r.page_shapes(0).expect("shapes");
        assert!(shapes.len() <= MAX_SHAPES);
        for s in &shapes {
            let v: Vec<f64> = match s {
                VectorShape::Line { from, to, width, .. } => vec![from[0], from[1], to[0], to[1], *width],
                VectorShape::Fill { rect, .. } => rect.to_vec(),
            };
            assert!(v.iter().all(|x| x.is_finite() && x.abs() <= MAX_COORDINATE), "{s:?}");
        }
    }
}
