// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! Exporting slides without a window: numbered PNGs or a PDF.
//!
//!   ibisOffice --export png [--out-dir <dir>] [--width 1920] [--slides 1,3-5] <file.pptx>
//!   ibisOffice --export pdf [--out-dir <dir>] [--slides …] <file.pptx>
//!   ibisOffice --export pdf-image [--out-dir <dir>] [--width 1920] [--slides …] <file.pptx>
//!
//! Files are named after the input: `<name>-01.png`, … or `<name>.pdf`, in
//! the output directory (the current directory by default).
//!
//! Slides are drawn by the same code as the editor, on a headless renderer
//! (GPUI's Metal one on macOS, the DirectX one of ibisOffice's fork of GPUI's
//! Windows platform on Windows), so an export shows exactly what the editor
//! shows. The PDF
//! is vector graphics made from what GPUI painted (text as text, in the
//! fonts drawn with: see `pdf_vector`), with worksheets on their paper, in
//! pages as Excel prints them; `pdf-image` holds one picture per slide or
//! sheet instead (text is not selectable).
//!
//! Needs the `export` feature (on by default), which enables GPUI's test
//! support for its headless renderer.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use ibis_doc_model::{EmuRect, Presentation, SheetPrint, Slide};

use crate::render::shape_view::{Resources, shape_element, slide_backdrop};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    /// Vector graphics.
    Pdf,
    /// A picture per page.
    PdfImage,
}

pub struct Options {
    pub format: Format,
    /// Directory the files are written to.
    pub out_dir: PathBuf,
    /// File name stem, usually the input's (`決算資料` for `決算資料.pptx`).
    pub name: String,
    /// Width of each slide picture in pixels.
    pub width: u32,
    /// 1-based slide numbers; empty for all.
    pub slides: Vec<usize>,
    /// Also write `<name>-NN.layout.tsv` next to each PNG: where each element
    /// is drawn, for telling what a rendering difference belongs to.
    pub layout: bool,
    /// The input's file name, for a document's `FILENAME` fields.
    pub file_name: Option<String>,
    /// 印刷オプション: 背景の色とイメージを印刷する (DOCX-OPTIONS-010).
    pub print_backgrounds: bool,
    /// 印刷オプション: 隠し文字を印刷する (DOCX-OPTIONS-010).
    pub print_hidden_text: bool,
    /// 印刷の設定: the colour of the pictures (`PdfImage`).
    pub color: crate::export_print::PrintColor,
    /// 配布資料: slides to a page (`PdfImage`).
    pub handout: Option<u8>,
    /// A frame round each slide (`PdfImage`).
    pub frame: bool,
    /// 配布資料 on a landscape page, with a header, date, footer and page number.
    pub handout_landscape: bool,
    /// ノート ページ (`PdfImage`): each slide above its notes on a page.
    pub notes_pages: bool,
    /// The file's bytes, for the notes' formatting (`notes_pages`).
    pub notes_source: Option<Vec<u8>>,
    pub handout_marks: crate::export_print::HandoutMarks,
}

/// The notes of slide `index` (from 0) with their formatting (bold, size, colour, bullets), read from the
/// notes page of the file; the plain `notes` if it cannot be.
fn rich_notes(source: Option<&[u8]>, index: usize, notes: &str) -> crate::export_print::NoteText {
    use crate::export_print::{NotePara, NoteRun, NoteText};
    let plain = || NoteText::plain(notes);
    if notes.trim().is_empty() {
        return plain();
    }
    let Some(data) = source else { return plain() };
    let Ok(temp) = ibis_pptx::notes_page::build(data, index) else { return plain() };
    let Ok(opened) = ibis_pptx::open(temp) else { return plain() };
    let Some(slide) = opened.presentation.slides.first() else { return plain() };
    let normal = |t: &str| t.replace('\u{b}', "\n").trim().to_string();
    let body = slide.shapes.iter().filter_map(|s| s.text_body()).find(|b| normal(&b.text()) == notes.trim()).or_else(|| slide.shapes.iter().filter_map(|s| s.text_body()).find(|b| !b.text().trim().is_empty()));
    let Some(body) = body else { return plain() };
    NoteText {
        paragraphs: body
            .paragraphs
            .iter()
            .map(|p| NotePara {
                level: p.level,
                bullet: p.bullet.as_ref().map(|b| b.text.clone()).filter(|t| !t.is_empty()),
                runs: p
                    .runs
                    .iter()
                    .map(|r| {
                        let style = body.style.with_run(&r.style);
                        let c = style.color.0;
                        NoteRun { text: r.text.clone(), bold: style.bold, size_pt: Some(style.font_size_pt()), color: Some((((c >> 16) & 0xff) as u8, ((c >> 8) & 0xff) as u8, (c & 0xff) as u8)) }
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Parse a slide list such as `1,3,5-8`.
pub fn parse_slides(text: &str) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bad = || format!("Cannot read the slide specification: {part}");
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b): (usize, usize) = (a.trim().parse().map_err(|_| bad())?, b.trim().parse().map_err(|_| bad())?);
                if a == 0 || b < a {
                    return Err(bad());
                }
                out.extend(a..=b);
            }
            None => out.push(part.parse().ok().filter(|n| *n > 0).ok_or_else(bad)?),
        }
    }
    Ok(out)
}

/// One slide drawn at a fixed size, as the export window's root view.
struct SlidePicture {
    slide: Slide,
    scale: f32,
    size: Size<Pixels>,
    /// The top-left corner of the area drawn, in pixels of the slide.
    origin: (f32, f32),
    /// Where the area goes in the window, and its size (a printed sheet's
    /// part on its paper; else the whole window).
    inset: (f32, f32),
    view: Size<Pixels>,
    resources: Rc<Resources>,
    /// A printed sheet's header and footer, in pixels of the page.
    marks: Vec<crate::xlsx::header_footer::PageMark>,
    /// Black lines on the page (x, y, width, height in pixels): printed
    /// row and column headings' frame.
    rules: Vec<(f32, f32, f32, f32)>,
    /// More parts of the slide drawn on the page (a printed sheet's title
    /// rows and columns): each its origin in the slide, its place on the
    /// page and its size, in pixels.
    extra: Vec<((f32, f32), (f32, f32), Size<Pixels>)>,
}

impl Render for SlidePicture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (scale, res) = (self.scale, &self.resources);
        let part = |origin: (f32, f32), inset: (f32, f32), view: Size<Pixels>| {
            div().absolute().left(px(inset.0)).top(px(inset.1)).w(view.width).h(view.height).overflow_hidden().child(
                div()
                    .absolute()
                    .left(px(-origin.0))
                    .top(px(-origin.1))
                    .size_full()
                    .child(slide_backdrop(&self.slide, scale, res))
                    .children(self.slide.shown_shapes().map(|s| shape_element(s, s.frame, scale, res, None))),
            )
        };
        let extra: Vec<Div> = self.extra.iter().map(|&(o, i, v)| part(o, i, v)).collect();
        div().relative().w(self.size.width).h(self.size.height).overflow_hidden().bg(rgb(0xffffff)).children(extra).child(
            div().absolute().left(px(self.inset.0)).top(px(self.inset.1)).w(self.view.width).h(self.view.height).overflow_hidden().child(
                div()
                    .absolute()
                    .left(px(-self.origin.0))
                    .top(px(-self.origin.1))
                    .size_full()
                    .child(slide_backdrop(&self.slide, scale, res))
                    .children(self.slide.shown_shapes().map(|s| shape_element(s, s.frame, scale, res, None))),
            ),
        )
        .when(!self.marks.is_empty() || !self.rules.is_empty(), |d| {
            let (marks, rules) = (self.marks.clone(), self.rules.clone());
            d.child(
                canvas(|_, _, _| {}, move |bounds, _, window, cx| {
                    for &(x, y, w, h) in &rules {
                        window.paint_quad(fill(Bounds::new(point(bounds.origin.x + px(x), bounds.origin.y + px(y)), gpui_kit::size(px(w), px(h))), gpui_kit::black()));
                    }
                    paint_marks(&marks, bounds.origin, window, cx)
                })
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
        })
    }
}

