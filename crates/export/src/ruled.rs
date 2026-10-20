//! KnuPdf: a page rebuilt from its text lines and rules, for the flowing Hangul export. Tables are
//! the areas the page encloses with rules — their cells, including cells merged across rows or
//! columns, come from which rules are drawn — and the text outside them becomes paragraphs line
//! by line: a line that starts with a list mark (`-`, `○`, `※`, `가.`, `1.`, …), follows a short
//! line, or changes size starts a new paragraph. Guessing tables from text alignment alone
//! swallowed body text and lost merged cells; joining a page's lines by proximity merged list
//! items.

use std::borrow::Cow;

use crate::{Block, Cell, Item, Page, Run, Shape, Table, TextLine, level};

/// Points two rule positions may differ by and still be one grid line (a double rule is one).
const SNAP: f64 = 1.6;
/// Shorter rules are marks, not table lines.
const MIN_RULE: f64 = 3.0;
/// A filled rectangle this thin is a rule.
const THIN: f64 = 2.0;
/// Most rules looked at on one page, and most grid rows or columns of one table.
const MAX_RULES: usize = 4_000;
const MAX_GRID: usize = 200;

#[derive(Clone, Copy, Debug)]
struct Rule {
    horizontal: bool,
    /// y of a horizontal rule, x of a vertical one.
    at: f64,
    /// Its extent along the other axis, low to high.
    from: f64,
    to: f64,
}

fn finite(v: &[f64]) -> bool {
    v.iter().all(|x| x.is_finite() && x.abs() < 1e6)
}

