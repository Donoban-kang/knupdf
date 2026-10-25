//! Compare files: the text differences between two open documents, a report, and the
//! differences marked as comments in the newer one.

use std::sync::Arc;

pub use pdfcraft_compare::{Change, Comparison, Kind, Side};

use crate::{DocId, Edit, EditError, Markup, NewAnnotation, NoteIcon, Session, Shape};

/// Acrobat's compare colours: replaced blue, inserted green, deleted red.
pub fn colour(kind: Kind) -> crate::Rgb {
    match kind {
        Kind::Replaced => [0.2, 0.45, 0.95],
        Kind::Inserted => [0.2, 0.75, 0.3],
        Kind::Deleted => [0.9, 0.25, 0.25],
    }
}

impl crate::Document {
    /// Standards ▸ Verify PDF/A: the rules the document breaks for `level`.
    pub fn pdfa_verify(&self, level: pdfcraft_preflight::Level) -> Vec<pdfcraft_preflight::Issue> {
        self.editor.as_ref().map(|e| pdfcraft_preflight::verify(&e.cos, level)).unwrap_or_default()
    }

    /// The standards the document declares (Standards panel).
    pub fn standards(&self) -> pdfcraft_preflight::Declared {
        self.editor.as_ref().map(|e| pdfcraft_preflight::declared(&e.cos)).unwrap_or_default()
    }

    /// Every word of the document in reading order, with page and box.
    pub fn words(&self) -> Vec<pdfcraft_compare::Word> {
        let config = pdfcraft_render::RenderConfig { password: self.password.as_deref().map(Arc::from), ..Default::default() };
        let mut r = pdfcraft_render::PageRenderer::new(self.bytes.clone(), config);
        let mut out = Vec::new();
        for (page, info) in self.info.pages.iter().enumerate() {
            let res = r.render(pdfcraft_render::RenderRequest { page, kind: pdfcraft_render::RequestKind::Text, scale: 1.0, ..Default::default() });
            if let Some(t) = res.text {
                out.extend(crate::js::page_words(&t, info).into_iter().map(|(text, rect)| pdfcraft_compare::Word { text, page, rect }));
            }
        }
        out
    }
}

impl Session {
    /// Compare files: the text differences from document `old` to document `new`.
    pub fn compare(&self, old: DocId, new: DocId) -> Result<Comparison, EditError> {
        let a = self.get(old).ok_or(EditError::NoDocument)?;
        let b = self.get(new).ok_or(EditError::NoDocument)?;
        Ok(pdfcraft_compare::compare(&a.words(), &b.words()))
    }

    /// Visual compare: regions where page n of `new` looks different from page n of `old`
    /// (rendered at `dpi`), as (page, user-space box in `new`).
    pub fn compare_visual(&self, old: DocId, new: DocId, dpi: f32) -> Result<Vec<(usize, [f64; 4])>, EditError> {
        let a = self.get(old).ok_or(EditError::NoDocument)?;
        let b = self.get(new).ok_or(EditError::NoDocument)?;
        let renderer = |d: &crate::Document| {
            pdfcraft_render::PageRenderer::new(
                d.bytes.clone(),
                pdfcraft_render::RenderConfig { password: d.password.as_deref().map(Arc::from), ..Default::default() },
            )
        };
        let (mut ra, mut rb) = (renderer(a), renderer(b));
        let scale = dpi.clamp(18.0, 150.0) / 72.0;
        let mut out = Vec::new();
        for (page, info) in b.info.pages.iter().enumerate().take(a.info.pages.len()) {
            let req = pdfcraft_render::RenderRequest { page, scale, ..Default::default() };
            let (x, y) = (ra.render(req), rb.render(req));
            if x.error.is_some() || y.error.is_some() {
                continue;
            }
            let s = y.width as f32 / info.width.max(1e-3);
            for r in pdfcraft_compare::visual_regions((&y.rgba, y.width, y.height), (&x.rgba, x.width, x.height), 24) {
                let p = info.view_to_user(r[0] as f32 / s, r[1] as f32 / s);
                let q = info.view_to_user(r[2] as f32 / s, r[3] as f32 / s);
                out.push((page, [p[0].min(q[0]) as f64, p[1].min(q[1]) as f64, p[0].max(q[0]) as f64, p[1].max(q[1]) as f64]));
            }
        }
        Ok(out)
    }