/// Paint a header's or footer's runs, each from its baseline.
fn paint_marks(marks: &[crate::xlsx::header_footer::PageMark], origin: Point<Pixels>, window: &mut Window, cx: &mut App) {
    for m in marks {
        if m.run.text.is_empty() {
            continue;
        }
        let rgb_of = |c: ibis_doc_model::Rgb| gpui_kit::rgb(c.0 & 0xFF_FFFF);
        let color: Hsla = m.run.color.map_or(gpui_kit::black(), |c| rgb_of(c).into());
        let size = m.run.size;
        let run = TextRun {
            len: m.run.text.len(),
            font: m.font.clone(),
            color,
            background_color: None,
            underline: (m.run.underline == 1).then(|| UnderlineStyle { thickness: px((size / 16.0).max(0.5)), color: Some(color), wavy: false }),
            strikethrough: m.run.strike.then(|| StrikethroughStyle { thickness: px((size / 16.0).max(0.5)), color: Some(color) }),
        };
        let line = window.text_system().shape_line(SharedString::from(m.run.text.clone()), px(size), &[run], None);
        let at = point(origin.x + px(m.x), origin.y + px(m.baseline) - line.ascent);
        line.paint(at, line.ascent + line.descent, TextAlign::Left, None, window, cx).ok();
        if m.run.underline == 2 {
            // A double underline: two thin lines under the baseline.
            let t = (size / 20.0).max(0.4);
            for k in [1.0, 2.6] {
                let y = origin.y + px(m.baseline + t * k * 1.5);
                window.paint_quad(fill(Bounds::new(point(origin.x + px(m.x), y), gpui_kit::size(line.width, px(t))), color));
            }
        }
    }
}

/// How a worksheet prints: its page setup, the scale it prints at, and the
/// part of `area` (EMU of the slide) on each page, in the order printed.
///
/// As Excel lays it out: a page holds whole columns and rows as far as they
/// fit (a column or row larger than the page is cut), a manual break starts
/// a new page, and fitting to a number of pages shrinks the sheet until its
/// pages are that many (and leaves out the manual breaks).
/// A sheet's print titles (`_xlnm.Print_Titles`): the rows repeated at the
/// top of each page and the columns at the left, as spans of the slide
/// (EMU).
pub(crate) type Titles = (Option<(i64, i64)>, Option<(i64, i64)>);

fn print_titles(presentation: &Presentation, slide: &Slide) -> Titles {
    let Some(workbook) = presentation.workbook.as_ref() else { return (None, None) };
    let Some(index) = workbook.sheets.iter().position(|s| s.slide == Some(slide.id)) else { return (None, None) };
    let Some(name) = workbook.names.iter().find(|n| n.sheet == Some(index) && n.name.eq_ignore_ascii_case("_xlnm.Print_Titles")) else { return (None, None) };
    let Some((frame, sheet)) = slide.shapes.iter().find_map(|s| match &s.kind {
        ibis_doc_model::ShapeKind::Sheet(sheet) => Some((s.frame, sheet)),
        _ => None,
    }) else {
        return (None, None);
    };
    let (rows, columns) = crate::xlsx::header_footer::parse_titles(&name.formula);
    let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
    let at = |list: &[i64], i: u32| list[(i as usize).min(list.len() - 1)];
    (
        rows.map(|(a, b)| (frame.y + at(&ys, a), frame.y + at(&ys, b + 1))).filter(|(a, b)| b > a),
        columns.map(|(a, b)| (frame.x + at(&xs, a), frame.x + at(&xs, b + 1))).filter(|(a, b)| b > a),
    )
}