/// The page's horizontal and vertical rules: stroked straight lines and thin filled boxes.
fn rules(shapes: &[Shape]) -> Vec<Rule> {
    let mut out = Vec::new();
    for s in shapes {
        if out.len() >= MAX_RULES {
            break;
        }
        let rule = match s {
            Shape::Line { from, to, .. } if finite(from) && finite(to) => {
                if (from[1] - to[1]).abs() < 0.75 {
                    Some(Rule { horizontal: true, at: (from[1] + to[1]) / 2.0, from: from[0].min(to[0]), to: from[0].max(to[0]) })
                } else if (from[0] - to[0]).abs() < 0.75 {
                    Some(Rule { horizontal: false, at: (from[0] + to[0]) / 2.0, from: from[1].min(to[1]), to: from[1].max(to[1]) })
                } else {
                    None
                }
            }
            Shape::Fill { rect, .. } if finite(rect) => {
                let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
                if h <= THIN && w > h {
                    Some(Rule { horizontal: true, at: (rect[1] + rect[3]) / 2.0, from: rect[0], to: rect[2] })
                } else if w <= THIN && h > w {
                    Some(Rule { horizontal: false, at: (rect[0] + rect[2]) / 2.0, from: rect[1], to: rect[3] })
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(r) = rule.filter(|r| r.to - r.from >= MIN_RULE) {
            out.push(r);
        }
    }
    out
}

/// Whether a horizontal and a vertical rule meet (within the snap distance).
fn cross(h: &Rule, v: &Rule) -> bool {
    v.at >= h.from - SNAP && v.at <= h.to + SNAP && h.at >= v.from - SNAP && h.at <= v.to + SNAP
}

/// Positions within the snap distance of each other as one, sorted.
fn snap(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, usize)> = Vec::new();
    for x in v {
        match out.last_mut() {
            Some((e, n)) if x - *e <= SNAP => {
                *e = (*e * *n as f64 + x) / (*n + 1) as f64;
                *n += 1;
            }
            _ => out.push((x, 1)),
        }
    }
    out.into_iter().map(|(e, _)| e).collect()
}

/// Whether the rules draw a line at `at` along most of `from..to`.
fn drawn(rules: &[&Rule], horizontal: bool, at: f64, from: f64, to: f64) -> bool {
    let need = (to - from) * 0.6;
    let covered: f64 =
        rules.iter().filter(|r| r.horizontal == horizontal && (r.at - at).abs() <= SNAP).map(|r| (r.to.min(to) - r.from.max(from)).max(0.0)).sum();
    covered >= need
}

/// A ruled table's grid: column edges left to right, row edges top to bottom (y up).
struct Grid {
    xs: Vec<f64>,
    ys: Vec<f64>,
}

/// The ruled tables on a page: groups of rules that cross one another, at least two each way.
fn grids(rules: &[Rule]) -> Vec<(Grid, Vec<&Rule>)> {
    let n = rules.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..n {
        for j in (i + 1)..n {
            let (a, b) = (&rules[i], &rules[j]);
            let meet = match (a.horizontal, b.horizontal) {
                (true, false) => cross(a, b),
                (false, true) => cross(b, a),
                _ => false,
            };
            if meet {
                let (ra, rb) = (root(&mut parent, i), root(&mut parent, j));
                parent[ra] = rb;
            }
        }
    }
    let mut groups: Vec<(usize, Vec<&Rule>)> = Vec::new();
    for (i, rule) in rules.iter().enumerate().take(n) {
        let r = root(&mut parent, i);
        match groups.iter_mut().find(|(g, _)| *g == r) {
            Some((_, members)) => members.push(rule),
            None => groups.push((r, vec![rule])),
        }
    }
    let mut out = Vec::new();
    for (_, members) in groups {
        let hs: Vec<&&Rule> = members.iter().filter(|r| r.horizontal).collect();
        let vs: Vec<&&Rule> = members.iter().filter(|r| !r.horizontal).collect();
        if hs.len() < 2 || vs.is_empty() {
            continue;
        }
        let mut xs: Vec<f64> = vs.iter().map(|r| r.at).collect();
        // A table without side borders still spans its horizontal rules.
        xs.push(hs.iter().map(|r| r.from).fold(f64::INFINITY, f64::min));
        xs.push(hs.iter().map(|r| r.to).fold(f64::NEG_INFINITY, f64::max));
        let xs = snap(xs);
        let mut ys = snap(hs.iter().map(|r| r.at).collect());
        ys.reverse();
        if xs.len() < 2 || ys.len() < 2 || xs.len() > MAX_GRID || ys.len() > MAX_GRID || (xs.len() - 1) * (ys.len() - 1) < 2 {
            continue;
        }
        out.push((Grid { xs, ys }, members));
    }
    out
}

/// Text lines whose centre lies in `rect` (y up).
fn inside(l: &TextLine, rect: [f64; 4]) -> bool {
    let (cx, cy) = ((l.rect[0] + l.rect[2]) / 2.0, (l.rect[1] + l.rect[3]) / 2.0);
    cx >= rect[0] && cx <= rect[2] && cy >= rect[1] && cy <= rect[3]
}

/// Text as the page draws it: a soft hyphen it shows is a hyphen (Hangul's PDF writer sets list
/// dashes that way).
fn shown(text: &str) -> String {
    text.replace('\u{AD}', "-")
}

/// Lines on one baseline (top to bottom, then left to right), each row's lines left to right.
fn rows(mut lines: Vec<&TextLine>) -> Vec<Vec<&TextLine>> {
    lines.sort_by(|a, b| b.rect[3].total_cmp(&a.rect[3]).then(a.rect[0].total_cmp(&b.rect[0])));
    let mut out: Vec<Vec<&TextLine>> = Vec::new();
    for l in lines {
        let same = out.last().and_then(|r| r.first()).is_some_and(|f| {
            let overlap = f.rect[3].min(l.rect[3]) - f.rect[1].max(l.rect[1]);
            overlap > 0.5 * (f.rect[3] - f.rect[1]).min(l.rect[3] - l.rect[1]).max(0.1)
        });
        match out.last_mut() {
            Some(row) if same => row.push(l),
            _ => out.push(vec![l]),
        }
    }
    for r in &mut out {
        r.sort_by(|a, b| a.rect[0].total_cmp(&b.rect[0]));
    }
    out
}

/// A row's runs: each line in its style, with a space where the page leaves a gap.
fn row_runs(row: &[&TextLine]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut prev: Option<&TextLine> = None;
    for l in row {
        let text = shown(&l.text);
        let gap = prev.map_or(0.0, |p| l.rect[0] - p.rect[2]);
        let space = prev.is_some_and(|p| gap > 0.2 * l.size.max(1.0) && !p.text.ends_with(' ') && !text.starts_with(' '));
        if space && let Some(last) = runs.last_mut() {
            last.text.push(' ');
        }
        match runs.last_mut() {
            Some(r) if r.bold == l.bold && r.italic == l.italic && r.color == l.color => r.text.push_str(&text),
            _ => runs.push(Run { text, bold: l.bold, italic: l.italic, color: l.color }),
        }
        prev = Some(l);
    }
    runs
}

/// Whether a row starts a list item or a numbered paragraph.
fn starts_item(text: &str) -> bool {
    let t = text.trim_start();
    let mut chars = t.chars();
    let Some(first) = chars.next() else { return false };
    if matches!(
        first,
        '-' | '‐'
            | '–'
            | '―'
            | '○'
            | '●'
            | '◦'
            | '□'
            | '■'
            | '※'
            | '·'
            | '•'
            | '▪'
            | '◇'
            | '◆'
            | '▶'
            | '｢'
            | '「'
            | '['
            | '<'
            | '('
    ) {
        return true;
    }
    // `1.`, `12)`, `가.`, `나)`: a number or one Hangul syllable, then `.` or `)`.
    let head: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = if head.is_empty() && ('가'..='힣').contains(&first) { chars.next() } else { t.chars().nth(head.len()) };
    (!head.is_empty() || ('가'..='힣').contains(&first)) && matches!(rest, Some('.' | ')')) && head.len() <= 3
}

/// The block for a paragraph of rows.
fn block(rows: &[Vec<&TextLine>]) -> Option<Block> {
    let mut runs: Vec<Run> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let mut rr = row_runs(row);
        if i > 0
            && let (Some(last), Some(first)) = (runs.last_mut(), rr.first())
            && !last.text.ends_with(' ')
            && !first.text.starts_with(' ')
        {
            last.text.push(' ');
        }
        // Rows join into one paragraph: merge a run that continues the last one's style.
        if let (Some(last), Some(first)) = (runs.last_mut(), rr.first())
            && last.bold == first.bold
            && last.italic == first.italic
            && last.color == first.color
        {
            last.text.push_str(&first.text);
            rr.remove(0);
        }
        runs.extend(rr);
    }
    let lines: Vec<&&TextLine> = rows.iter().flatten().collect();
    let first = lines.first()?;
    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    if text.trim().is_empty() {
        return None;
    }
    let rect = lines.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |r, l| {
        [r[0].min(l.rect[0]), r[1].min(l.rect[1]), r[2].max(l.rect[2]), r[3].max(l.rect[3])]
    });
    let size = lines.iter().map(|l| l.size).filter(|s| s.is_finite()).fold(0.0, f64::max).max(first.size.max(1.0));
    Some(Block {
        bold: runs.iter().all(|r| r.bold),
        italic: runs.iter().all(|r| r.italic),
        color: first.color,
        text,
        rect,
        size,
        rotation: 0.0,
        runs,
    })
}