    /// The compare report as a new PDF (not opened).
    pub fn compare_report(&self, old: DocId, new: DocId) -> Result<Arc<Vec<u8>>, EditError> {
        let c = self.compare(old, new)?;
        let name = |id| self.get(id).map(|d| d.name.clone()).unwrap_or_default();
        self.create_report("Compare Report", &pdfcraft_compare::report(&c, &name(old), &name(new)))
    }

    /// Mark the differences in `new` as comments: highlights over replaced and inserted text
    /// (blue, green) and a note where text was deleted (red), authored "Compare". One undoable
    /// step; returns how many comments were added.
    pub fn mark_differences(&mut self, old: DocId, new: DocId) -> Result<usize, EditError> {
        let c = self.compare(old, new)?;
        let mut edits = Vec::new();
        for ch in &c.changes {
            let color = colour(ch.kind);
            let contents = match ch.kind {
                Kind::Replaced => format!("Replaced: \"{}\" with \"{}\"", ch.old.text, ch.new.text),
                Kind::Inserted => format!("Inserted: \"{}\"", ch.new.text),
                Kind::Deleted => format!("Deleted: \"{}\"", ch.old.text),
            };
            let shape = if ch.new.rects.is_empty() {
                let Some(r) = ch.new.near else { continue };
                Shape::Note { at: [r[0], r[3]], icon: NoteIcon::Note }
            } else {
                Shape::TextMarkup {
                    kind: Markup::Highlight,
                    quads: ch.new.rects.iter().map(|r| [r[0], r[3], r[2], r[3], r[0], r[1], r[2], r[1]]).collect(),
                }
            };
            let mut style = crate::Style::default_for(&shape);
            style.color = color;
            edits.push(Edit::AddAnnotation(NewAnnotation { page: ch.new.page, shape, style, contents, author: "Compare".into() }));
        }
        let n = edits.len();
        if n > 0 {
            self.apply(new, Edit::Batch { label: "Mark differences".into(), edits })?;
        }
        Ok(n)
    }
}

/// Export a PDF ▸ Word, PowerPoint, Hangul (HWPX), HTML or RTF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeFormat {
    Docx,
    Hwpx,
    /// KnuPdf: Hangul (.hwpx) that keeps each page as it is drawn: every line where it stands,
    /// in its own font, with the page's rules, boxes and pictures.
    HwpxLayout,
    Pptx,
    Html,
    Rtf,
}

impl OfficeFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OfficeFormat::Docx => "docx",
            OfficeFormat::Hwpx | OfficeFormat::HwpxLayout => "hwpx",
            OfficeFormat::Pptx => "pptx",
            OfficeFormat::Html => "html",
            OfficeFormat::Rtf => "rtf",
        }
    }

    pub fn from_extension(ext: &str) -> Option<OfficeFormat> {
        match ext.to_ascii_lowercase().as_str() {
            "docx" => Some(OfficeFormat::Docx),
            "hwpx" => Some(OfficeFormat::Hwpx),
            "pptx" => Some(OfficeFormat::Pptx),
            "html" | "htm" => Some(OfficeFormat::Html),
            "rtf" => Some(OfficeFormat::Rtf),
            _ => None,
        }
    }
}

/// KnuPdf: resolution of the pictures drawn for what the export cannot take out of a page.
const PICTURE_DPI: f32 = 150.0;
/// KnuPdf: most such pictures one page gets.
const MAX_DRAWN_PICTURES: usize = 64;

