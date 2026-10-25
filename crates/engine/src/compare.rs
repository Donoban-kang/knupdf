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

/// KnuPdf: reads a rendered page (RGBA, width, height) into lines of words in its pixels, or
/// `None` when nothing on this computer can.
pub(crate) type ScanReader<'a> = dyn Fn(&[u8], u32, u32) -> Option<Vec<pdfcraft_ocr::Line>> + 'a;

/// KnuPdf: the language scanned pages are read in.
const SCAN_LANGUAGE: &str = "ko";
/// KnuPdf: resolution scanned pages are read at.
const OCR_DPI: f32 = 300.0;

/// KnuPdf: Windows' own text recognition, in Korean, when this computer has it.
fn system_reader(rgba: &[u8], width: u32, height: u32) -> Option<Vec<pdfcraft_ocr::Line>> {
    pdfcraft_ocr::system::recognize(rgba, width, height, SCAN_LANGUAGE).ok()
}

/// KnuPdf: what an export did with scanned pages (0-based).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanReport {
    /// Read as text.
    pub recognized: Vec<usize>,
    /// Kept as pictures: this computer cannot read Korean text.
    pub unreadable: Vec<usize>,
}

/// KnuPdf: lines read from a scan, `sx` × `sy` pixels a point on a page `page_h` points tall,
/// as text lines (y up) and as paragraphs: a paragraph breaks where the gap above a line is
/// larger than most of a line or the text size changes. Words are joined with a space only
/// where the scan shows one, so text that recognition split letter by letter reads whole.
fn scanned_text(lines: &[pdfcraft_ocr::Line], sx: f64, sy: f64, page_h: f64) -> (Vec<pdfcraft_export::TextLine>, Vec<pdfcraft_export::Block>) {
    let mut text_lines: Vec<pdfcraft_export::TextLine> = Vec::new();
    for line in lines {
        let mut words: Vec<&pdfcraft_ocr::Word> = line.words.iter().filter(|w| w.rect.iter().all(|v| v.is_finite())).collect();
        words.sort_by(|a, b| a.rect[0].total_cmp(&b.rect[0]));
        let Some(first) = words.first() else { continue };
        let mut text = String::new();
        let mut rect = first.rect;
        let mut prev: Option<&pdfcraft_ocr::Word> = None;
        for w in &words {
            if let Some(p) = prev {
                let height = (w.rect[3] - w.rect[1]).max(p.rect[3] - p.rect[1]).max(1.0);
                if w.rect[0] - p.rect[2] > 0.25 * height {
                    text.push(' ');
                }
            }
            text.push_str(w.text.trim());
            rect = [rect[0].min(w.rect[0]), rect[1].min(w.rect[1]), rect[2].max(w.rect[2]), rect[3].max(w.rect[3])];
            prev = Some(w);
        }
        if text.trim().is_empty() {
            continue;
        }
        let [l, t, r, b] = rect.map(f64::from);
        let size = (((b - t) / sy * 0.8) * 2.0).round() / 2.0;
        text_lines.push(pdfcraft_export::TextLine {
            text,
            rect: [l / sx, page_h - b / sy, r / sx, page_h - t / sy],
            size: if size.is_finite() { size.clamp(5.0, 72.0) } else { 10.0 },
            font: String::new(),
            bold: false,
            italic: false,
            color: [0.0; 3],
        });
    }
    // Top to bottom, as the page reads.
    text_lines.sort_by(|a, b| b.rect[3].total_cmp(&a.rect[3]));
    let mut blocks: Vec<pdfcraft_export::Block> = Vec::new();
    let mut last: Option<&pdfcraft_export::TextLine> = None;
    for line in &text_lines {
        let joins = last.is_some_and(|p| {
            let height = p.rect[3] - p.rect[1];
            let gap = p.rect[1] - line.rect[3];
            gap <= 0.7 * height && (line.size - p.size).abs() <= 0.2 * p.size.max(1.0)
        });
        match blocks.last_mut() {
            Some(block) if joins => {
                block.text.push(' ');
                block.text.push_str(&line.text);
                block.rect = [block.rect[0].min(line.rect[0]), line.rect[1].min(block.rect[1]), block.rect[2].max(line.rect[2]), block.rect[3]];
            }
            _ => blocks.push(pdfcraft_export::Block {
                text: line.text.clone(),
                rect: line.rect,
                size: line.size,
                bold: false,
                italic: false,
                color: [0.0; 3],
                rotation: 0.0,
                runs: Vec::new(),
            }),
        }
        last = Some(line);
    }
    (text_lines, blocks)
}

impl crate::Document {
    /// The pages as paragraphs and images (for Word, HTML and RTF export), including what
    /// form XObjects draw.
    pub fn export_pages(&self) -> Vec<pdfcraft_export::Page> {
        self.export_pages_read_with(&system_reader).0
    }

