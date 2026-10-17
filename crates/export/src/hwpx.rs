//! Hangul word processor document (.hwpx, OWPML / KS X 6101): a zip package with a header of
//! shared character and paragraph shapes, one section of flowing paragraphs, tables and pictures,
//! and the pictures themselves under `BinData/`.
//!
//! Written from the package structure the standard describes: `mimetype` first and stored,
//! `version.xml`, `META-INF/` (container, manifest, rdf), `Contents/content.hpf` (the package
//! manifest and spine), `Contents/header.xml`, `Contents/section0.xml` and `settings.xml`.
//! Every page flows (HWPX has no equivalent here of Word's positioned-page fallback), and the
//! section takes the first page's size; later pages start on a new page.

use crate::{Block, Image, Item, Page, Table, Zip, allowed, items, room_for};

/// HWPUNIT per point (1 HWPUNIT = 1/7200 inch).
const UNIT: f64 = 100.0;

/// The namespaces every OWPML part declares.
const NS: &str = "xmlns:ha=\"http://www.hancom.co.kr/hwpml/2011/app\" xmlns:hp=\"http://www.hancom.co.kr/hwpml/2011/paragraph\" \
xmlns:hp10=\"http://www.hancom.co.kr/hwpml/2016/paragraph\" xmlns:hs=\"http://www.hancom.co.kr/hwpml/2011/section\" \
xmlns:hc=\"http://www.hancom.co.kr/hwpml/2011/core\" xmlns:hh=\"http://www.hancom.co.kr/hwpml/2011/head\" \
xmlns:hhs=\"http://www.hancom.co.kr/hwpml/2011/history\" xmlns:hm=\"http://www.hancom.co.kr/hwpml/2011/master-page\" \
xmlns:hpf=\"http://www.hancom.co.kr/schema/2011/hpf\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:opf=\"http://www.idpf.org/2007/opf/\" xmlns:ooxmlchart=\"http://www.hancom.co.kr/hwpml/2016/ooxmlchart\" \
xmlns:hwpunitchar=\"http://www.hancom.co.kr/hwpml/2016/HwpUnitChar\" xmlns:epub=\"http://www.idpf.org/2007/ops\" \
xmlns:config=\"urn:oasis:names:tc:opendocument:xmlns:config:1.0\"";

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>";

/// The seven script groups a font face list and a character shape name.
const LANGS: [&str; 7] = ["HANGUL", "LATIN", "HANJA", "JAPANESE", "OTHER", "SYMBOL", "USER"];

/// Points as HWPUNIT, clamped to `lo..=hi` (NaN and infinities read as `lo`).
fn unit(pt: f64, lo: i64, hi: i64) -> i64 {
    let v = pt * UNIT;
    // Clamped to lo..=hi before the cast, so it cannot truncate or wrap.
    if v.is_finite() { v.round().clamp(lo as f64, hi as f64) as i64 } else { lo }
}

/// Character shape: size in HWPUNIT, bold, italic, `#RRGGBB`.
type CharKey = (i64, bool, bool, String);

/// The character shapes used, in first-use order; a shape's id is its index.
#[derive(Default)]
struct CharShapes(Vec<CharKey>);

impl CharShapes {
    fn id(&mut self, size: f64, bold: bool, italic: bool, color: [f64; 3]) -> usize {
        let [r, g, b] = crate::rgb8(color);
        // 1 pt to 4096 pt, the range of font sizes a word processor offers.
        let key = (unit(size, 100, 409_600), bold, italic, format!("#{r:02X}{g:02X}{b:02X}"));
        match self.0.iter().position(|k| *k == key) {
            Some(i) => i,
            None => {
                self.0.push(key);
                self.0.len() - 1
            }
        }
    }

    fn xml(&self) -> String {
        let all = |v: &str| LANGS.iter().map(|l| format!(" {}=\"{v}\"", l.to_ascii_lowercase())).collect::<String>();
        let mut s = format!("<hh:charProperties itemCnt=\"{}\">", self.0.len());
        for (i, (height, bold, italic, color)) in self.0.iter().enumerate() {
            s.push_str(&format!(
                "<hh:charPr id=\"{i}\" height=\"{height}\" textColor=\"{color}\" shadeColor=\"none\" useFontSpace=\"0\" useKerning=\"0\" symMark=\"NONE\" borderFillIDRef=\"2\">\
<hh:fontRef{}/><hh:ratio{}/><hh:spacing{}/><hh:relSz{}/><hh:offset{}/>",
                all("0"),
                all("100"),
                all("0"),
                all("100"),
                all("0")
            ));
            if *bold {
                s.push_str("<hh:bold/>");
            }
            if *italic {
                s.push_str("<hh:italic/>");
            }
            s.push_str(
                "<hh:underline type=\"NONE\" shape=\"SOLID\" color=\"#000000\"/><hh:strikeout shape=\"NONE\" color=\"#000000\"/>\
<hh:outline type=\"NONE\"/><hh:shadow type=\"NONE\" color=\"#C0C0C0\" offsetX=\"10\" offsetY=\"10\"/></hh:charPr>",
            );
        }
        s.push_str("</hh:charProperties>");
        s
    }
}