/// The part `rect` (visible points, y up) of a page rendered to `width` × `height` opaque RGBA
/// pixels, as a PNG placed where it stood. `None` when the part is empty, or, for a whole page
/// (`skip_blank`), when the page draws nothing but white.
fn cut(rgba: &[u8], width: u32, height: u32, page_w: f64, page_h: f64, rect: [f64; 4], skip_blank: bool) -> Option<pdfcraft_export::Image> {
    let (sx, sy) = (f64::from(width) / page_w, f64::from(height) / page_h);
    let clamp = |v: f64, max: u32| if v.is_finite() { v.clamp(0.0, f64::from(max)) as u32 } else { 0 };
    let (x0, x1) = (clamp((rect[0] * sx).floor(), width), clamp((rect[2] * sx).ceil(), width));
    let (y0, y1) = (clamp(((page_h - rect[3]) * sy).floor(), height), clamp(((page_h - rect[1]) * sy).ceil(), height));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (cw, ch) = (x1 - x0, y1 - y0);
    let row = (width as usize).checked_mul(4)?;
    let mut out = Vec::with_capacity((cw as usize).saturating_mul(ch as usize).saturating_mul(4));
    for y in y0..y1 {
        let start = (y as usize).checked_mul(row)?.checked_add((x0 as usize).checked_mul(4)?)?;
        out.extend_from_slice(rgba.get(start..start.checked_add((cw as usize).checked_mul(4)?)?)?);
    }
    // Near-white throughout: a blank page, nothing worth a picture.
    if skip_blank && out.as_chunks::<4>().0.iter().all(|p| p[0] >= 250 && p[1] >= 250 && p[2] >= 250) {
        return None;
    }
    let bytes = crate::export::encode_png(cw, ch, &out).ok()?;
    let placed = [f64::from(x0) / sx, page_h - f64::from(y1) / sy, f64::from(x1) / sx, page_h - f64::from(y0) / sy];
    Some(pdfcraft_export::Image { ext: "png", bytes, rect: placed, size: [placed[2] - placed[0], placed[3] - placed[1]], rotation: 0.0 })
}