    /// [`Self::export_pages`], scanned pages read by `read` (an RGBA page image to its lines, or
    /// `None` when nothing can read it), and what became of them.
    pub(crate) fn export_pages_read_with(&self, read: &ScanReader<'_>) -> (Vec<pdfcraft_export::Page>, ScanReport) {
        let Some(cos) = self.editor.as_ref().map(|e| &e.cos) else { return (Vec::new(), ScanReport::default()) };
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
        let report = self.add_drawn_pictures(&mut pages, &unreadable, read);
        (pages, report)
    }

    /// KnuPdf: what the export could not take out of a page, as the page draws it. Pictures that
    /// cannot be written as files (scanned pages in JBIG2, CCITT fax or JPEG 2000, inline images)
    /// are cut from the rendered page; a page with neither text nor pictures that still draws
    /// something (text turned into outlines, vector art) becomes one picture of the whole page.
    /// Without this a scanned page came out empty in every format.
    fn add_drawn_pictures(&self, pages: &mut [pdfcraft_export::Page], unreadable: &[Vec<[f64; 4]>], read: &ScanReader<'_>) -> ScanReport {
        let mut report = ScanReport::default();
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
            // A scanned page (pictures that cannot be taken out cover most of it, or it draws
            // without text or pictures) and no text of its own: read it, so the export holds
            // text that can be edited rather than a picture.
            let covered: f64 = rects.iter().map(|r| ((r[2] - r[0]) * (r[3] - r[1])).max(0.0)).sum();
            if page.blocks.is_empty() && (whole || covered >= 0.5 * w * h) {
                let scale = pdfcraft_render::effective_scale(w as f32, h as f32, OCR_DPI / 72.0);
                let r = renderer.render(pdfcraft_render::RenderRequest { page: i, scale, tag: pdfcraft_render::PRINT_TAG, ..Default::default() });
                if r.error.is_none() && r.width > 0 && r.height > 0 {
                    let mut rgba = r.rgba.to_vec();
                    crate::print_raster::over_white(&mut rgba, false);
                    match read(&rgba, r.width, r.height) {
                        Some(lines) if !lines.is_empty() => {
                            let (sx, sy) = (f64::from(r.width) / w, f64::from(r.height) / h);
                            let (text_lines, blocks) = scanned_text(&lines, sx, sy, h);
                            if !blocks.is_empty() {
                                page.lines.extend(text_lines);
                                page.blocks.extend(blocks);
                                report.recognized.push(i);
                                continue;
                            }
                        }
                        Some(_) => {}
                        None => report.unreadable.push(i),
                    }
                }
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
        report
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
            // Lines read from a scan stay (a scanned page has none of its own).
            let read_from_scan = std::mem::take(&mut page.lines);
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
                .chain(read_from_scan)
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
        self.export_office_reported(format).0
    }

    /// KnuPdf: [`Self::export_office`], and which scanned pages became text and which stayed
    /// pictures because this computer cannot read Korean.
    pub fn export_office_reported(&self, format: OfficeFormat) -> (Vec<u8>, ScanReport) {
        self.export_office_read_with(format, &system_reader)
    }

    pub(crate) fn export_office_read_with(&self, format: OfficeFormat, read: &ScanReader<'_>) -> (Vec<u8>, ScanReport) {
        let (mut pages, report) = self.export_pages_read_with(read);
        // KnuPdf: both Hangul exports read the page's text lines and rules (ruled tables,
        // paragraphs split at list marks); the other formats keep the plain text blocks.
        if matches!(format, OfficeFormat::Hwpx | OfficeFormat::HwpxLayout) {
            self.add_layout(&mut pages);
        }
        let title = self.info.title.clone().unwrap_or_else(|| self.name.trim_end_matches(".pdf").to_string());
        let bytes = match format {
            OfficeFormat::Docx => pdfcraft_export::docx(&pages, &title),
            OfficeFormat::Hwpx => pdfcraft_export::hwpx(&pages, &title),
            OfficeFormat::HwpxLayout => pdfcraft_export::hwpx_layout(&pages, &title),
            OfficeFormat::Pptx => pdfcraft_export::pptx(&pages, &title),
            OfficeFormat::Html => pdfcraft_export::html(&pages, &title).into_bytes(),
            OfficeFormat::Rtf => pdfcraft_export::rtf(&pages).into_bytes(),
        };
        (bytes, report)
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    use crate::Session;

    /// A one-page scan: an inline image over the page, which no export can take out.
    fn scan_pdf() -> Vec<u8> {
        let c = "q 595 0 0 842 0 0 cm BI /W 2 /H 2 /CS /G /BPC 8 ID \x00\u{7f}\u{7f}\x00 EI Q";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R >>".to_string(),
            format!("<< /Length {} >>\nstream\n{c}\nendstream", c.len()),
        ];
        let mut pdf = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for o in offsets {
            pdf.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF", objs.len() + 1).as_bytes());
        pdf
    }

    fn word(text: &str, l: f32, t: f32, r: f32, b: f32) -> pdfcraft_ocr::Word {
        pdfcraft_ocr::Word { text: text.into(), rect: [l, t, r, b] }
    }

    /// What Windows reads off the scan, in its pixels: a title, then two lines of one
    /// paragraph, the second word of the title split letter by letter.
    fn read(_: &[u8], width: u32, height: u32) -> Option<Vec<pdfcraft_ocr::Line>> {
        assert!(width > 2000 && height > 3000, "read at 300 dpi: {width} × {height}");
        Some(vec![
            pdfcraft_ocr::Line {
                words: vec![word("공", 300.0, 300.0, 360.0, 360.0), word("문", 362.0, 300.0, 422.0, 360.0), word("제목", 460.0, 300.0, 580.0, 360.0)],
            },
            pdfcraft_ocr::Line { words: vec![word("첫째", 300.0, 500.0, 400.0, 540.0), word("줄", 420.0, 500.0, 460.0, 540.0)] },
            pdfcraft_ocr::Line { words: vec![word("둘째", 300.0, 550.0, 400.0, 590.0), word("줄", 420.0, 550.0, 460.0, 590.0)] },
        ])
    }

    #[test]
    fn scanned_pages_are_read_into_text_that_can_be_edited() {
        let mut s = Session::new();
        let id = s.open("scan.pdf", None, std::sync::Arc::new(scan_pdf()), None).unwrap();
        let doc = s.get(id).unwrap();
        let (pages, report) = doc.export_pages_read_with(&read);
        assert_eq!(report, ScanReport { recognized: vec![0], unreadable: Vec::new() });
        let page = &pages[0];
        assert!(page.images.is_empty(), "the scan is replaced by its text");
        let texts: Vec<&str> = page.blocks.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(texts, ["공문 제목", "첫째 줄 둘째 줄"], "letters join, words keep their space, close lines make one paragraph");
        // Placed where the scan shows them, in points (y up): 300 px at ~300 dpi is ~72 pt.
        let title = &page.lines[0];
        assert!((title.rect[0] - 72.0).abs() < 3.0 && (title.rect[3] - (842.0 - 72.0)).abs() < 3.0, "{:?}", title.rect);
        assert!(title.size > 8.0 && title.size < 15.0, "{}", title.size);
        // Both Hangul exports carry it as text.
        for format in [OfficeFormat::Hwpx, OfficeFormat::HwpxLayout, OfficeFormat::Docx] {
            let (bytes, _) = doc.export_office_read_with(format, &read);
            let text = unzip_text(&bytes);
            assert!(text.contains("공문 제목") && text.contains("둘째 줄"), "{format:?}");
        }
    }

    #[test]
    fn without_korean_recognition_a_scan_stays_a_picture_and_says_so() {
        let mut s = Session::new();
        let id = s.open("scan.pdf", None, std::sync::Arc::new(scan_pdf()), None).unwrap();
        let (pages, report) = s.get(id).unwrap().export_pages_read_with(&|_: &[u8], _, _| None);
        assert_eq!(report, ScanReport { recognized: Vec::new(), unreadable: vec![0] });
        assert_eq!(pages[0].images.len(), 1);
        assert!(pages[0].blocks.is_empty());
    }

    /// Every part of a zip that is stored or deflated, as text (enough to look for words).
    fn unzip_text(zip: &[u8]) -> String {
        let mut out = String::new();
        let mut at = 0;
        while let Some(i) = zip.get(at..).and_then(|z| z.windows(4).position(|w| w == b"PK\x03\x04")) {
            let h = at + i;
            let method = u16::from_le_bytes([zip[h + 8], zip[h + 9]]);
            let size = u32::from_le_bytes([zip[h + 18], zip[h + 19], zip[h + 20], zip[h + 21]]) as usize;
            let (n, e) = (u16::from_le_bytes([zip[h + 26], zip[h + 27]]) as usize, u16::from_le_bytes([zip[h + 28], zip[h + 29]]) as usize);
            let start = h + 30 + n + e;
            let data = &zip[start..start + size];
            let raw = if method == 8 {
                let mut raw = Vec::new();
                std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(data), &mut raw).unwrap();
                raw
            } else {
                data.to_vec()
            };
            out.push_str(&String::from_utf8_lossy(&raw));
            at = start + size;
        }
        out
    }
}