/// A paragraph shape: body text (`id` 0) or a heading kept with the next paragraph (`id` 1).
fn para_pr(id: usize, before: i64, after: i64, keep_with_next: bool) -> String {
    let margin = format!(
        "<hh:margin><hc:intent value=\"0\" unit=\"HWPUNIT\"/><hc:left value=\"0\" unit=\"HWPUNIT\"/><hc:right value=\"0\" unit=\"HWPUNIT\"/>\
<hc:prev value=\"{before}\" unit=\"HWPUNIT\"/><hc:next value=\"{after}\" unit=\"HWPUNIT\"/></hh:margin><hh:lineSpacing type=\"PERCENT\" value=\"160\" unit=\"HWPUNIT\"/>"
    );
    format!(
        "<hh:paraPr id=\"{id}\" tabPrIDRef=\"0\" condense=\"0\" fontLineHeight=\"0\" snapToGrid=\"1\" suppressLineNumbers=\"0\" checked=\"0\" textDir=\"LTR\">\
<hh:align horizontal=\"LEFT\" vertical=\"BASELINE\"/><hh:heading type=\"NONE\" idRef=\"0\" level=\"0\"/>\
<hh:breakSetting breakLatinWord=\"KEEP_WORD\" breakNonLatinWord=\"KEEP_WORD\" widowOrphan=\"0\" keepWithNext=\"{}\" keepLines=\"0\" pageBreakBefore=\"0\" lineWrap=\"BREAK\"/>\
<hh:autoSpacing eAsianEng=\"0\" eAsianNum=\"0\"/><hp:switch><hp:case hp:required-namespace=\"http://www.hancom.co.kr/hwpml/2016/HwpUnitChar\">{margin}</hp:case>\
<hp:default>{margin}</hp:default></hp:switch><hh:border borderFillIDRef=\"2\" offsetLeft=\"0\" offsetRight=\"0\" offsetTop=\"0\" offsetBottom=\"0\" connect=\"0\" ignoreMargin=\"0\"/></hh:paraPr>",
        u8::from(keep_with_next)
    )
}

fn header(chars: &CharShapes) -> String {
    let mut s = format!(
        "{XML_DECL}<hh:head {NS} version=\"1.5\" secCnt=\"1\"><hh:beginNum page=\"1\" footnote=\"1\" endnote=\"1\" pic=\"1\" tbl=\"1\" equation=\"1\"/>\
<hh:refList><hh:fontfaces itemCnt=\"{}\">",
        LANGS.len()
    );
    for lang in LANGS {
        s.push_str(&format!(
            "<hh:fontface lang=\"{lang}\" fontCnt=\"1\"><hh:font id=\"0\" face=\"함초롬바탕\" type=\"TTF\" isEmbedded=\"0\">\
<hh:typeInfo familyType=\"FCAT_MYUNGJO\" weight=\"6\" proportion=\"4\" contrast=\"0\" strokeVariation=\"1\" armStyle=\"1\" letterform=\"1\" midline=\"1\" xHeight=\"1\"/></hh:font></hh:fontface>"
        ));
    }
    let border = |kind: &str| {
        ["left", "right", "top", "bottom"]
            .iter()
            .map(|e| format!("<hh:{e}Border type=\"{kind}\" width=\"0.1 mm\" color=\"#000000\"/>"))
            .collect::<String>()
    };
    // 1: no border (pages, table cells: alignment alone cannot prove the PDF drew rules);
    // 2: no border and no fill (character and paragraph shapes).
    s.push_str(&format!(
        "</hh:fontfaces><hh:borderFills itemCnt=\"2\">\
<hh:borderFill id=\"1\" threeD=\"0\" shadow=\"0\" centerLine=\"NONE\" breakCellSeparateLine=\"0\"><hh:slash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/><hh:backSlash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/>{0}<hh:diagonal type=\"SOLID\" width=\"0.1 mm\" color=\"#000000\"/></hh:borderFill>\
<hh:borderFill id=\"2\" threeD=\"0\" shadow=\"0\" centerLine=\"NONE\" breakCellSeparateLine=\"0\"><hh:slash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/><hh:backSlash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/>{0}<hh:diagonal type=\"SOLID\" width=\"0.1 mm\" color=\"#000000\"/>\
<hc:fillBrush><hc:winBrush faceColor=\"none\" hatchColor=\"#999999\" alpha=\"0\"/></hc:fillBrush></hh:borderFill></hh:borderFills>",
        border("NONE")
    ));
    s.push_str(&chars.xml());
    s.push_str("<hh:tabProperties itemCnt=\"1\"><hh:tabPr id=\"0\" autoTabLeft=\"0\" autoTabRight=\"0\"/></hh:tabProperties>");
    s.push_str(&format!("<hh:paraProperties itemCnt=\"2\">{}{}</hh:paraProperties>", para_pr(0, 0, 400, false), para_pr(1, 1200, 400, true)));
    s.push_str(
        "<hh:styles itemCnt=\"1\"><hh:style id=\"0\" type=\"PARA\" name=\"바탕글\" engName=\"Normal\" paraPrIDRef=\"0\" charPrIDRef=\"0\" nextStyleIDRef=\"0\" langID=\"1042\" lockForm=\"0\"/></hh:styles>\
</hh:refList><hh:compatibleDocument targetProgram=\"HWP201X\"><hh:layoutCompatibility/></hh:compatibleDocument>\
<hh:docOption><hh:linkinfo path=\"\" pageInherit=\"0\" footnoteInherit=\"0\"/></hh:docOption></hh:head>",
    );
    s
}