fn sheet_pages(slide: &Slide, area: EmuRect, titles: Titles, headings: (f64, f64)) -> Option<(SheetPrint, f64, Vec<EmuRect>)> {
    let (frame, sheet) = slide.shapes.iter().find_map(|s| match &s.kind {
        ibis_doc_model::ShapeKind::Sheet(sheet) => Some((s.frame, sheet)),
        _ => None,
    })?;
    let print = sheet.print.clone();
    let (paper_w, paper_h) = print.page_size();
    let [left, right, top, bottom] = print.margins;
    let least = ibis_doc_model::EMU_PER_INCH / 2;
    let room = ((paper_w - left - right).max(least) as f64, (paper_h - top - bottom).max(least) as f64);
    let xs: Vec<i64> = sheet.column_offsets().iter().map(|x| frame.x + x).collect();
    let ys: Vec<i64> = sheet.row_offsets().iter().map(|y| frame.y + y).collect();
    let at = |edges: &[i64], breaks: &[u32]| -> Vec<i64> { breaks.iter().filter_map(|b| edges.get(*b as usize)).copied().collect() };
    // Fitting to a number of pages leaves out the manual breaks along that
    // axis; along an axis of automatic pages (0) they are kept: Excel for Mac
    // prints 1 wide × automatic tall with a row break on 2 pages and with a
    // column break on 1 (XPAGE-61 to 67).
    let (column_breaks, row_breaks) = match print.fit_to {
        Some((across, down)) => (
            if across == 0 { at(&xs, &print.column_breaks) } else { Vec::new() },
            if down == 0 { at(&ys, &print.row_breaks) } else { Vec::new() },
        ),
        None => (at(&xs, &print.column_breaks), at(&ys, &print.row_breaks)),
    };
    let mut scale = match print.fit_to {
        Some((across, down)) => {
            // The sheet prints a point wider and taller than its cells: the
            // grid line before the first column and row (borders and fills
            // reach over it), as Excel fits it (a 554 pt wide print area on
            // 537.7 pt of room prints at 96 %, not 97: xlsx-XBRD BB14).
            let (w, h) = (area.width + 12_700, area.height + 12_700);
            let mut s: f64 = 1.0;
            if across > 0 {
                s = s.min(room.0 * across as f64 / w.max(1) as f64);
            }
            if down > 0 {
                s = s.min(room.1 * down as f64 / h.max(1) as f64);
            }
            // A whole percentage, as Excel prints.
            (s * 100.0).floor() / 100.0
        }
        None => print.scale as f64 / 100.0,
    }
    .max(0.1);
    loop {
        // A page that does not hold the title rows (columns) has them
        // printed over (left of) it, taking their room.
        let reserve = |t: Option<(i64, i64)>| t.map(|(a, b)| (b, b - a));
        // Printed row and column headings take their room on every page.
        let (hw, hh) = if print.headings { (headings.0 * 12_700.0, headings.1 * 12_700.0) } else { (0.0, 0.0) };
        let columns = page_spans(&xs, &column_breaks, area.x, area.x + area.width, room.0 / scale - hw, reserve(titles.1));
        let rows = page_spans(&ys, &row_breaks, area.y, area.y + area.height, room.1 / scale - hh, reserve(titles.0));
        let fits = print.fit_to.is_none_or(|(across, down)| (across == 0 || columns.len() <= across as usize) && (down == 0 || rows.len() <= down as usize));
        if fits || scale <= 0.1 {
            let page = |(x0, x1): (i64, i64), (y0, y1): (i64, i64)| EmuRect::new(x0, y0, x1 - x0, y1 - y0);
            let pages = if print.over_then_down {
                rows.iter().flat_map(|r| columns.iter().map(|c| page(*c, *r))).collect()
            } else {
                columns.iter().flat_map(|c| rows.iter().map(|r| page(*c, *r))).collect()
            };
            return Some((print, scale, pages));
        }
        scale = ((scale * 100.0).round() - 1.0).max(10.0) / 100.0;
    }
}

/// `start..end` cut into spans no longer than `room`, at the cell `edges`
/// (sorted) where they can be and at the manual `breaks`.
fn page_spans(edges: &[i64], breaks: &[i64], start: i64, end: i64, room: f64, titles: Option<(i64, i64)>) -> Vec<(i64, i64)> {
    let room = (room.floor() as i64).max(1);
    let mut out = Vec::new();
    let mut at = start;
    while at < end {
        // Past the titles (their end, their size), a page holds less.
        let here = match titles {
            Some((title_end, size)) if at >= title_end => (room - size).max(room / 4).max(1),
            _ => room,
        };
        let limit = (at + here).min(end);
        let next = match breaks.iter().find(|b| **b > at && **b < limit) {
            Some(b) => *b,
            None if limit == end => end,
            None => {
                // The furthest edge that fits; a cell larger than the page is cut.
                let i = edges.partition_point(|e| *e <= limit);
                edges[..i].last().copied().filter(|e| *e > at).unwrap_or(limit)
            }
        };
        out.push((at, next));
        at = next;
    }
    out
}

/// The part of `slide` to export: a worksheet's print area, as Excel
/// prints it, or the whole slide.
/// The pages a worksheet prints on (as spans of the slide, EMU), for the
/// page break preview and the page layout view: the cut lines are where it
/// breaks; and the scale it prints at (cells are drawn at that scale on paper).
pub(crate) fn preview_pages(presentation: &Presentation, slide: &Slide, resources: &Resources) -> (f64, Vec<EmuRect>) {
    let area = export_area(presentation, slide);
    let titles = print_titles(presentation, slide);
    let default_style = presentation.workbook.as_ref().and_then(|w| w.default_style.clone()).unwrap_or_default();
    let headings = slide
        .shapes
        .iter()
        .find_map(|s| match &s.kind {
            ibis_doc_model::ShapeKind::Sheet(sheet) => Some(crate::xlsx::header_footer::heading_size(sheet, &default_style, resources)),
            _ => None,
        })
        .unwrap_or((0.0, 0.0));
    sheet_pages(slide, area, titles, headings).map(|(_, scale, pages)| (scale, pages)).unwrap_or((1.0, Vec::new()))
}

fn export_area(presentation: &Presentation, slide: &Slide) -> EmuRect {
    let size = presentation.size_of(slide);
    let whole = ibis_doc_model::EmuRect::new(0, 0, size.width, size.height);
    let Some(sheet) = slide.shapes.iter().find_map(|s| match &s.kind {
        ibis_doc_model::ShapeKind::Sheet(sheet) => Some((s.frame, sheet)),
        _ => None,
    }) else {
        return whole;
    };
    let (frame, sheet) = sheet;
    let Some(area) = sheet.print_area else { return whole };
    let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
    let at = |list: &[i64], i: u32| list[(i as usize).min(list.len() - 1)];
    let (x0, y0) = (at(&xs, area.first_column), at(&ys, area.first_row));
    // Whole rows or columns reach only as far as the sheet is used (the
    // grid goes on past it).
    let (x1, y1) = (at(&xs, area.last_column.saturating_add(1)).min(size.width - frame.x), at(&ys, area.last_row.saturating_add(1)).min(size.height - frame.y));
    ibis_doc_model::EmuRect::new(frame.x + x0, frame.y + y0, (x1 - x0).max(1), (y1 - y0).max(1))
}

/// A sheet printed black and white (`pageSetup/@blackAndWhite`): no fills,
/// black text and black lines (Excel for Mac's PDF of XPAGE-42: the header's
/// blue fill gone, the grey borders black).
fn black_and_white(slide: &mut Slide) {
    for shape in &mut slide.shapes {
        let ibis_doc_model::ShapeKind::Sheet(sheet) = &mut shape.kind else { continue };
        if !sheet.print.black_and_white {
            continue;
        }
        let sheet = std::sync::Arc::make_mut(sheet);
        for style in &mut sheet.styles {
            style.fill = None;
            style.fill_theme = None;
            style.gradient = None;
            style.pattern = None;
            style.color = ibis_doc_model::Rgb::BLACK;
            style.color_theme = None;
            for border in [&mut style.left, &mut style.right, &mut style.top, &mut style.bottom, &mut style.diagonal_up, &mut style.diagonal_down].into_iter().flatten() {
                border.color = ibis_doc_model::Rgb::BLACK;
            }
        }
    }
}

