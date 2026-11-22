//! Windows: the flyout's frosted glass. Just before the flyout opens, the strip of screen it will cover
//! is copied, shrunk and blurred, and the page draws it under its translucent surface.
//!
//! The window effects Windows offers don't give that look reliably: Acrylic is nearly opaque, the
//! older acrylic accent makes the window stutter when it resizes, and all of them turn solid when
//! Windows draws no transparency (with transparency effects off, on battery saver, or through a remote
//! desktop). A snapshot is the same on every machine and costs nothing while the flyout is open.
//! Because the flyout closes as soon as it loses focus, a snapshot taken as it opens stays true.

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CAPTUREBLT, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HALFTONE, ReleaseDC, SRCCOPY,
    SelectObject, SetBrushOrgEx, SetStretchBltMode, StretchBlt,
};

use crate::positioning::Rect;

/// One sample covers this many logical pixels; the blur hides the coarseness.
const SAMPLE: f64 = 4.0;
/// Box blur radius in samples. Three passes approximate a Gaussian with a standard deviation of about
/// 7.5 samples (30 logical pixels), close to the mockup's `blur(28px)`.
const BLUR_RADIUS: usize = 7;
const BLUR_PASSES: usize = 3;

/// A blurred copy of part of the screen, in RGBA, one sample per `SAMPLE` logical pixels.
pub struct Snapshot {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Copies `area` (physical pixels) of the screen, shrunk for a monitor at `scale`, and blurs it.
pub fn capture(area: Rect, scale: f64) -> Option<Snapshot> {
    if area.w <= 0 || area.h <= 0 {
        return None;
    }
    let step = (SAMPLE * scale).max(1.0);
    let width = (f64::from(area.w) / step).ceil().max(1.0) as usize;
    let height = (f64::from(area.h) / step).ceil().max(1.0) as usize;
    let mut bgra = vec![0u8; width * height * 4];

    // SAFETY: plain GDI calls on DCs and a bitmap created here and released before returning; the
    // buffer is sized for `height` top-down rows of 32-bit pixels, which is what GetDIBits writes.
    let copied = unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width as i32, height as i32);
        let previous = SelectObject(memory, bitmap.into());
        SetStretchBltMode(memory, HALFTONE);
        let _ = SetBrushOrgEx(memory, 0, 0, None);
        let ok = StretchBlt(
            memory,
            0,
            0,
            width as i32,
            height as i32,
            Some(screen),
            area.x,
            area.y,
            area.w,
            area.h,
            SRCCOPY | CAPTUREBLT,
        )
        .as_bool();
        SelectObject(memory, previous);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // negative: rows top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let rows = GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            Some(bgra.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        ok && rows == height as i32
    };
    if !copied {
        return None;
    }

    // BGRA (alpha undefined) to RGBA, opaque.
    let mut rgba = bgra;
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    for _ in 0..BLUR_PASSES {
        box_blur(&mut rgba, width, height, BLUR_RADIUS);
    }
    Some(Snapshot {
        width,
        height,
        rgba,
    })
}

/// One horizontal and one vertical box blur pass over the RGB channels, repeating the edge pixels.
fn box_blur(rgba: &mut [u8], width: usize, height: usize, radius: usize) {
    let mut line = Vec::new();
    // Horizontal.
    for y in 0..height {
        line.clear();
        line.extend((0..width).map(|x| pixel(rgba, y * width + x)));
        let blurred = blur_line(&line, radius);
        for (x, p) in blurred.into_iter().enumerate() {
            put(rgba, y * width + x, p);
        }
    }
    // Vertical.
    for x in 0..width {
        line.clear();
        line.extend((0..height).map(|y| pixel(rgba, y * width + x)));
        let blurred = blur_line(&line, radius);
        for (y, p) in blurred.into_iter().enumerate() {
            put(rgba, y * width + x, p);
        }
    }
}

fn pixel(rgba: &[u8], i: usize) -> [u32; 3] {
    [
        u32::from(rgba[i * 4]),
        u32::from(rgba[i * 4 + 1]),
        u32::from(rgba[i * 4 + 2]),
    ]
}

fn put(rgba: &mut [u8], i: usize, p: [u32; 3]) {
    rgba[i * 4] = p[0] as u8;
    rgba[i * 4 + 1] = p[1] as u8;
    rgba[i * 4 + 2] = p[2] as u8;
}

/// A running-sum box blur of one row or column; positions outside it take the nearest end's value.
fn blur_line(line: &[[u32; 3]], radius: usize) -> Vec<[u32; 3]> {
    let n = line.len();
    let at = |i: isize| line[i.clamp(0, n as isize - 1) as usize];
    let r = radius as isize;
    let size = 2 * radius as u32 + 1;
    let mut sum = [0u32; 3];
    for i in -r..=r {
        let p = at(i);
        for c in 0..3 {
            sum[c] += p[c];
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n as isize {
        out.push([sum[0] / size, sum[1] / size, sum[2] / size]);
        let (add, remove) = (at(i + r + 1), at(i - r));
        for c in 0..3 {
            sum[c] = sum[c] + add[c] - remove[c];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_keeps_a_flat_colour() {
        let line = vec![[10, 20, 30]; 9];
        assert_eq!(blur_line(&line, 3), line);
    }

    #[test]
    fn blur_spreads_a_step_and_keeps_the_ends() {
        let mut line = vec![[0, 0, 0]; 20];
        for p in &mut line[10..] {
            *p = [210, 210, 210];
        }
        let out = blur_line(&line, 3);
        assert_eq!(out[0], [0, 0, 0]);
        assert_eq!(out[19], [210, 210, 210]);
        assert!(out[9][0] > 0 && out[9][0] < 210);
        assert!(out.windows(2).all(|w| w[0][0] <= w[1][0]));
    }
}