impl crate::Document {
    /// The pages as paragraphs and images (for Word, HTML and RTF export), including what
    /// form XObjects draw.
    pub fn export_pages(&self) -> Vec<pdfcraft_export::Page> {
        let Some(cos) = self.editor.as_ref().map(|e| &e.cos) else { return Vec::new() };
        // KnuPdf: where each page has pictures that cannot be taken out as files (JBIG2, CCITT
        // fax and JPEG 2000 scans, inline images), in visible points.
        let mut unreadable: Vec<Vec<[f64; 4]>> = vec![Vec::new(); self.info.pages.len()];
        let mut pages: Vec<pdfcraft_export::Page> = self
            .info
            .pages
            .iter()
            .enumerate()
            .map(|(i, info)| {
                // Export uses the visible page in points, y up. Raw PDF coordinates include
                // CropBox offsets and precede /Rotate and /UserUnit.
                let visible_rect = |r: [f64; 4]| {
                    let a = info.user_to_view(r[0] as f32, r[1] as f32);
                    let b = info.user_to_view(r[2] as f32, r[3] as f32);
                    [a[0].min(b[0]) as f64, (info.height - a[1].max(b[1])) as f64, a[0].max(b[0]) as f64, (info.height - a[1].min(b[1])) as f64]
                };
                let crop_width = f64::from(info.crop[2] - info.crop[0]);
                let display_width = f64::from(if info.rotation % 180 == 0 { info.width } else { info.height });
                let unit = if crop_width > 0.0 && crop_width.is_finite() && display_width.is_finite() { display_width / crop_width } else { 1.0 };
                let blocks = pdfcraft_edit::reading_blocks(cos, i)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|b| !b.text.trim().is_empty())
                    .map(|b| {
                        let bold = |font: &str| {
                            let f = font.to_ascii_lowercase();
                            f.contains("bold") || f.contains("black") || f.contains("heavy")
                        };
                        let italic = |font: &str| {
                            let f = font.to_ascii_lowercase();
                            f.contains("italic") || f.contains("oblique")
                        };
                        let runs: Vec<pdfcraft_export::Run> = b
                            .runs
                            .iter()
                            .map(|r| pdfcraft_export::Run {
                                text: r.text.clone(),
                                bold: bold(&r.base_font),
                                italic: italic(&r.base_font),
                                color: r.color,
                            })
                            .collect();
                        // A paragraph is bold or italic (a heading candidate) only when all of it is.
                        let all = |f: fn(&pdfcraft_export::Run) -> bool, whole: bool| if runs.is_empty() { whole } else { runs.iter().all(f) };
                        pdfcraft_export::Block {
                            bold: all(|r| r.bold, bold(&b.base_font)),
                            italic: all(|r| r.italic, italic(&b.base_font)),
                            text: b.text,
                            rect: visible_rect(b.rect),
                            size: b.size * unit,
                            color: b.color,
                            rotation: b.rotation - f64::from(info.rotation),
                            runs,
                        }
                    })
                    .collect();
                let images = pdfcraft_edit::reading_images(cos, i)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|im| {
                        let file = im.object.and_then(|o| pdfcraft_create::image_file(cos, o).ok());
                        let Some((ext, bytes)) = file else {
                            if let Some(rects) = unreadable.get_mut(i) {
                                rects.push(visible_rect(im.rect));
                            }
                            return None;
                        };
                        Some(pdfcraft_export::Image {
                            ext: if ext == "jpg" { "jpg" } else { "png" },
                            bytes,
                            rect: visible_rect(im.rect),
                            size: [im.matrix[0].hypot(im.matrix[1]) * unit, im.matrix[2].hypot(im.matrix[3]) * unit],
                            rotation: im.matrix[1].atan2(im.matrix[0]).to_degrees() - f64::from(info.rotation),
                        })
                    })
                    .collect();
                pdfcraft_export::Page { width: info.width as f64, height: info.height as f64, blocks, images, lines: Vec::new(), shapes: Vec::new() }
            })
            .collect();
        self.add_drawn_pictures(&mut pages, &unreadable);
        pages
    }

    /// KnuPdf: what the export could not take out of a page, as the page draws it. Pictures that
    /// cannot be written as files (scanned pages in JBIG2, CCITT fax or JPEG 2000, inline images)
    /// are cut from the rendered page; a page with neither text nor pictures that still draws
    /// something (text turned into outlines, vector art) becomes one picture of the whole page.
    /// Without this a scanned page came out empty in every format.
    fn add_drawn_pictures(&self, pages: &mut [pdfcraft_export::Page], unreadable: &[Vec<[f64; 4]>]) {
        let mut renderer = None;
        for (i, page) in pages.iter_mut().enumerate() {
            let rects = unreadable.get(i).map_or(&[][..], Vec::as_slice);
            let whole = page.blocks.is_empty() && page.images.is_empty() && rects.is_empty();
            if rects.is_empty() && !whole {
                continue;
            }
            let renderer = renderer.get_or_insert_with(|| {
                let src = self.export_source();
                pdfcraft_render::PageRenderer::new(src.bytes, src.config)
            });
            let (w, h) = (page.width, page.height);
            if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
                continue;
            }
            let scale = pdfcraft_render::effective_scale(w as f32, h as f32, PICTURE_DPI / 72.0);
            let r = renderer.render(pdfcraft_render::RenderRequest { page: i, scale, tag: pdfcraft_render::PRINT_TAG, ..Default::default() });
            if r.error.is_some() || r.width == 0 || r.height == 0 {
                continue;
            }
            let mut rgba = r.rgba.to_vec();
            crate::print_raster::over_white(&mut rgba, false);
            let whole_page = [[0.0, 0.0, w, h]];
            let wanted: &[[f64; 4]] = if whole { &whole_page } else { rects };
            for rect in wanted.iter().take(MAX_DRAWN_PICTURES) {
                if let Some(image) = cut(&rgba, r.width, r.height, w, h, *rect, whole) {
                    page.images.push(image);
                }
            }
        }
    }

    /// KnuPdf: each page's text line by line and its rules and boxes, for the export that keeps
    /// pages as they are drawn ([`OfficeFormat::HwpxLayout`]). Pages that cannot be read keep
    /// what they have.
    pub(crate) fn add_layout(&self, pages: &mut [pdfcraft_export::Page]) {
        let Some(cos) = self.editor.as_ref().map(|e| &e.cos) else { return };
        let mut renderer = pdfcraft_render::PageRenderer::new(
            self.bytes.clone(),
            pdfcraft_render::RenderConfig { password: self.password.as_deref().map(Arc::from), ..Default::default() },
        );
        for (i, (page, info)) in pages.iter_mut().zip(&self.info.pages).enumerate() {
            let visible_rect = |r: [f64; 4]| {
                let a = info.user_to_view(r[0] as f32, r[1] as f32);
                let b = info.user_to_view(r[2] as f32, r[3] as f32);
                [a[0].min(b[0]) as f64, (info.height - a[1].max(b[1])) as f64, a[0].max(b[0]) as f64, (info.height - a[1].min(b[1])) as f64]
            };
            let crop_width = f64::from(info.crop[2] - info.crop[0]);
            let display_width = f64::from(if info.rotation % 180 == 0 { info.width } else { info.height });
            let unit = if crop_width > 0.0 && crop_width.is_finite() && display_width.is_finite() { display_width / crop_width } else { 1.0 };
            page.lines = pdfcraft_edit::text_lines(cos, i)
                .unwrap_or_default()
                .into_iter()
                .filter(|l| !l.text.trim().is_empty())
                .map(|l| pdfcraft_export::TextLine {
                    rect: visible_rect(l.rect),
                    size: l.size * unit,
                    // `ABCDEF+` marks a subset; the family is what follows.
                    font: l.base_font.split_once('+').map_or(l.base_font.as_str(), |(_, f)| f).to_owned(),
                    bold: l.bold,
                    italic: l.italic,
                    color: l.color,
                    text: l.text,
                })
                .collect();
            let height = f64::from(info.height);
            let up = |p: [f64; 2]| [p[0], height - p[1]];
            page.shapes = renderer
                .page_shapes(i)
                .unwrap_or_default()
                .into_iter()
                .map(|s| match s {
                    pdfcraft_render::VectorShape::Line { from, to, width, color } => {
                        pdfcraft_export::Shape::Line { from: up(from), to: up(to), width, color }
                    }
                    pdfcraft_render::VectorShape::Fill { rect, color } => {
                        pdfcraft_export::Shape::Fill { rect: [rect[0], height - rect[3], rect[2], height - rect[1]], color }
                    }
                })
                .collect();
        }
    }

    /// The document as a Word, PowerPoint, Hangul (HWPX), HTML or RTF file.
    pub fn export_office(&self, format: OfficeFormat) -> Vec<u8> {
        let mut pages = self.export_pages();
        // KnuPdf: both Hangul exports read the page's text lines and rules (ruled tables,
        // paragraphs split at list marks); the other formats keep the plain text blocks.
        if matches!(format, OfficeFormat::Hwpx | OfficeFormat::HwpxLayout) {
            self.add_layout(&mut pages);
        }
        let title = self.info.title.clone().unwrap_or_else(|| self.name.trim_end_matches(".pdf").to_string());
        match format {
            OfficeFormat::Docx => pdfcraft_export::docx(&pages, &title),
            OfficeFormat::Hwpx => pdfcraft_export::hwpx(&pages, &title),
            OfficeFormat::HwpxLayout => pdfcraft_export::hwpx_layout(&pages, &title),
            OfficeFormat::Pptx => pdfcraft_export::pptx(&pages, &title),
            OfficeFormat::Html => pdfcraft_export::html(&pages, &title).into_bytes(),
            OfficeFormat::Rtf => pdfcraft_export::rtf(&pages).into_bytes(),
        }
    }
}