/// A headless app to draw in, with the platform's text system (text laid
/// out as in the editor) and its headless renderer if it has one, and the
/// platform, to be kept as long as the app.
pub(crate) fn headless_app() -> (Rc<dyn Platform>, HeadlessAppContext) {
    // On Windows the headless platform has no DirectWrite (its text system
    // lays out nothing): the full one, which opens no window either.
    let platform = gpui_kit::platform::current_platform(cfg!(not(windows)));
    let cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(crate::shell::icons::Assets::new("")),
        headless_renderer,
    );
    (platform, cx)
}

/// The platform's renderer drawing without a window: GPUI's Metal one, or
/// on Windows the DirectX one of ibisOffice's fork (GPUI has none there).
fn headless_renderer() -> Option<Box<dyn PlatformHeadlessRenderer>> {
    #[cfg(windows)]
    return gpui_windows::DirectXHeadlessRenderer::new()
        .map_err(|e| log::error!("DirectX: {e:#}"))
        .ok()
        .map(|renderer| Box::new(renderer) as Box<dyn PlatformHeadlessRenderer>);
    #[cfg(not(windows))]
    gpui_kit::platform::current_headless_renderer()
}

/// Print the time since `since` (and restart it) under `IBIS_DEBUG_TIMING`.
fn lap(what: &str, since: &mut crate::clock::Instant) {
    if std::env::var_os("IBIS_DEBUG_TIMING").is_some() {
        eprintln!("export: {what} {:.0} ms", since.elapsed().as_secs_f64() * 1000.0);
    }
    *since = crate::clock::Instant::now();
}