/// Page size and margins (HWPUNIT) for the section, from the first page.
fn page_geometry(pages: &[Page]) -> (i64, i64, i64, i64) {
    let (w, h) = pages.first().map_or((595.0, 842.0), |p| (p.width, p.height));
    // 1 in to 22 in either way, like the Word writer; A4 when unknown.
    let w = if w.is_finite() && w > 0.0 { unit(w, 7_200, 158_400) } else { 59_528 };
    let h = if h.is_finite() && h > 0.0 { unit(h, 7_200, 158_400) } else { 84_186 };
    (w, h, (w / 8).min(7_200), (h / 8).min(7_200))
}

fn sec_pr(w: i64, h: i64, mx: i64, my: i64) -> String {
    let note = |place: &str| {
        format!(
            "<hp:autoNumFormat type=\"DIGIT\" userChar=\"\" prefixChar=\"\" suffixChar=\")\" supscript=\"0\"/><hp:noteLine length=\"-1\" type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/>\
<hp:noteSpacing betweenNotes=\"283\" belowLine=\"567\" aboveLine=\"850\"/><hp:numbering type=\"CONTINUOUS\" newNum=\"1\"/><hp:placement place=\"{place}\" beneathText=\"0\"/>"
        )
    };
    let fill = ["BOTH", "EVEN", "ODD"]
        .iter()
        .map(|t| {
            format!(
                "<hp:pageBorderFill type=\"{t}\" borderFillIDRef=\"1\" textBorder=\"PAPER\" headerInside=\"0\" footerInside=\"0\" fillArea=\"PAPER\"><hp:offset left=\"1417\" right=\"1417\" top=\"1417\" bottom=\"1417\"/></hp:pageBorderFill>"
            )
        })
        .collect::<String>();
    // `landscape="WIDELY"` is the normal orientation: width and height are taken as given.
    format!(
        "<hp:secPr id=\"\" textDirection=\"HORIZONTAL\" spaceColumns=\"1134\" tabStop=\"8000\" tabStopVal=\"4000\" tabStopUnit=\"HWPUNIT\" outlineShapeIDRef=\"0\" memoShapeIDRef=\"0\" textVerticalWidthHead=\"0\" masterPageCnt=\"0\">\
<hp:grid lineGrid=\"0\" charGrid=\"0\" wonggojiFormat=\"0\"/><hp:startNum pageStartsOn=\"BOTH\" page=\"0\" pic=\"0\" tbl=\"0\" equation=\"0\"/>\
<hp:visibility hideFirstHeader=\"0\" hideFirstFooter=\"0\" hideFirstMasterPage=\"0\" border=\"SHOW_ALL\" fill=\"SHOW_ALL\" hideFirstPageNum=\"0\" hideFirstEmptyLine=\"0\" showLineNumber=\"0\"/>\
<hp:lineNumberShape restartType=\"0\" countBy=\"0\" distance=\"0\" startNumber=\"0\"/>\
<hp:pagePr landscape=\"WIDELY\" width=\"{w}\" height=\"{h}\" gutterType=\"LEFT_ONLY\"><hp:margin header=\"{}\" footer=\"{}\" gutter=\"0\" left=\"{mx}\" right=\"{mx}\" top=\"{my}\" bottom=\"{my}\"/></hp:pagePr>\
<hp:footNotePr>{}</hp:footNotePr><hp:endNotePr>{}</hp:endNotePr>{fill}</hp:secPr>\
<hp:ctrl><hp:colPr id=\"\" type=\"NEWSPAPER\" layout=\"LEFT\" colCount=\"1\" sameSz=\"1\" sameGap=\"0\"/></hp:ctrl>",
        my / 2,
        my / 2,
        note("EACH_COLUMN"),
        note("END_OF_DOCUMENT")
    )
}