/// Paragraphs from rows: a new one where a row starts a list item, follows a row that stopped
/// short of the text's right edge, sits a line or more below the last, or changes size.
fn paragraphs(rows: Vec<Vec<&TextLine>>) -> Vec<Block> {
    let right = rows.iter().flatten().map(|l| l.rect[2]).filter(|x| x.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    let mut out = Vec::new();
    let mut current: Vec<Vec<&TextLine>> = Vec::new();
    for row in rows {
        let (Some(first), Some(last)) = (row.first(), row.last()) else { continue };
        let size = first.size.max(1.0);
        let text: String = row.iter().map(|l| l.text.as_str()).collect();
        let starts = match current.last().and_then(|r| r.first().zip(r.last())) {
            None => true,
            Some((pf, pl)) => {
                let short = pl.rect[2] < right - 3.0 * size;
                let gap = pf.rect[1] - last.rect[3].max(first.rect[3]);
                let resized = (pf.size - first.size).abs() > 0.15 * pf.size.max(first.size);
                starts_item(&shown(&text)) || short || gap > 1.0 * size || resized
            }
        };
        if starts && !current.is_empty() {
            out.extend(block(&current));
            current.clear();
        }
        current.push(row);
    }
    out.extend(block(&current));
    out
}

/// A ruled table from its grid: cells merged where the page draws no rule between them, the
/// text of the lines inside each, its background where a filled box covers it.
fn table(grid: &Grid, rules: &[&Rule], lines: &[&TextLine], fills: &[([f64; 4], [f64; 3])]) -> Table {
    let (cols, rows) = (grid.xs.len() - 1, grid.ys.len() - 1);
    let at = |r: usize, c: usize| r * cols + c;
    let mut parent: Vec<usize> = (0..rows * cols).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for r in 0..rows {
        for c in 0..cols {
            let (top, bottom, left, right) = (grid.ys[r], grid.ys[r + 1], grid.xs[c], grid.xs[c + 1]);
            if c + 1 < cols && !drawn(rules, false, right, bottom, top) {
                let (a, b) = (root(&mut parent, at(r, c)), root(&mut parent, at(r, c + 1)));
                parent[a] = b;
            }
            if r + 1 < rows && !drawn(rules, true, bottom, left, right) {
                let (a, b) = (root(&mut parent, at(r, c)), root(&mut parent, at(r + 1, c)));
                parent[a] = b;
            }
        }
    }
    // Each merged region's extent in grid cells; a region that is not a rectangle is split back.
    let mut regions: Vec<(usize, usize, usize, usize, usize)> = Vec::new(); // root, r0, r1, c0, c1
    let mut count: Vec<(usize, usize)> = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let k = root(&mut parent, at(r, c));
            match regions.iter_mut().find(|g| g.0 == k) {
                Some(g) => {
                    g.1 = g.1.min(r);
                    g.2 = g.2.max(r);
                    g.3 = g.3.min(c);
                    g.4 = g.4.max(c);
                }
                None => regions.push((k, r, r, c, c)),
            }
            match count.iter_mut().find(|e| e.0 == k) {
                Some(e) => e.1 += 1,
                None => count.push((k, 1)),
            }
        }
    }
    let mut owner: Vec<Option<(usize, usize, usize, usize)>> = vec![None; rows * cols];
    for &(k, r0, r1, c0, c1) in &regions {
        let n = count.iter().find(|e| e.0 == k).map_or(0, |e| e.1);
        if n == (r1 - r0 + 1) * (c1 - c0 + 1) {
            for r in r0..=r1 {
                for c in c0..=c1 {
                    owner[at(r, c)] = Some((r0, r1, c0, c1));
                }
            }
        }
    }
    let mut out_rows: Vec<Vec<Cell>> = vec![Vec::new(); rows];
    for r in 0..rows {
        for c in 0..cols {
            let (r0, r1, c0, c1) = owner[at(r, c)].unwrap_or((r, r, c, c));
            // A merged cell is written once, at its top left grid cell.
            if (r0, c0) != (r, c) {
                continue;
            }
            let rect = [grid.xs[c0], grid.ys[r1 + 1], grid.xs[c1 + 1], grid.ys[r0]];
            let mine: Vec<&TextLine> = lines.iter().copied().filter(|l| inside(l, rect)).collect();
            let first = mine.first().copied();
            let text = rows_text(rows_of(mine));
            let fill = fills.iter().find(|(f, _)| {
                let (cx, cy) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
                cx >= f[0] && cx <= f[2] && cy >= f[1] && cy <= f[3]
            });
            out_rows[r].push(Cell {
                text,
                size: first.map_or(10.0, |l| l.size),
                bold: first.is_some_and(|l| l.bold),
                italic: first.is_some_and(|l| l.italic),
                color: first.map_or([0.0; 3], |l| l.color),
                span: c1 - c0 + 1,
                rowspan: r1 - r0 + 1,
                fill: fill.map(|(_, color)| *color),
            });
        }
    }
    Table { rect: [grid.xs[0], grid.ys[rows], grid.xs[cols], grid.ys[0]], cols: grid.xs[..cols].to_vec(), rows: out_rows, row_edges: grid.ys.clone() }
}