/// Export `presentation` as `options` say. Prints each file written.
pub fn export(mut presentation: Presentation, options: &Options) -> Result<(), String> {
    // Pictures are drawn as a PDF rasterizer draws them (the coverage
    // tests' references): glyphs without macOS' font smoothing, which
    // thickens their stems (16 % more ink in PTXT; the screen keeps it, as
    // macOS and Office draw text there).
    #[cfg(target_os = "macos")]
    gpui_macos::set_font_smoothing(false);
    // A picture of a page needs the platform's headless renderer (what paints
    // it); the vector PDF is written from what the window recorded, and needs
    // none. Linux, where the app draws with wgpu, has no headless renderer:
    // say so here rather than after laying the whole document out.
    if options.format != Format::Pdf && headless_renderer().is_none() {
        return Err("this platform cannot draw pages as pictures (--export png, pdf-image); it writes them as a vector PDF (--export pdf)".into());
    }
    let mut lap_at = crate::clock::Instant::now();
    let (_platform, mut cx) = headless_app();
    lap("headless app", &mut lap_at);
    // Picture decoding runs on real threads.
    cx.allow_parking();
    // Sheets come out as Excel prints them, without grid lines.
    let resources = Rc::new(cx.update(|app| {
        let mut resources = Resources::new(&presentation, app);
        resources.print = true;
        resources.print_backgrounds = options.print_backgrounds;
        resources.print_hidden_text = options.print_hidden_text;
        // Shapes where they are meant to be, as PDF rasterizers draw them.
        // A presentation's picture (PNG, a picture per PDF page) stays on
        // whole pixels as the screen draws it; its vector PDF is exact, its
        // coordinates PowerPoint's (2026-10-06, tasks/doc/pptx-pdf-vs-png.md:
        // exact matched PowerPoint's PDF coordinates in 32 more tests and in
        // none fewer).
        resources.exact = presentation.flow.is_some() || presentation.workbook.is_some() || options.format == Format::Pdf;
        // Printed with every picture.
        resources.preload.wait_all();
        resources
    }));
    // A flowing document becomes its pages; the art borders' pictures are
    // made for the export's pixels (the pages fill `options.width`).
    if let Some(doc) = &presentation.flow {
        let page_pt = doc.page.size.width as f32 / ibis_doc_model::EMU_PER_POINT as f32;
        if page_pt > 0.0 {
            crate::docx::art_border::set_export_px_per_pt(options.width.max(16) as f32 / page_pt);
        }
    }
    crate::docx::flow_layout::update_fields_on_open(&mut presentation, &resources);
    crate::docx::flow_layout::expand_pages(&mut presentation, &resources, options.file_name.as_deref());
    let ts = cx.update(|app| app.text_system().clone());
    crate::xlsx::pivot::fit_built_pivots(&mut presentation, &resources, &ts);
    lap("fields, pages, pivots", &mut lap_at);
    let count = presentation.slides.len();
    let numbers: Vec<usize> = if options.slides.is_empty() { (1..=count).collect() } else { options.slides.clone() };
    if let Some(n) = numbers.iter().find(|n| **n > count) {
        return Err(format!("Slide {n} does not exist ({count} slides in total)"));
    }

    // The pictures drawn through SVG (rotated, cut to shapes, recoloured),
    // rasterized ahead on several threads: making the pictures' and shapes'
    // elements makes them.
    for &number in &numbers {
        let slide = &presentation.slides[number - 1];
        let _ = slide_backdrop(slide, 1.0, &resources);
        for shape in slide.shapes.iter().filter(|s| matches!(s.kind, ibis_doc_model::ShapeKind::Picture(_) | ibis_doc_model::ShapeKind::Auto(_))) {
            let _ = shape_element(shape, shape.frame, 1.0, &resources, None);
        }
    }
    cx.update(|app| resources.rasterize_svg_pictures(&app.svg_renderer()));

    // The renderer draws at the display's pixel density; size the window so
    // pictures come out `options.width` pixels wide.
    let density = pixel_density(&mut cx)?;
    crate::docx::art_border::set_device_scale(density);
    let width = options.width.max(16) as f32 / density;

    lap("pictures, density", &mut lap_at);
    let mut pages = Vec::new();
    let mut vector = (options.format == Format::Pdf).then(crate::pdf_vector::PdfWriter::new);
    crate::render::chart_view::VECTOR_EXPORT.with(|v| v.set(vector.is_some()));
    // The window a vector PDF's pages are drawn in, and its size.
    let mut page_window: Option<(WindowHandle<SlidePicture>, Size<Pixels>)> = None;
    for &number in &numbers {
        let mut slide = presentation.slides[number - 1].clone();
        black_and_white(&mut slide);
        let area = export_area(&presentation, &slide);
        // The scale the sheet prints at, for its dashed borders.
        let titles = print_titles(&presentation, &slide);
        let default_style = presentation.workbook.as_ref().and_then(|w| w.default_style.clone()).unwrap_or_default();
        let headings = slide.shapes.iter().find_map(|s| match &s.kind {
            ibis_doc_model::ShapeKind::Sheet(sheet) => Some(crate::xlsx::header_footer::heading_size(sheet, &default_style, &resources)),
            _ => None,
        }).unwrap_or((0.0, 0.0));
        resources.print_scale.set(sheet_pages(&slide, area, titles, headings).map_or(1.0, |(_, scale, _)| scale));
        // A worksheet in a PDF comes out on its paper, a page at a time.
        if let Some(writer) = &mut vector
            && let Some((print, scale, sheet_pages)) = sheet_pages(&slide, area, titles, headings)
        {
            print_sheet(&mut cx, writer, slide, &print, scale, &sheet_pages, titles, headings, density, &resources, &default_style, options.file_name.as_deref().unwrap_or(""))?;
            continue;
        }
        // The renderer's textures reach 16 384 pixels; a longer sheet comes
        // out narrower.
        let scale = (width / area.width as f32).min(16_000.0 / density / area.height as f32);
        let window_size = gpui_kit::size(px((area.width as f32 * scale).round()), px((area.height as f32 * scale).round()));
        let origin = (area.x as f32 * scale, area.y as f32 * scale);
        if let Some(writer) = &mut vector {
            // One window for the pages of a size (making one takes a
            // renderer), the page changed in it, which draws it.
            let mut media = Vec::new();
            crate::render::preload::slide_media(&slide, &mut media);
            let reused = page_window.as_ref().filter(|(_, size)| *size == window_size).map(|(window, _)| *window);
            let window = match reused {
                Some(window) => {
                    cx.update(|app| {
                        window.update(app, |picture, _, cx| {
                            (picture.slide, picture.scale, picture.origin) = (slide, scale, origin);
                            cx.notify();
                        })
                    })
                    .map_err(|e| e.to_string())?;
                    window
                }
                None => {
                    if let Some((old, _)) = page_window.take() {
                        cx.update_window(old.into(), |_, window, _| window.remove_window()).ok();
                    }
                    let resources = resources.clone();
                    let window = cx
                        .open_window(window_size, |_, app| app.new(|_| SlidePicture { slide, scale, size: window_size, origin, inset: (0.0, 0.0), view: window_size, resources, marks: Vec::new(), extra: Vec::new(), rules: Vec::new() }))
                        .map_err(|e| format!("Cannot create the window for drawing: {e}"))?;
                    cx.update_window(window.into(), |_, window, _| {
                        window.record_paints();
                        // Only recorded: the PDF is made of the record.
                        window.record_paints_only(true);
                    })
                    .map_err(|e| e.to_string())?;
                    page_window = Some((window, window_size));
                    window
                }
            };
            // A page with pictures is drawn until those GPUI loads itself
            // (turned and recoloured ones) have arrived; one without is
            // drawn once.
            let rounds = if media.is_empty() && reused.is_some() { 0 } else { 3 };
            for _ in 0..rounds {
                cx.run_until_parked();
                cx.update_window(window.into(), |_, window, _| window.refresh()).map_err(|e| e.to_string())?;
            }
            cx.run_until_parked();
            let records = cx
                .update_window(window.into(), |_, window, _| {
                    let records = window.take_recorded_paints();
                    // Recording goes on for the next page.
                    window.record_paints();
                    records
                })
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            let text_system = cx.text_system().clone();
            let page_pt = (area.width as f32 / 12_700.0, area.height as f32 / 12_700.0);
            writer.add_page(records, f32::from(window_size.width), page_pt, density, &mut |id| text_system.font_data(id));
            continue;
        }
        let resources = resources.clone();
        let window = cx
            .open_window(window_size, |_, app| app.new(|_| SlidePicture { slide, scale, size: window_size, origin, inset: (0.0, 0.0), view: window_size, resources, marks: Vec::new(), extra: Vec::new(), rules: Vec::new() }))
            .map_err(|e| format!("Cannot create the window for drawing: {e}"))?;
        // Pictures decode asynchronously; draw until they have all arrived.
        let mut image = None;
        for _ in 0..4 {
            cx.run_until_parked();
            cx.update_window(window.into(), |_, window, _| window.refresh()).map_err(|e| e.to_string())?;
            cx.run_until_parked();
            image = Some(cx.capture_screenshot(window.into()).map_err(|e| format!("Cannot draw slide {number}: {e}"))?);
        }
        let mut image = image.expect("at least one capture");
        crate::export_print::recolor(&mut image, options.color);
        if options.frame && options.handout.is_none() && !options.notes_pages {
            crate::export_print::frame(&mut image);
        }
        cx.update_window(window.into(), |_, window, _| window.remove_window()).ok();
        // if image.width() > options.width {
        //     image = image::imageops::crop_imm(&image, 0, 0, options.width, image.height()).to_image();
        // }

        match options.format {
            Format::Png => {
                let path = options.out_dir.join(format!("{}-{number:02}.png", options.name));
                image.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                println!("{}", path.display());
                if options.layout {
                    let px_per_emu = image.width() as f32 / area.width as f32;
                    let path = options.out_dir.join(format!("{}-{number:02}.layout.tsv", options.name));
                    let slide = &presentation.slides[number - 1];
                    std::fs::write(&path, layout_tsv(slide, px_per_emu, (area.x, area.y))).map_err(|e| format!("{}: {e}", path.display()))?;
                    // The size of the area drawn, in points (a sheet's print
                    // area), for matching the picture to a printed page.
                    let path = options.out_dir.join(format!("{}-{number:02}.area.txt", options.name));
                    let points = format!("{} {}\n", area.width as f32 / 12_700.0, area.height as f32 / 12_700.0);
                    std::fs::write(&path, points).map_err(|e| format!("{}: {e}", path.display()))?;
                }
            }
            Format::PdfImage => pages.push((image, (area.width as f32 / 12_700.0, area.height as f32 / 12_700.0))),
            Format::Pdf => {}
        }
    }
    lap("all pages", &mut lap_at);
    if let Some((window, _)) = page_window {
        cx.update_window(window.into(), |_, window, _| window.remove_window()).ok();
    }
    if let Some(writer) = vector {
        crate::render::chart_view::VECTOR_EXPORT.with(|v| v.set(false));
        if writer.missing_glyphs > 0 {
            eprintln!("ibisOffice: could not get the font data, so {} characters could not be written", writer.missing_glyphs);
        }
        let path = options.out_dir.join(format!("{}.pdf", options.name));
        let (pdf, outlined) = writer.finish()?;
        lap("PDF written (finish)", &mut lap_at);
        if !outlined.is_empty() {
            eprintln!("ibisOffice: characters of fonts that cannot be subsetted were written as outlines (they cannot be selected as text): {}", outlined.join(", "));
        }
        std::fs::write(&path, pdf).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{}", path.display());
    }
    if options.format == Format::PdfImage {
        let path = options.out_dir.join(format!("{}.pdf", options.name));
        // ノート ページ: each slide's picture above its notes.
        if options.notes_pages {
            let images: Vec<image::RgbaImage> = pages.into_iter().map(|(image, _)| image).collect();
            let notes: Vec<crate::export_print::NoteText> = numbers.iter().map(|n| rich_notes(options.notes_source.as_deref(), n - 1, &presentation.slides[n - 1].notes)).collect();
            pages = crate::export_print::notes_pages(&images, &notes, &options.handout_marks);
        } else if let Some(per_page) = options.handout {
            // 配布資料: the slides' pictures put on pages.
            let images: Vec<image::RgbaImage> = pages.into_iter().map(|(image, _)| image).collect();
            pages = crate::export_print::handouts(&images, per_page, options.frame, options.handout_landscape, &options.handout_marks);
        }
        write_pdf(&path, &pages)?;
        println!("{}", path.display());
    }
    Ok(())
}