/// Paragraph text: XML-escaped, line feeds as line breaks, other forbidden characters dropped.
fn text_xml(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars().filter(|c| allowed(*c)) {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\n' => o.push_str("<hp:lineBreak/>"),
            '\r' => {}
            // A bare tab element needs its width; a space keeps the words apart.
            '\t' => o.push(' '),
            c => o.push(c),
        }
    }
    o
}

/// Builds the section body: paragraphs numbered in order, the section properties in the first.
struct Section<'a> {
    xml: String,
    next_id: u64,
    /// The section properties, still to be put in the first paragraph.
    sec: Option<String>,
    /// The next paragraph starts a new page.
    page_break: bool,
    chars: CharShapes,
    media: Vec<(String, &'a Image)>,
    /// Text width in HWPUNIT (pictures and tables are scaled to fit it).
    text_w: i64,
    prev_text: String,
}

impl<'a> Section<'a> {
    /// One paragraph holding `runs` (already-built `hp:run` elements).
    fn para(&mut self, para_pr: usize, runs: &str) {
        let sec = self.sec.take().map_or(String::new(), |s| format!("<hp:run charPrIDRef=\"0\">{s}</hp:run>"));
        self.xml.push_str(&format!(
            "<hp:p id=\"{}\" paraPrIDRef=\"{para_pr}\" styleIDRef=\"0\" pageBreak=\"{}\" columnBreak=\"0\" merged=\"0\">{sec}{runs}</hp:p>",
            self.next_id,
            u8::from(self.page_break)
        ));
        self.next_id = self.next_id.saturating_add(1);
        self.page_break = false;
    }

    fn run(&mut self, text: &str, size: f64, bold: bool, italic: bool, color: [f64; 3]) -> String {
        let id = self.chars.id(size, bold, italic, color);
        if self.prev_text.len() < 1024 {
            self.prev_text.push_str(text);
            self.prev_text.push('\n');
        }
        format!("<hp:run charPrIDRef=\"{id}\"><hp:t>{}</hp:t></hp:run>", text_xml(text))
    }

    fn block(&mut self, b: &Block, level: u8) {
        let mut runs = String::new();
        for r in b.styled() {
            let id = self.chars.id(b.size, r.bold || level > 0, r.italic, r.color);
            runs.push_str(&format!("<hp:run charPrIDRef=\"{id}\"><hp:t>{}</hp:t></hp:run>", text_xml(&r.text)));
        }
        if self.prev_text.len() < 1024 {
            self.prev_text.push_str(&b.text);
            self.prev_text.push('\n');
        }
        self.para(usize::from(level > 0), &runs);
    }

