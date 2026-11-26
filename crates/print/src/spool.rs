//! System print spooler supplied by craft-print. PDF composition and permission checks
//! stay in PdfCraft; these adapters retain the application's print-error surface.

pub use craft_print::{
    Duplex, HANDLED_OPTIONS, HAS_PRINTER_PREFERENCES, Job, Printer, PrinterOption, Scaling, lp_args, lpstat_command, parse_lpoptions, parse_lpstat,
    parse_lpstat_e, parse_ppd, parse_windows_printers, ppd_text, printers, printers_parsed,
};

/// The driver's job options for a printer (CUPS: its PPD, with the queue's current defaults);
/// empty when it has none or on other platforms. Reads files and runs `lpoptions`, so call it
/// when the user asks, not every frame.
pub fn printer_options(printer: &str) -> Vec<PrinterOption> {
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    {
        printer_options_via(printer, std::path::Path::new("/etc/cups/ppd"), std::process::Command::new("lpoptions"))
    }
    #[cfg(not(all(unix, not(target_arch = "wasm32"))))]
    {
        craft_print::printer_options(printer)
    }
}

/// Read a destination's options with the PPD directory and command supplied, so tests need
/// neither a system queue nor a physical printer. CUPS instances share their queue's PPD but
/// have their own defaults, queried with the complete `queue/instance` destination.
#[cfg(all(unix, not(target_arch = "wasm32")))]
pub(crate) fn printer_options_via(printer: &str, ppd_dir: &std::path::Path, mut lpoptions: std::process::Command) -> Vec<PrinterOption> {
    let (queue, instance) = printer.split_once('/').map_or((printer, None), |(q, i)| (q, Some(i)));
    // Retain the conservative name checks, applied to each component. Neither component is
    // a path: reject dot components, backslashes and any additional separator before I/O.
    let valid = |name: &str| {
        !name.is_empty()
            && name.len() <= 64
            && !matches!(name, "." | "..")
            && name.bytes().all(|b| b.is_ascii_graphic() && !matches!(b, b'/' | b'\\' | b':' | b'=' | b'"'))
    };
    if !valid(queue) || instance.is_some_and(|name| !valid(name)) {
        return Vec::new();
    }
    const PPD_CAP: u64 = 16 << 20;
    let ppd = std::fs::File::open(ppd_dir.join(format!("{queue}.ppd"))).ok().and_then(|f| {
        use std::io::Read as _;
        let mut bytes = Vec::new();
        f.take(PPD_CAP).read_to_end(&mut bytes).ok().map(|_| bytes)
    });
    let mut options = ppd.map(|b| parse_ppd(&ppd_text(&b))).unwrap_or_default();
    let current = lpoptions.args(["-p", printer, "-l"]).output().map(|o| parse_lpoptions(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default();
    for (key, value) in current {
        if let Some(o) = options.iter_mut().find(|o| o.key == key)
            && o.choices.iter().any(|(c, _)| *c == value)
        {
            o.default = value;
        }
    }
    options
}

pub fn submit(pdf: &[u8], job: &Job) -> Result<String, crate::PrintError> {
    submit_with_progress(pdf, job, &mut |_, _| {})
}

/// [`submit`], told `(sent, sheets)` as each sheet reaches the spooler (Windows; CUPS takes the
/// whole PDF at once and reports nothing).
///
/// KnuPdf: on Windows every sheet is drawn by our own renderer, as on screen, at the printer's
/// resolution (300 to 600 dpi) and sent to the driver as pixels a band at a time
/// ([`craft_print::submit_rendered`]). Windows' own PDF path rasterized through Direct2D at one
/// resolution for the whole page: soft text at 300 dpi, and at 600 dpi an office PCL 6 printer
/// closed the program.
pub fn submit_with_progress(pdf: &[u8], job: &Job, progress: &mut dyn FnMut(usize, usize)) -> Result<String, crate::PrintError> {
    craft_print::submit_rendered(pdf, job, progress, &rasterize).map_err(|e| crate::PrintError::Spool(e.to_string()))
}

/// The [`craft_print::Rasterizer`]: the print-ready PDF parsed once, its sheets drawn in bands.
pub(crate) fn rasterize(pdf: Vec<u8>) -> Result<Box<dyn craft_print::SheetPainter>, String> {
    let renderer = pdfcraft_render::PageRenderer::new(std::sync::Arc::new(pdf), pdfcraft_render::RenderConfig::default());
    if renderer.page_count() == 0 {
        return Err("the print-ready PDF could not be read".into());
    }
    Ok(Box::new(Painter(renderer)))
}

struct Painter(pdfcraft_render::PageRenderer);

impl craft_print::SheetPainter for Painter {
    fn band(&mut self, sheet: usize, scale: f64, x: u32, y: u32, w: u32, h: u32) -> Result<Vec<u8>, String> {
        let page = self.0.render(pdfcraft_render::RenderRequest {
            page: sheet,
            tile: Some(pdfcraft_render::Tile { x, y, w, h }),
            scale: scale as f32,
            // The document's true colours, whatever the reading colours on screen.
            tag: pdfcraft_render::PRINT_TAG,
            ..Default::default()
        });
        if let Some(e) = page.error {
            return Err(e);
        }
        if (page.width, page.height) != (w, h) {
            return Err(format!("the renderer drew {}×{} pixels for a {w}×{h} band", page.width, page.height));
        }
        Ok(page.rgba.as_ref().to_vec())
    }
}

pub fn open_printer_preferences(printer: &str) -> Result<(), crate::PrintError> {
    craft_print::open_printer_preferences(printer).map_err(|e| crate::PrintError::Spool(e.to_string()))
}