/// Add the pages of a worksheet to `writer`: each `pages` part of the
/// sheet, at `scale`, on the paper and within the margins of `print`.
#[allow(clippy::too_many_arguments)]
fn print_sheet(
    cx: &mut HeadlessAppContext,
    writer: &mut crate::pdf_vector::PdfWriter,
    slide: Slide,
    print: &SheetPrint,
    scale: f64,
    pages: &[EmuRect],
    titles: Titles,
    headings: (f64, f64),
    density: f32,
    resources: &Rc<Resources>,
    default_style: &ibis_doc_model::CellStyle,
    file_name: &str,
) -> Result<(), String> {
    // A pixel a point: the PDF is vector graphics, so the size drawn at
    // matters only for layout (and drawing larger takes longer).
    const PX_PER_EMU: f64 = 1.0 / 12_700.0;
    let (paper_w, paper_h) = print.page_size();
    let [left, right, top, bottom] = print.margins;
    let window_size = gpui_kit::size(px((paper_w as f64 * PX_PER_EMU) as f32), px((paper_h as f64 * PX_PER_EMU) as f32));
    let px_per_emu = (scale * PX_PER_EMU) as f32;
    let resources = resources.clone();
    let sheet_name = slide.name.clone().unwrap_or_default();
    // Where the sheet's charts are, to cut them out of its pages
    // (`IBIS_CHART_RECTS=<file>`: for comparing charts with Excel's PDF).
    let chart_frames: Vec<(EmuRect, [usize; 4])> = std::env::var_os("IBIS_CHART_RECTS")
        .map(|_| {
            let cells = slide.shapes.iter().find_map(|s| match &s.kind {
                ibis_doc_model::ShapeKind::Sheet(sheet) => Some((s.frame, sheet.column_offsets(), sheet.row_offsets())),
                _ => None,
            });
            // The first and last column and row (from 0) a frame covers.
            let index = |edges: &[i64], start: i64, at: i64| edges.partition_point(|e| start + *e <= at).saturating_sub(1);
            slide
                .shapes
                .iter()
                .filter(|s| matches!(s.kind, ibis_doc_model::ShapeKind::Chart(_)))
                .map(|s| {
                    let f = s.frame;
                    let c = cells.as_ref().map_or([0; 4], |(o, xs, ys)| {
                        [index(xs, o.x, f.x), index(ys, o.y, f.y), index(xs, o.x, f.x + f.width - 1), index(ys, o.y, f.y + f.height - 1)]
                    });
                    (f, c)
                })
                .collect()
        })
        .unwrap_or_default();
    let window = cx
        .open_window(window_size, |_, app| {
            app.new(|_| SlidePicture { slide, scale: px_per_emu, size: window_size, origin: (0.0, 0.0), inset: (0.0, 0.0), view: window_size, resources, marks: Vec::new(), extra: Vec::new(), rules: Vec::new() })
        })
        .map_err(|e| format!("Cannot create the window for drawing: {e}"))?;
    cx.update_window(window.into(), |_, window, _| {
                        window.record_paints();
                        // Only recorded: the PDF is made of the record.
                        window.record_paints_only(true);
                    })
                    .map_err(|e| e.to_string())?;
    let page_pt = (paper_w as f32 / 12_700.0, paper_h as f32 / 12_700.0);
    let (mut drawing, mut adding) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    for (i, page) in pages.iter().enumerate() {
        let page_started = crate::clock::Instant::now();
        // The title rows over a page past them, the title columns left of one.
        let title_rows = titles.0.filter(|(_, end)| page.y >= *end);
        let title_columns = titles.1.filter(|(_, end)| page.x >= *end);
        let (tw, th) = (title_columns.map_or(0, |(a, b)| b - a), title_rows.map_or(0, |(a, b)| b - a));
        let view = (page.width as f64 * scale, page.height as f64 * scale);
        let whole = ((page.width + tw) as f64 * scale, (page.height + th) as f64 * scale);
        let room = ((paper_w - left - right) as f64, (paper_h - top - bottom) as f64);
        let corner_at = (
            left as f64 + if print.centered.0 { ((room.0 - whole.0) / 2.0).max(0.0) } else { 0.0 },
            top as f64 + if print.centered.1 { ((room.1 - whole.1) / 2.0).max(0.0) } else { 0.0 },
        );
        // Printed headings: the cells move right of the row numbers and
        // under the column letters.
        let (hw, hh) = if print.headings { (headings.0 * 12_700.0 * scale, headings.1 * 12_700.0 * scale) } else { (0.0, 0.0) };
        let corner_at = (corner_at.0 + hw, corner_at.1 + hh);
        let inset = (corner_at.0 + tw as f64 * scale, corner_at.1 + th as f64 * scale);
        let p = |v: f64| (v * PX_PER_EMU) as f32;
        let mut extra = Vec::new();
        if let Some((y0, _)) = title_rows {
            extra.push(((page.x as f32 * px_per_emu, y0 as f32 * px_per_emu), (p(inset.0), p(corner_at.1)), gpui_kit::size(px(p(view.0)), px(p(th as f64 * scale)))));
        }
        if let Some((x0, _)) = title_columns {
            extra.push(((x0 as f32 * px_per_emu, page.y as f32 * px_per_emu), (p(corner_at.0), p(inset.1)), gpui_kit::size(px(p(tw as f64 * scale)), px(p(view.1)))));
        }
        if let (Some((x0, _)), Some((y0, _))) = (title_columns, title_rows) {
            extra.push(((x0 as f32 * px_per_emu, y0 as f32 * px_per_emu), (p(corner_at.0), p(corner_at.1)), gpui_kit::size(px(p(tw as f64 * scale)), px(p(th as f64 * scale)))));
        }
        cx.update(|app| {
            window.update(app, |picture, _, cx| {
                picture.extra = extra;
                picture.origin = (page.x as f32 * px_per_emu, page.y as f32 * px_per_emu);
                picture.inset = ((inset.0 * PX_PER_EMU) as f32, (inset.1 * PX_PER_EMU) as f32);
                picture.view = gpui_kit::size(px((view.0 * PX_PER_EMU) as f32), px((view.1 * PX_PER_EMU) as f32));
                picture.marks = crate::xlsx::header_footer::page_marks(print, default_style, scale as f32, i as u32 + 1, pages.len() as u32, &sheet_name, file_name, &picture.resources);
                picture.rules.clear();
                if print.headings && let Some(sheet) = picture.slide.shapes.iter().find_map(|s| match &s.kind {
                    ibis_doc_model::ShapeKind::Sheet(sheet) => Some((s.frame, sheet.clone())),
                    _ => None,
                }) {
                    let (frame, sheet) = sheet;
                    // The columns and rows shown, title ones first (EMU spans of the slide).
                    let spans = |offsets: Vec<i64>, start: i64, title: Option<(i64, i64)>, from: i64, to: i64| -> Vec<(u32, i64, i64)> {
                        let mut out = Vec::new();
                        for (lo, hi) in title.into_iter().chain(std::iter::once((from, to))) {
                            for (k, w) in offsets.windows(2).enumerate() {
                                let (a, b) = (start + w[0], start + w[1]);
                                if b > lo && a < hi && b > a {
                                    out.push((k as u32, a.max(lo), b.min(hi)));
                                }
                            }
                        }
                        out
                    };
                    let cols = spans(sheet.column_offsets(), frame.x, title_columns, page.x, page.x + page.width);
                    let rows = spans(sheet.row_offsets(), frame.y, title_rows, page.y, page.y + page.height);
                    let (x0, y0) = (p(corner_at.0 - hw), p(corner_at.1 - hh));
                    let (hwp, hhp) = (p(hw), p(hh));
                    let width: f32 = cols.iter().map(|c| (c.2 - c.1) as f32 * px_per_emu).sum();
                    let height: f32 = rows.iter().map(|r| (r.2 - r.1) as f32 * px_per_emu).sum();
                    let (right, bottom) = (x0 + hwp + width, y0 + hhp + height);
                    // The frame: round it all, and between the headings and the cells.
                    picture.rules.extend([
                        (x0 - 1.0, y0 - 1.0, right - x0 + 1.0, 1.0),
                        (x0 - 1.0, y0 - 1.0, 1.0, bottom - y0 + 1.0),
                        (x0 - 1.0, bottom - 1.0, right - x0 + 1.0, 1.0),
                        (right - 1.0, y0 - 1.0, 1.0, bottom - y0 + 1.0),
                        (x0 + hwp - 1.0, y0 + hhp - 1.0, 1.0, bottom - y0 - hhp + 1.0),
                        (x0 + hwp - 1.0, y0 + hhp - 1.0, right - x0 - hwp + 1.0, 1.0),
                    ]);
                    let labels = crate::xlsx::header_footer::heading_marks(&cols, &rows, (x0, y0), (hwp, hhp), px_per_emu, default_style, scale as f32, &picture.resources);
                    picture.marks.extend(labels);
                }
                cx.notify();
            })
        })
        .map_err(|e| e.to_string())?;
        // Changing the view draws the page; the first waits for the
        // pictures GPUI loads itself.
        for _ in 0..if i == 0 { 3 } else { 0 } {
            cx.run_until_parked();
            cx.update_window(window.into(), |_, window, _| window.refresh()).map_err(|e| e.to_string())?;
            cx.run_until_parked();
        }
        let records = cx.update_window(window.into(), |_, window, _| window.take_recorded_paints()).map_err(|e| e.to_string())?.unwrap_or_default();
        // Recording goes on for the next page.
        cx.update_window(window.into(), |_, window, _| {
                        window.record_paints();
                        // Only recorded: the PDF is made of the record.
                        window.record_paints_only(true);
                    })
                    .map_err(|e| e.to_string())?;
        drawing += page_started.elapsed();
        let adding_started = crate::clock::Instant::now();
        let text_system = cx.text_system().clone();
        // Excel puts the sheet's corner on a whole point of the sheet at
        // or inside the margin rounded up to a point (29 pt of a 0.4 inch
        // margin: at 87 % 33 points of the sheet, 28.71 pt of paper; its
        // PDFs of xlsx-XALN and XBRD at 73 to 100 %); the window put it on
        // a whole pixel. Not when centred (not measured).
        let corner = |margin: i64, centred: bool, at: f64| -> f32 {
            if centred {
                return 0.0;
            }
            let want = ((margin as f64 / 12_700.0 - 1e-6).ceil() / scale + 1e-6).floor() * scale;
            (want - at.round()) as f32
        };
        let shift = (corner(left, print.centered.0, (corner_at.0 - hw) * PX_PER_EMU), corner(top, print.centered.1, (corner_at.1 - hh) * PX_PER_EMU));
        if let Some(path) = std::env::var_os("IBIS_CHART_RECTS") {
            use std::io::Write as _;
            // `sheet  page  x y width height` of each chart on this page, in
            // points from the page's top left.
            let mut text = String::new();
            for (frame, cells) in &chart_frames {
                let intersects = frame.x < page.x + page.width && frame.x + frame.width > page.x && frame.y < page.y + page.height && frame.y + frame.height > page.y;
                if !intersects {
                    continue;
                }
                let to_pt = |v: i64| v as f64 * scale / 12_700.0;
                let x = inset.0 / 12_700.0 + to_pt(frame.x - page.x) + shift.0 as f64;
                let y = inset.1 / 12_700.0 + to_pt(frame.y - page.y) + shift.1 as f64;
                text.push_str(&format!("{sheet_name}\t{}\t{x:.2}\t{y:.2}\t{:.2}\t{:.2}\t{}\t{}\t{}\t{}\n", writer.page_count() + 1, to_pt(frame.width), to_pt(frame.height), cells[0], cells[1], cells[2], cells[3]));
            }
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                file.write_all(text.as_bytes()).ok();
            }
        }
        writer.add_page_shifted(records, f32::from(window_size.width), page_pt, density, shift, &mut |id| text_system.font_data(id));
        adding += adding_started.elapsed();
    }
    if std::env::var_os("IBIS_DEBUG_TIMING").is_some() {
        eprintln!("export: sheet {sheet_name:?} {} pages: draw {:.0} ms, add to PDF {:.0} ms", pages.len(), drawing.as_secs_f64() * 1000.0, adding.as_secs_f64() * 1000.0);
    }
    cx.update_window(window.into(), |_, window, _| window.remove_window()).ok();
    Ok(())
}