    fn table(&mut self, t: &Table) {
        let cols = t.cols.len().max(1);
        let mut edges = t.cols.clone();
        edges.push(t.rect[2]);
        let mut widths: Vec<i64> = edges.windows(2).map(|w| unit((w[1] - w[0]).max(20.0), 2_000, 158_400)).collect();
        widths.resize(cols, 2_000);
        // Scale the grid down to the text width.
        let total: i64 = widths.iter().sum();
        if total > self.text_w && total > 0 {
            for w in &mut widths {
                *w = (*w as i128 * self.text_w as i128 / total as i128) as i64;
            }
        }
        let mut rows = String::new();
        let mut height = 0i64;
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        for (r, row) in t.rows.iter().enumerate() {
            let row_h = row.iter().map(|c| unit(c.size * 1.6, 1_000, 40_000)).max().unwrap_or(1_000).saturating_add(282);
            height = height.saturating_add(row_h);
            rows.push_str("<hp:tr>");
            let mut col = 0usize;
            let mut cells: Vec<(Option<&crate::Cell>, usize)> = row.iter().map(|c| (Some(c), c.span.max(1))).collect();
            let used: usize = cells.iter().map(|c| c.1).sum();
            cells.extend((used..cols).map(|_| (None, 1)));
            for (cell, span) in cells {
                if col >= cols {
                    break;
                }
                let span = span.min(cols - col);
                let w: i64 = widths.iter().skip(col).take(span).sum();
                let runs = match cell {
                    Some(c) => self.run(&c.text, c.size, c.bold, c.italic, c.color),
                    None => "<hp:run charPrIDRef=\"0\"><hp:t/></hp:run>".into(),
                };
                rows.push_str(&format!(
                    "<hp:tc name=\"\" header=\"0\" hasMargin=\"0\" protect=\"0\" editable=\"0\" dirty=\"1\" borderFillIDRef=\"1\">\
<hp:subList id=\"\" textDirection=\"HORIZONTAL\" lineWrap=\"BREAK\" vertAlign=\"TOP\" linkListIDRef=\"0\" linkListNextIDRef=\"0\" textWidth=\"0\" textHeight=\"0\" hasTextRef=\"0\" hasNumRef=\"0\">\
<hp:p id=\"{}\" paraPrIDRef=\"0\" styleIDRef=\"0\" pageBreak=\"0\" columnBreak=\"0\" merged=\"0\">{runs}</hp:p></hp:subList>\
<hp:cellAddr colAddr=\"{col}\" rowAddr=\"{r}\"/><hp:cellSpan colSpan=\"{span}\" rowSpan=\"1\"/><hp:cellSz width=\"{w}\" height=\"{row_h}\"/>\
<hp:cellMargin left=\"510\" right=\"510\" top=\"141\" bottom=\"141\"/></hp:tc>",
                    self.next_id
                ));
                self.next_id = self.next_id.saturating_add(1);
                col += span;
            }
            rows.push_str("</hp:tr>");
        }
        let width: i64 = widths.iter().sum();
        let tbl = format!(
            "<hp:run charPrIDRef=\"0\"><hp:tbl id=\"{id}\" zOrder=\"0\" numberingType=\"TABLE\" textWrap=\"TOP_AND_BOTTOM\" textFlow=\"BOTH_SIDES\" lock=\"0\" dropcapstyle=\"None\" pageBreak=\"CELL\" repeatHeader=\"0\" \
rowCnt=\"{}\" colCnt=\"{cols}\" cellSpacing=\"0\" borderFillIDRef=\"1\" noAdjust=\"0\">\
<hp:sz width=\"{width}\" widthRelTo=\"ABSOLUTE\" height=\"{height}\" heightRelTo=\"ABSOLUTE\" protect=\"0\"/>\
<hp:pos treatAsChar=\"1\" affectLSpacing=\"0\" flowWithText=\"1\" allowOverlap=\"0\" holdAnchorAndSO=\"0\" vertRelTo=\"PARA\" horzRelTo=\"COLUMN\" vertAlign=\"TOP\" horzAlign=\"LEFT\" vertOffset=\"0\" horzOffset=\"0\"/>\
<hp:outMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/><hp:inMargin left=\"510\" right=\"510\" top=\"141\" bottom=\"141\"/>{rows}</hp:tbl></hp:run>",
            t.rows.len()
        );
        self.para(0, &tbl);
    }

    fn picture(&mut self, im: &'a Image) {
        if !room_for(&self.media, im) {
            return;
        }
        let n = self.media.len().saturating_add(1);
        let bin = format!("BIN{n:04}");
        // Size on the page, at most the text width.
        let (w, h) = (unit(im.rect[2] - im.rect[0], 100, 158_400), unit(im.rect[3] - im.rect[1], 100, 158_400));
        let (w, h) = if w > self.text_w { (self.text_w, (h as i128 * self.text_w as i128 / w as i128).max(100) as i64) } else { (w, h) };
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let pic = format!(
            "<hp:run charPrIDRef=\"0\"><hp:pic id=\"{id}\" zOrder=\"{n}\" numberingType=\"PICTURE\" textWrap=\"TOP_AND_BOTTOM\" textFlow=\"BOTH_SIDES\" lock=\"0\" dropcapstyle=\"None\" href=\"\" groupLevel=\"0\" instid=\"{id}\" reverse=\"0\">\
<hp:offset x=\"0\" y=\"0\"/><hp:orgSz width=\"{w}\" height=\"{h}\"/><hp:curSz width=\"{w}\" height=\"{h}\"/><hp:flip horizontal=\"0\" vertical=\"0\"/>\
<hp:rotationInfo angle=\"0\" centerX=\"{}\" centerY=\"{}\" rotateimage=\"1\"/>\
<hp:renderingInfo><hc:transMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/><hc:scaMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/><hc:rotMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/></hp:renderingInfo>\
<hp:imgRect><hc:pt0 x=\"0\" y=\"0\"/><hc:pt1 x=\"{w}\" y=\"0\"/><hc:pt2 x=\"{w}\" y=\"{h}\"/><hc:pt3 x=\"0\" y=\"{h}\"/></hp:imgRect>\
<hp:imgClip left=\"0\" right=\"{w}\" top=\"0\" bottom=\"{h}\"/><hp:inMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/><hp:imgDim dimwidth=\"{w}\" dimheight=\"{h}\"/>\
<hc:img binaryItemIDRef=\"{bin}\" bright=\"0\" contrast=\"0\" effect=\"REAL_PIC\" alpha=\"0\"/><hp:effects/>\
<hp:sz width=\"{w}\" widthRelTo=\"ABSOLUTE\" height=\"{h}\" heightRelTo=\"ABSOLUTE\" protect=\"0\"/>\
<hp:pos treatAsChar=\"1\" affectLSpacing=\"0\" flowWithText=\"1\" allowOverlap=\"0\" holdAnchorAndSO=\"0\" vertRelTo=\"PARA\" horzRelTo=\"COLUMN\" vertAlign=\"TOP\" horzAlign=\"LEFT\" vertOffset=\"0\" horzOffset=\"0\"/>\
<hp:outMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/><hp:shapeComment/></hp:pic></hp:run>",
            w / 2,
            h / 2
        );
        self.para(0, &pic);
        self.media.push((format!("{bin}.{}", im.ext), im));
    }
}