fn rows_of(lines: Vec<&TextLine>) -> Vec<Vec<&TextLine>> {
    rows(lines)
}

/// A cell's text: its rows, each joined as the page spaces it, one row per line.
fn rows_text(rows: Vec<Vec<&TextLine>>) -> String {
    rows.iter().map(|r| row_runs(r).iter().map(|run| run.text.as_str()).collect::<String>().trim().to_owned()).collect::<Vec<_>>().join("\n")
}

/// The page's ruled tables and paragraphs, each with its top edge and left edge for ordering.
pub(crate) fn items(page: &Page, body: f64) -> Vec<(f64, f64, Item<'static>)> {
    let rules = rules(&page.shapes);
    let fills: Vec<([f64; 4], [f64; 3])> = page
        .shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Fill { rect, color } if finite(rect) && rect[2] - rect[0] > THIN && rect[3] - rect[1] > THIN => Some((*rect, *color)),
            _ => None,
        })
        .collect();
    let mut taken = vec![false; page.lines.len()];
    let mut out: Vec<(f64, f64, Item<'static>)> = Vec::new();
    for (grid, members) in grids(&rules) {
        let bounds = [grid.xs[0] - 1.0, grid.ys[grid.ys.len() - 1] - 1.0, grid.xs[grid.xs.len() - 1] + 1.0, grid.ys[0] + 1.0];
        let mut mine = Vec::new();
        for (i, l) in page.lines.iter().enumerate() {
            if !taken[i] && inside(l, bounds) {
                taken[i] = true;
                mine.push(l);
            }
        }
        let t = table(&grid, &members, &mine, &fills);
        out.push((t.rect[3], t.rect[0], Item::Table(t)));
    }
    let rest: Vec<&TextLine> = page.lines.iter().zip(&taken).filter(|(l, t)| !**t && !l.text.trim().is_empty()).map(|(l, _)| l).collect();
    for b in paragraphs(rows(rest)) {
        let lvl = level(&b, body);
        out.push((b.rect[3], b.rect[0], Item::Para(Cow::Owned(b), lvl)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x0: f64, top: f64, x1: f64) -> TextLine {
        TextLine {
            text: text.into(),
            rect: [x0, top - 12.0, x1, top],
            size: 12.0,
            font: "Batang".into(),
            bold: false,
            italic: false,
            color: [0.0; 3],
        }
    }
    fn h(y: f64, x0: f64, x1: f64) -> Shape {
        Shape::Line { from: [x0, y], to: [x1, y], width: 0.4, color: [0.0; 3] }
    }
    fn v(x: f64, y0: f64, y1: f64) -> Shape {
        Shape::Line { from: [x, y0], to: [x, y1], width: 0.4, color: [0.0; 3] }
    }

    #[test]
    fn list_marks_start_paragraphs_and_wrapped_lines_join() {
        let lines = vec![
            line("2. 첫 문단은 오른쪽 끝까지 이어지다가", 50.0, 700.0, 540.0),
            line("다음 줄에서 끝납니다.", 70.0, 682.0, 200.0),
            line("\u{AD} 근정훈장: 재직기간 33년 이상", 70.0, 664.0, 300.0),
            line("가. 추천대상", 60.0, 646.0, 140.0),
            line("※ 참고 사항", 60.0, 628.0, 140.0),
        ];
        let page = Page { width: 595.0, height: 842.0, lines, ..Page::default() };
        let texts: Vec<String> = items(&page, 12.0)
            .into_iter()
            .filter_map(|(_, _, it)| match it {
                Item::Para(b, _) => Some(b.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            ["2. 첫 문단은 오른쪽 끝까지 이어지다가 다음 줄에서 끝납니다.", "- 근정훈장: 재직기간 33년 이상", "가. 추천대상", "※ 참고 사항"]
        );
    }

    /// A ruled 3×3 grid whose right column's lower two cells are one (no rule between them) and
    /// whose header row is shaded; body text outside the rules stays a paragraph.
    #[test]
    fn ruled_tables_keep_merged_cells_and_leave_body_text_out() {
        let mut shapes = vec![h(500.0, 100.0, 400.0), h(480.0, 100.0, 400.0), h(460.0, 100.0, 300.0), h(440.0, 100.0, 400.0)];
        shapes.extend([v(100.0, 440.0, 500.0), v(200.0, 440.0, 500.0), v(300.0, 440.0, 500.0), v(400.0, 440.0, 500.0)]);
        shapes.push(Shape::Fill { rect: [100.0, 480.0, 400.0, 500.0], color: [0.9, 0.9, 0.9] });
        let lines = vec![
            line("본문은 표 밖에 있습니다.", 60.0, 560.0, 300.0),
            line("서류", 110.0, 496.0, 150.0),
            line("방법", 210.0, 496.0, 250.0),
            line("제출일", 310.0, 496.0, 350.0),
            line("공적조서", 110.0, 476.0, 170.0),
            line("한글파일", 210.0, 476.0, 270.0),
            line("10.19.(월)", 310.0, 466.0, 370.0),
            line("확인서", 110.0, 456.0, 160.0),
            line("PDF", 210.0, 456.0, 240.0),
        ];
        let page = Page { width: 595.0, height: 842.0, lines, shapes, ..Page::default() };
        let items = items(&page, 12.0);
        let tables: Vec<&Table> = items.iter().filter_map(|(_, _, it)| if let Item::Table(t) = it { Some(t) } else { None }).collect();
        assert_eq!(tables.len(), 1);
        let t = tables[0];
        let texts: Vec<Vec<(&str, usize)>> = t.rows.iter().map(|r| r.iter().map(|c| (c.text.as_str(), c.rowspan)).collect()).collect();
        assert_eq!(
            texts,
            [
                vec![("서류", 1), ("방법", 1), ("제출일", 1)],
                vec![("공적조서", 1), ("한글파일", 1), ("10.19.(월)", 2)],
                vec![("확인서", 1), ("PDF", 1)]
            ]
        );
        assert!(t.rows[0].iter().all(|c| c.fill.is_some()) && t.rows[1][0].fill.is_none());
        assert_eq!(t.row_edges.len(), 4);
        let paras: Vec<&str> = items.iter().filter_map(|(_, _, it)| if let Item::Para(b, _) = it { Some(b.text.as_str()) } else { None }).collect();
        assert_eq!(paras, ["본문은 표 밖에 있습니다."]);
    }

    #[test]
    fn list_marks() {
        for t in ["- 항목", "\u{AD} 항목", "○ 각 부서", "※ 참고", "1. 관련", "12) 항목", "가. 추천", "나) 항목", "[붙임 2]의", "  가. 들여쓴"]
        {
            assert!(starts_item(&shown(t)), "{t}");
        }
        for t in ["제출하여 주시기 바랍니다.", "2027년도 2월말", "가나다라", "100,000원"] {
            assert!(!starts_item(t), "{t}");
        }
    }

    #[test]
    fn hostile_rules_stay_bounded() {
        let shapes: Vec<Shape> =
            (0..10_000).map(|i| if i % 2 == 0 { h(f64::from(i % 300), 0.0, 500.0) } else { v(f64::from(i % 500), 0.0, 300.0) }).collect();
        let page = Page { width: 595.0, height: 842.0, lines: vec![line("x", 10.0, 100.0, 20.0)], shapes, ..Page::default() };
        let items = items(&page, 12.0);
        for (_, _, it) in &items {
            if let Item::Table(t) = it {
                assert!(t.cols.len() <= MAX_GRID && t.rows.len() <= MAX_GRID);
            }
        }
        let nan = Page {
            lines: vec![TextLine { rect: [f64::NAN; 4], ..line("y", 0.0, 0.0, 0.0) }],
            shapes: vec![h(f64::NAN, 0.0, f64::INFINITY)],
            ..Page::default()
        };
        let _ = super::items(&nan, 12.0);
    }
}