/// What each element of `slide` is and where it is drawn, back to front, in
/// pixels of the exported picture.
///
/// Columns: `z layer kind x y width height detail`. `kind` is one of `text`,
/// `picture`, `picture-rotated`, `picture-undrawable`, `chart`,
/// `chart-unread`, `table`, `shape`, `preset` (a preset geometry drawn as its
/// bounding box), `line`.
fn layout_tsv(slide: &Slide, px_per_emu: f32, origin: (i64, i64)) -> String {
    use ibis_doc_model::{Geometry, Placeholder, ShapeKind};
    let mut out = String::from("z\tlayer\tkind\tx\ty\twidth\theight\tdetail\n");
    let shapes = slide.shown_backdrop().iter().map(|s| ("backdrop", s)).chain(slide.shown_shapes().map(|s| ("slide", s)));
    for (z, (layer, shape)) in shapes.enumerate() {
        let has_text = shape.text_body().is_some_and(|b| !b.is_empty());
        let (kind, detail) = match &shape.kind {
            _ if has_text => ("text", shape.text_body().map(|b| b.text().chars().take(40).collect::<String>()).unwrap_or_default()),
            ShapeKind::TextBox(_) => ("text", String::new()),
            ShapeKind::Auto(a) => match &a.geometry {
                _ if matches!(a.fill, ibis_doc_model::Fill::Image(_) | ibis_doc_model::Fill::TileImage { .. }) => ("picture", "図形の画像塗り".into()), // i18n-ok（layout TSV の書き出し。試験・確認用のファイルに入り、画面には出ない）
                Geometry::Formula(g) => ("shape", g.preset.clone().unwrap_or_else(|| "custom".into())),
                _ => ("shape", String::new()),
            },
            ShapeKind::Picture(_) if shape.rotation.rem_euclid(21_600_000) != 0 => {
                ("picture", format!("回転 {:.0}°", shape.rotation as f32 / 60_000.0)) // i18n-ok（layout TSV の書き出し。試験・確認用のファイルに入り、画面には出ない）
            }
            ShapeKind::Picture(_) => ("picture", String::new()),
            ShapeKind::Line(_) => ("line", String::new()),
            ShapeKind::Chart(_) => ("chart", String::new()),
            ShapeKind::Table(_) => ("table", String::new()),
            ShapeKind::Sheet(_) => ("sheet", String::new()),
            ShapeKind::Page { .. } => ("page", String::new()),
            ShapeKind::Slicer(s) => ("slicer", s.name.clone()),
            ShapeKind::Placeholder(Placeholder::Table) => ("table", String::new()),
            ShapeKind::Placeholder(Placeholder::Chart) => ("chart-unread", String::new()),
            ShapeKind::Placeholder(Placeholder::Diagram) => ("smartart", String::new()),
            ShapeKind::Placeholder(Placeholder::Picture(format)) => ("picture-undrawable", format!("{format:?}")),
        };
        let f = shape.frame;
        let px = |v: i64| (v as f32 * px_per_emu).round() as i64;
        let detail = detail.replace(['\t', '\n'], " ");
        out.push_str(&format!("{z}\t{layer}\t{kind}\t{}\t{}\t{}\t{}\t{detail}\n", px(f.x - origin.0), px(f.y - origin.1), px(f.width), px(f.height)));
    }
    out
}