/// A Hangul document (.hwpx, OWPML). Pages flow as paragraphs, tables and inline pictures, and
/// each source page after the first starts on a new page.
pub fn hwpx(pages: &[Page], title: &str) -> Vec<u8> {
    let (w, h, mx, my) = page_geometry(pages);
    let mut sec = Section {
        xml: String::new(),
        next_id: 0,
        sec: Some(sec_pr(w, h, mx, my)),
        page_break: false,
        chars: CharShapes::default(),
        media: Vec::new(),
        text_w: (w - 2 * mx).max(1_000),
        prev_text: String::new(),
    };
    // Shape 0 is the default (body size, plain, black); the style and empty runs refer to it.
    let body = crate::body_size(pages);
    sec.chars.id(body, false, false, [0.0; 3]);
    let items = items(pages);
    for it in &items {
        match it {
            Item::Para(b, lvl) => sec.block(b, *lvl),
            Item::Table(t) => sec.table(t),
            Item::Img(im) => sec.picture(im),
            Item::PageBreak => {
                // Two breaks in a row (an empty page) need a paragraph to carry the first.
                if sec.page_break {
                    sec.para(0, "<hp:run charPrIDRef=\"0\"><hp:t/></hp:run>");
                }
                sec.page_break = true;
            }
        }
    }
    // A section needs at least one paragraph, and the section properties a home.
    if sec.sec.is_some() || sec.page_break {
        sec.para(0, "<hp:run charPrIDRef=\"0\"><hp:t/></hp:run>");
    }
    let section = format!("{XML_DECL}<hs:sec {NS}>{}</hs:sec>", sec.xml);
    let header = header(&sec.chars);

    let mut manifest = String::from(
        "<opf:item id=\"header\" href=\"Contents/header.xml\" media-type=\"application/xml\"/>\
<opf:item id=\"section0\" href=\"Contents/section0.xml\" media-type=\"application/xml\"/>\
<opf:item id=\"settings\" href=\"settings.xml\" media-type=\"application/xml\"/>",
    );
    for (name, im) in &sec.media {
        let id = name.split('.').next().unwrap_or(name);
        let mime = if im.ext == "jpg" { "image/jpg" } else { "image/png" };
        manifest.push_str(&format!("<opf:item id=\"{id}\" href=\"BinData/{name}\" media-type=\"{mime}\" isEmbeded=\"1\"/>"));
    }
    let hpf = format!(
        "{XML_DECL}<opf:package {NS} version=\"\" unique-identifier=\"\" id=\"\"><opf:metadata><opf:title>{}</opf:title><opf:language>ko</opf:language>\
<opf:meta name=\"creator\" content=\"text\">KnuPdf</opf:meta></opf:metadata><opf:manifest>{manifest}</opf:manifest>\
<opf:spine><opf:itemref idref=\"header\" linear=\"yes\"/><opf:itemref idref=\"section0\" linear=\"yes\"/></opf:spine></opf:package>",
        crate::esc(title)
    );
    let version = format!(
        "{XML_DECL}<hv:HCFVersion xmlns:hv=\"http://www.hancom.co.kr/hwpml/2011/version\" tagetApplication=\"WORDPROCESSOR\" major=\"5\" minor=\"1\" micro=\"1\" buildNumber=\"0\" os=\"1\" xmlVersion=\"1.5\" application=\"KnuPdf\" appVersion=\"{}\"/>",
        env!("CARGO_PKG_VERSION")
    );
    let container = format!(
        "{XML_DECL}<ocf:container xmlns:ocf=\"urn:oasis:names:tc:opendocument:xmlns:container\" xmlns:hpf=\"http://www.hancom.co.kr/schema/2011/hpf\"><ocf:rootfiles>\
<ocf:rootfile full-path=\"Contents/content.hpf\" media-type=\"application/hwpml-package+xml\"/><ocf:rootfile full-path=\"Preview/PrvText.txt\" media-type=\"text/plain\"/>\
<ocf:rootfile full-path=\"META-INF/container.rdf\" media-type=\"application/rdf+xml\"/></ocf:rootfiles></ocf:container>"
    );
    let manifest_xml = format!("{XML_DECL}<odf:manifest xmlns:odf=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\"/>");
    let part = |about: &str, kind: &str| {
        format!(
            "<rdf:Description rdf:about=\"\"><pkg:hasPart rdf:resource=\"{about}\"/></rdf:Description><rdf:Description rdf:about=\"{about}\"><rdf:type rdf:resource=\"http://www.hancom.co.kr/hwpml/2016/meta/pkg#{kind}\"/></rdf:Description>"
        )
    };
    let rdf = format!(
        "{XML_DECL}<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" xmlns:pkg=\"http://www.hancom.co.kr/hwpml/2016/meta/pkg#\">{}{}\
<rdf:Description rdf:about=\"\"><rdf:type rdf:resource=\"http://www.hancom.co.kr/hwpml/2016/meta/pkg#Document\"/></rdf:Description></rdf:RDF>",
        part("Contents/header.xml", "HeaderFile"),
        part("Contents/section0.xml", "SectionFile")
    );
    let settings = format!(
        "{XML_DECL}<ha:HWPApplicationSetting xmlns:ha=\"http://www.hancom.co.kr/hwpml/2011/app\" xmlns:config=\"urn:oasis:names:tc:opendocument:xmlns:config:1.0\"><ha:CaretPosition listIDRef=\"0\" paraIDRef=\"0\" pos=\"0\"/></ha:HWPApplicationSetting>"
    );
    let preview: String = sec.prev_text.chars().filter(|c| allowed(*c)).take(1024).collect();

    let mut z = Zip::default();
    // The package type comes first and uncompressed, so it can be read at a fixed offset.
    z.add("mimetype", b"application/hwp+zip", false);
    z.add("version.xml", version.as_bytes(), true);
    z.add("Contents/header.xml", header.as_bytes(), true);
    z.add("Contents/section0.xml", section.as_bytes(), true);
    z.add("Contents/content.hpf", hpf.as_bytes(), true);
    z.add("settings.xml", settings.as_bytes(), true);
    z.add("Preview/PrvText.txt", preview.as_bytes(), true);
    z.add("META-INF/container.xml", container.as_bytes(), true);
    z.add("META-INF/manifest.xml", manifest_xml.as_bytes(), true);
    z.add("META-INF/container.rdf", rdf.as_bytes(), true);
    for (name, im) in &sec.media {
        z.add(&format!("BinData/{name}"), &im.bytes, false);
    }
    z.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::part;
    use crate::{Block, Cell};

    fn block(text: &str, y: f64, size: f64, bold: bool) -> Block {
        Block { text: text.into(), rect: [72.0, y, 500.0, y + size], size, bold, italic: false, color: [0.0; 3], runs: Vec::new(), rotation: 0.0 }
    }

    fn page(blocks: Vec<Block>) -> Page {
        Page { width: 595.0, height: 842.0, blocks, images: Vec::new() }
    }

    #[test]
    fn package_starts_with_a_stored_mimetype() {
        let z = hwpx(&[page(vec![block("안녕하세요", 700.0, 11.0, false)])], "제목");
        assert_eq!(&z[..4], b"PK\x03\x04");
        assert_eq!(&z[8..10], &[0, 0], "stored, not deflated");
        assert_eq!(&z[30..38], b"mimetype");
        assert_eq!(&z[38..57], b"application/hwp+zip");
        let hpf = part(&z, "Contents/content.hpf");
        assert!(hpf.contains("<opf:title>제목</opf:title>"));
        assert!(part(&z, "META-INF/container.xml").contains("Contents/content.hpf"));
        assert!(part(&z, "Preview/PrvText.txt").contains("안녕하세요"));
    }

    #[test]
    fn text_shapes_and_page_breaks() {
        let p1 = page(vec![
            block("큰 제목", 760.0, 24.0, true),
            block("본문 <내용> & 기호가 들어간 문단입니다. 본문 크기를 정하는 긴 문장입니다.", 700.0, 11.0, false),
        ]);
        let p2 = page(vec![block("둘째 쪽\n다음 줄\u{0}", 700.0, 11.0, false)]);
        let z = hwpx(&[p1, p2], "t");
        let sec = part(&z, "Contents/section0.xml");
        assert!(sec.contains("<hp:secPr "), "section properties present");
        assert_eq!(sec.matches("<hp:secPr ").count(), 1);
        assert!(sec.contains("width=\"59500\" height=\"84200\""), "page size from the first page: {sec}");
        assert!(sec.contains("본문 &lt;내용&gt; &amp; 기호"));
        assert!(sec.contains("둘째 쪽<hp:lineBreak/>다음 줄</hp:t>"));
        assert_eq!(sec.matches("pageBreak=\"1\"").count(), 1);
        let head = part(&z, "Contents/header.xml");
        // Body (shape 0) and the bold 24 pt heading.
        assert!(head.contains("<hh:charProperties itemCnt=\"2\">"), "{head}");
        assert!(head.contains("height=\"2400\""));
        assert!(head.contains("<hh:bold/>"));
        assert!(head.contains("face=\"함초롬바탕\""));
        // Every referenced shape exists.
        let n = head.matches("<hh:charPr ").count();
        for r in sec.split("charPrIDRef=\"").skip(1) {
            let id: usize = r.split('"').next().unwrap().parse().unwrap();
            assert!(id < n, "charPrIDRef {id} of {n}");
        }
    }

    #[test]
    fn tables_and_pictures() {
        let cell = |t: &str, span: usize| Cell { text: t.into(), size: 10.0, bold: false, italic: false, color: [0.8, 0.0, 0.0], span };
        let table = Table {
            rect: [72.0, 500.0, 400.0, 560.0],
            cols: vec![72.0, 200.0, 300.0],
            rows: vec![vec![cell("품명", 2), cell("수량", 1)], vec![cell("파일철", 1)]],
        };
        let im = Image { ext: "jpg", bytes: vec![0xFF, 0xD8, 0xFF], rect: [0.0, 0.0, 1000.0, 100.0], size: [1000.0, 100.0], rotation: 0.0 };
        let mut s = Section {
            xml: String::new(),
            next_id: 0,
            sec: None,
            page_break: false,
            chars: CharShapes::default(),
            media: Vec::new(),
            text_w: 45_000,
            prev_text: String::new(),
        };
        s.table(&table);
        s.picture(&im);
        assert!(s.xml.contains("rowCnt=\"2\" colCnt=\"3\""));
        assert!(s.xml.contains("<hp:cellSpan colSpan=\"2\" rowSpan=\"1\"/>"));
        // The short second row is padded to the grid.
        assert_eq!(s.xml.matches("<hp:tc ").count(), 5);
        assert!(s.xml.contains("<hp:cellAddr colAddr=\"2\" rowAddr=\"1\"/>"));
        // The wide picture is scaled to the text width.
        assert!(s.xml.contains("<hp:curSz width=\"45000\" height=\"4500\"/>"), "{}", s.xml);
        assert!(s.xml.contains("binaryItemIDRef=\"BIN0001\""));
        assert_eq!(s.media[0].0, "BIN0001.jpg");
        assert!(s.chars.xml().contains("textColor=\"#CC0000\""));
    }

    #[test]
    fn hostile_input_stays_bounded() {
        let mut b = block("x", 0.0, f64::NAN, false);
        b.rect = [f64::NEG_INFINITY, f64::NAN, f64::INFINITY, 1e300];
        let p = Page {
            width: f64::NAN,
            height: -5.0,
            blocks: vec![b],
            images: vec![Image { ext: "png", bytes: Vec::new(), rect: [0.0; 4], size: [0.0; 2], rotation: 0.0 }],
        };
        let z = hwpx(&[p, Page::default(), Page::default()], "");
        let sec = part(&z, "Contents/section0.xml");
        assert!(sec.contains("width=\"59528\" height=\"84186\""), "A4 when the size is unusable");
        assert!(!sec.contains("<hp:pic "), "empty images are skipped");
        assert_eq!(sec.matches("pageBreak=\"1\"").count(), 2);
        assert!(hwpx(&[], "").starts_with(b"PK"));
        assert!(part(&hwpx(&[], ""), "Contents/section0.xml").contains("<hp:secPr "));
    }
}
