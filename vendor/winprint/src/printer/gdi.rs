//! PdfCraft patch (KnuPdf): a print job through GDI, for pixels the application draws itself.
//!
//! The XPS path (`DxgiPrintContext`) hands a page to Direct2D, which rasterizes what it cannot
//! print as vectors at one resolution for the whole page, in this process; on an office PCL 6
//! driver text came out soft at 300 dpi and the program closed at 600. Here the application
//! sends bitmaps of its own, a band at a time, to a printer device context created from the
//! job's PrintTicket (paper, orientation, copies, two-sided and colour travel in its DEVMODE),
//! as every GDI program prints. Nothing is rasterized by Direct2D or interpreted by
//! Windows.Data.Pdf.

use crate::printer::PrinterDevice;
use crate::ticket::{PrintTicket, ToDevModeError};
use crate::utils::wchar;
use std::ffi::OsStr;
use thiserror::Error;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, GetDeviceCaps, SetStretchBltMode, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DEVMODEW, DIB_RGB_COLORS, HALFTONE,
    HDC, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY,
};
use windows::Win32::Storage::Xps::{AbortDoc, EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};

#[derive(Error, Debug)]
pub enum GdiPrintError {
    #[error("the PrintTicket could not be converted for the driver")]
    PrintTicket(#[source] ToDevModeError),
    #[error("the printer could not be opened for drawing")]
    CreateDc,
    #[error("the spooler did not start the job")]
    StartDoc,
    #[error("the printer did not start a page")]
    StartPage,
    #[error("the printer did not take a page")]
    EndPage,
    #[error("the printer did not take a picture of the page")]
    Draw,
    #[error("the spooler did not finish the job")]
    EndDoc,
    #[error("a picture of the page was malformed")]
    Bitmap,
}

/// The paper as the device context sees it, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GdiPage {
    pub dpi_x: i32,
    pub dpi_y: i32,
    pub width: i32,
    pub height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
}

/// An open GDI print job. Dropped unfinished, it is cancelled (`AbortDoc`), so a failure never
/// leaves half a job in the queue.
pub struct GdiPrintJob {
    hdc: HDC,
    in_page: bool,
    finished: bool,
}

impl GdiPrintJob {
    /// Open `device` with `ticket`'s settings and start a job called `title`.
    pub fn start(device: &PrinterDevice, ticket: &PrintTicket, title: &OsStr) -> Result<Self, GdiPrintError> {
        let dev_mode = ticket.to_dev_mode(device).map_err(GdiPrintError::PrintTicket)?;
        // The DEVMODE bytes, copied to memory aligned for DEVMODEW (a Vec<u8> need not be).
        let words = dev_mode.len().div_ceil(8).max(std::mem::size_of::<DEVMODEW>().div_ceil(8));
        let mut aligned = vec![0u64; words];
        for (i, b) in dev_mode.iter().enumerate() {
            let (w, s) = (i / 8, (i % 8) * 8);
            if let Some(word) = aligned.get_mut(w) {
                *word |= u64::from(*b) << s;
            }
        }
        let name = wchar::to_wide_chars(device.os_name());
        let title = wchar::to_wide_chars(title);
        // SAFETY: `name` is a NUL-terminated wide string that outlives the call, and `aligned`
        // holds a whole DEVMODEW (at least its size, plus the driver's private part).
        let hdc = unsafe { CreateDCW(PCWSTR::null(), PCWSTR(name.as_ptr()), PCWSTR::null(), Some(aligned.as_ptr() as *const DEVMODEW)) };
        if hdc.is_invalid() {
            return Err(GdiPrintError::CreateDc);
        }
        let mut job = Self { hdc, in_page: false, finished: true };
        let info = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: PCWSTR(title.as_ptr()), ..Default::default() };
        // SAFETY: a valid printer DC and a DOCINFOW whose strings outlive the call.
        if unsafe { StartDocW(job.hdc, &info) } <= 0 {
            return Err(GdiPrintError::StartDoc);
        }
        // SAFETY: a valid DC. Stretched bitmaps (a printer whose two resolutions differ) are
        // averaged rather than having rows dropped.
        unsafe { SetStretchBltMode(job.hdc, HALFTONE) };
        job.finished = false;
        Ok(job)
    }

    /// The paper of the page about to be drawn.
    pub fn page(&self) -> GdiPage {
        // SAFETY: a valid DC; GetDeviceCaps only reads.
        let cap = |i| unsafe { GetDeviceCaps(Some(self.hdc), i) };
        GdiPage {
            dpi_x: cap(LOGPIXELSX),
            dpi_y: cap(LOGPIXELSY),
            width: cap(PHYSICALWIDTH),
            height: cap(PHYSICALHEIGHT),
            offset_x: cap(PHYSICALOFFSETX),
            offset_y: cap(PHYSICALOFFSETY),
        }
    }

    pub fn start_page(&mut self) -> Result<(), GdiPrintError> {
        // SAFETY: a valid DC with a started document.
        if unsafe { StartPage(self.hdc) } <= 0 {
            return Err(GdiPrintError::StartPage);
        }
        self.in_page = true;
        Ok(())
    }

    /// Draw a `width` × `height` bitmap (rows top to bottom, blue-green-red, each row padded to
    /// four bytes) into the device rectangle at (`x`, `y`), `dest_w` × `dest_h` device pixels,
    /// relative to the printable area's origin.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_bgr(&mut self, x: i32, y: i32, dest_w: i32, dest_h: i32, width: u32, height: u32, bgr: &[u8]) -> Result<(), GdiPrintError> {
        let stride = (width as usize).checked_mul(3).and_then(|v| v.checked_add(3)).map(|v| v & !3).ok_or(GdiPrintError::Bitmap)?;
        let needed = stride.checked_mul(height as usize).ok_or(GdiPrintError::Bitmap)?;
        let (Ok(w), Ok(h)) = (i32::try_from(width), i32::try_from(height)) else {
            return Err(GdiPrintError::Bitmap);
        };
        if !self.in_page || w == 0 || h == 0 || bgr.len() < needed || dest_w <= 0 || dest_h <= 0 {
            return Err(GdiPrintError::Bitmap);
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                // Negative: rows run top to bottom.
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: a valid DC inside a page; `bgr` holds `height` rows of `stride` bytes, as the
        // header describes, and outlives the call.
        let lines = unsafe { StretchDIBits(self.hdc, x, y, dest_w, dest_h, 0, 0, w, h, Some(bgr.as_ptr().cast()), &info, DIB_RGB_COLORS, SRCCOPY) };
        if lines == 0 {
            return Err(GdiPrintError::Draw);
        }
        Ok(())
    }

    pub fn end_page(&mut self) -> Result<(), GdiPrintError> {
        self.in_page = false;
        // SAFETY: a valid DC inside a page.
        if unsafe { EndPage(self.hdc) } <= 0 {
            return Err(GdiPrintError::EndPage);
        }
        Ok(())
    }

    /// Finish the job: the spooler has it once this returns.
    pub fn finish(mut self) -> Result<(), GdiPrintError> {
        if self.in_page {
            self.end_page()?;
        }
        self.finished = true;
        // SAFETY: a valid DC with a started document.
        if unsafe { EndDoc(self.hdc) } <= 0 {
            return Err(GdiPrintError::EndDoc);
        }
        Ok(())
    }
}

impl Drop for GdiPrintJob {
    fn drop(&mut self) {
        // SAFETY: the DC is valid until deleted here, once.
        unsafe {
            if !self.finished {
                AbortDoc(self.hdc);
            }
            let _ = DeleteDC(self.hdc);
        }
    }
}