/// Device pixels per logical pixel of the headless renderer: a headless
/// window's scale factor, which its pictures are drawn at.
fn pixel_density(cx: &mut HeadlessAppContext) -> Result<f32, String> {
    struct Blank;
    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }
    let probe = gpui_kit::size(px(100.), px(100.));
    let window = cx.open_window(probe, |_, app| app.new(|_| Blank)).map_err(|e| e.to_string())?;
    let density = cx.update_window(window.into(), |_, window, _| window.scale_factor()).map_err(|e| e.to_string())?;
    cx.update_window(window.into(), |_, window, _| window.remove_window()).ok();
    Ok(density.max(0.25))
}

/// A PDF with one full-page picture per page, each page's size (in points)
/// given with its picture.
fn write_pdf(path: &Path, pages: &[(image::RgbaImage, (f32, f32))]) -> Result<(), String> {
    let mut pdf: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    // Objects: 1 catalog, 2 page tree, then per page: page, contents, image.
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| 3 + i * 3).collect();
    let mut object = |pdf: &mut Vec<u8>, id: usize, body: &[u8]| {
        if offsets.len() < id {
            offsets.resize(id, 0);
        }
        offsets[id - 1] = pdf.len();
        pdf.extend(format!("{id} 0 obj\n").as_bytes());
        pdf.extend(body);
        pdf.extend(b"\nendobj\n");
    };
    object(&mut pdf, 1, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: String = page_ids.iter().map(|id| format!("{id} 0 R ")).collect();
    object(&mut pdf, 2, format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", pages.len()).as_bytes());
    for ((page, (pw, ph)), id) in pages.iter().zip(&page_ids) {
        let (contents, picture) = (id + 1, id + 2);
        object(
            &mut pdf,
            *id,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im0 {picture} 0 R >> >> /Contents {contents} 0 R >>"
            )
            .as_bytes(),
        );
        let draw = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /Im0 Do Q");
        object(&mut pdf, contents, format!("<< /Length {} >>\nstream\n{draw}\nendstream", draw.len()).as_bytes());
        let rgb: Vec<u8> = page.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect();
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6);
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
            page.width(),
            page.height(),
            data.len()
        )
        .into_bytes();
        body.extend(&data);
        body.extend(b"\nendstream");
        object(&mut pdf, picture, &body);
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for offset in &offsets {
        pdf.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes());
    std::fs::write(path, pdf).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: GPUI's `test` attribute would shadow the standard one.
    use super::{page_spans, parse_slides};

    #[test]
    fn slide_lists() {
        assert_eq!(parse_slides("1,3,5-7").unwrap(), vec![1, 3, 5, 6, 7]);
        assert!(parse_slides("0").is_err());
        assert!(parse_slides("4-2").is_err());
        assert!(parse_slides("x").is_err());
    }

    #[test]
    fn pages_hold_whole_cells() {
        let edges = [0, 40, 80, 120, 300, 340];
        // As many whole cells as fit; a cell larger than the page is cut.
        assert_eq!(page_spans(&edges, &[], 0, 340, 100.0, None), vec![(0, 80), (80, 120), (120, 220), (220, 300), (300, 340)]);
        // A manual break starts a page.
        assert_eq!(page_spans(&edges, &[40], 0, 120, 100.0, None), vec![(0, 40), (40, 120)]);
        assert_eq!(page_spans(&edges, &[], 0, 120, 500.0, None), vec![(0, 120)]);
        // Title rows 0..20: the pages past them hold 20 less.
        let rows: Vec<i64> = (0..=20).map(|i| i * 20).collect();
        assert_eq!(page_spans(&rows, &[], 0, 400, 100.0, Some((20, 20))), vec![(0, 100), (100, 180), (180, 260), (260, 340), (340, 400)]);
    }
}
