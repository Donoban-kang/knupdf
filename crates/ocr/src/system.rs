//! KnuPdf: the operating system's own text recognition, for scripts the bundled models do not
//! read. On Windows 10 and 11 this is Windows.Media.Ocr, which reads Korean when Windows has the
//! Korean optical character recognition feature (Korean Windows has it; otherwise it is added in
//! Settings ▸ Time & language ▸ Language). Recognition runs on this computer: nothing is sent
//! anywhere. Elsewhere nothing is available and callers keep the page as a picture.

use crate::{Line, OcrError};

/// Whether this computer can read `language` (a BCP-47 tag such as `ko`).
pub fn available(language: &str) -> bool {
    #[cfg(windows)]
    {
        windows_ocr::available(language)
    }
    #[cfg(not(windows))]
    {
        let _ = language;
        false
    }
}

/// The lines of `language` text in an RGBA image, `width` × `height` pixels, word boxes in its
/// pixels. [`OcrError::SystemUnavailable`] when [`available`] is false.
pub fn recognize(rgba: &[u8], width: u32, height: u32, language: &str) -> Result<Vec<Line>, OcrError> {
    if width == 0 || height == 0 || (rgba.len() as u64) < u64::from(width) * u64::from(height) * 4 {
        return Err(OcrError::EmptyImage);
    }
    #[cfg(windows)]
    {
        windows_ocr::recognize(rgba, width, height, language)
    }
    #[cfg(not(windows))]
    {
        Err(OcrError::SystemUnavailable(language.to_owned()))
    }
}

/// `rgba` (`width` × `height`) averaged over `k` × `k` blocks, as BGRA: the image Windows reads
/// when the page is larger than its recogniser takes. `k` is at least 1.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn shrink_to_bgra(rgba: &[u8], width: u32, height: u32, k: u32) -> (Vec<u8>, u32, u32) {
    let k = k.max(1);
    let (w, h) = (width.div_ceil(k).max(1), height.div_ceil(k).max(1));
    let mut out = Vec::with_capacity((w as usize).saturating_mul(h as usize).saturating_mul(4));
    for by in 0..h {
        for bx in 0..w {
            let (mut sum, mut n) = ([0u32; 3], 0u32);
            for y in (by * k)..((by + 1) * k).min(height) {
                for x in (bx * k)..((bx + 1) * k).min(width) {
                    let i = (y as usize * width as usize + x as usize) * 4;
                    if let Some(p) = rgba.get(i..i + 3) {
                        sum[0] += u32::from(p[0]);
                        sum[1] += u32::from(p[1]);
                        sum[2] += u32::from(p[2]);
                        n += 1;
                    }
                }
            }
            let n = n.max(1);
            out.extend_from_slice(&[(sum[2] / n) as u8, (sum[1] / n) as u8, (sum[0] / n) as u8, 255]);
        }
    }
    (out, w, h)
}

#[cfg(windows)]
mod windows_ocr {
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::core::HSTRING;

    use crate::{Line, OcrError, Word};

    fn engine(language: &str) -> windows::core::Result<Option<OcrEngine>> {
        let language = Language::CreateLanguage(&HSTRING::from(language))?;
        if !OcrEngine::IsLanguageSupported(&language)? {
            return Ok(None);
        }
        OcrEngine::TryCreateFromLanguage(&language).map(Some)
    }

    /// Runs `f` on a thread of its own, so WinRT gets an apartment of its own whoever calls (the
    /// UI thread is single-threaded); a panic there is an error, not a crash.
    fn on_own_thread<T: Send>(f: impl FnOnce() -> T + Send) -> Option<T> {
        std::thread::scope(|s| s.spawn(f).join().ok())
    }

    pub(super) fn available(language: &str) -> bool {
        on_own_thread(|| engine(language).ok().flatten().is_some()).unwrap_or(false)
    }

    pub(super) fn recognize(rgba: &[u8], width: u32, height: u32, language: &str) -> Result<Vec<Line>, OcrError> {
        on_own_thread(|| read(rgba, width, height, language)).unwrap_or_else(|| Err(OcrError::Recognize("Windows text recognition stopped".into())))
    }

    fn read(rgba: &[u8], width: u32, height: u32, language: &str) -> Result<Vec<Line>, OcrError> {
        let fail = |e: windows::core::Error| OcrError::Recognize(e.message().to_string());
        let engine = engine(language).map_err(fail)?.ok_or_else(|| OcrError::SystemUnavailable(language.to_owned()))?;
        // Windows reads images up to a size; a larger page is averaged down to fit.
        let max = OcrEngine::MaxImageDimension().map_err(fail)?.max(1);
        let k = width.max(height).div_ceil(max).max(1);
        let (bgra, w, h) = super::shrink_to_bgra(rgba, width, height, k);
        let (wi, hi) =
            (i32::try_from(w).map_err(|e| OcrError::Recognize(e.to_string()))?, i32::try_from(h).map_err(|e| OcrError::Recognize(e.to_string()))?);
        let writer = DataWriter::new().map_err(fail)?;
        writer.WriteBytes(&bgra).map_err(fail)?;
        let buffer = writer.DetachBuffer().map_err(fail)?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, wi, hi).map_err(fail)?;
        let result = engine.RecognizeAsync(&bitmap).map_err(fail)?.join().map_err(fail)?;
        let lines = result.Lines().map_err(fail)?;
        let scale = k as f32;
        let mut out = Vec::new();
        for i in 0..lines.Size().map_err(fail)? {
            let words = lines.GetAt(i).and_then(|l| l.Words()).map_err(fail)?;
            let mut line = Line::default();
            for j in 0..words.Size().map_err(fail)? {
                let word = words.GetAt(j).map_err(fail)?;
                let text = word.Text().map_err(fail)?.to_string_lossy();
                let r = word.BoundingRect().map_err(fail)?;
                if !text.trim().is_empty() {
                    line.words.push(Word { text, rect: [r.X * scale, r.Y * scale, (r.X + r.Width) * scale, (r.Y + r.Height) * scale] });
                }
            }
            if !line.words.is_empty() {
                out.push(line);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_averages_blocks_into_bgra() {
        // 2 × 2 RGBA: red, green / blue, white; one 2 × 2 block averages them.
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255];
        let (bgra, w, h) = shrink_to_bgra(&rgba, 2, 2, 2);
        assert_eq!((w, h), (1, 1));
        assert_eq!(bgra, vec![127, 127, 127, 255]);
        // k = 1 only swaps red and blue; an odd edge keeps its partial block.
        let (bgra, w, h) = shrink_to_bgra(&rgba, 2, 2, 1);
        assert_eq!((w, h, &bgra[..4]), (2, 2, &[0u8, 0, 255, 255][..]));
        let (_, w, h) = shrink_to_bgra(&[0; 3 * 3 * 4], 3, 3, 2);
        assert_eq!((w, h), (2, 2));
    }

    #[test]
    fn short_or_empty_images_are_refused_and_other_systems_have_nothing() {
        assert!(matches!(recognize(&[], 0, 0, "ko"), Err(OcrError::EmptyImage)));
        assert!(matches!(recognize(&[0; 8], 2, 2, "ko"), Err(OcrError::EmptyImage)));
        if !cfg!(windows) {
            assert!(!available("ko"));
            assert!(matches!(recognize(&[255; 16], 2, 2, "ko"), Err(OcrError::SystemUnavailable(_))));
        }
    }
}
