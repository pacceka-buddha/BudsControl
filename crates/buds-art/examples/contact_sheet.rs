//! Renders every tray-icon state at the taskbar sizes onto dark and light taskbar colours, enlarged
//! with nearest-neighbour scaling, so the pixels can be inspected.
//!
//!     cargo run -p buds-art --example contact_sheet -- docs/screenshots/tray-icons.png

use buds_art::{TrayIconSpec, render_tray_icon};
use tiny_skia::{Color, Pixmap};

const ZOOM: u32 = 8;
const SIZES: [u32; 4] = [16, 20, 24, 32];
const PAD: u32 = 6;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tray-icons.png".into());
    // (label, left, right, connected)
    let states: [(&str, Option<u8>, Option<u8>, bool); 7] = [
        ("100%", Some(100), Some(100), true),
        ("75%", Some(75), Some(75), true),
        ("L 80% R 40%", Some(80), Some(40), true),
        ("15%", Some(15), Some(15), true),
        ("left only", Some(60), None, true),
        ("unknown", None, None, true),
        ("disconnected", Some(80), Some(80), false),
    ];

    let cell = |size: u32| size * ZOOM + PAD * 2;
    let col_w: u32 = SIZES.iter().map(|s| cell(*s)).sum::<u32>();
    let row_h = cell(32);
    let width = col_w * 2; // dark half, light half
    let height = row_h * states.len() as u32;
    let mut sheet = Pixmap::new(width, height).expect("sheet");

    for (half, light) in [false, true].into_iter().enumerate() {
        let bg = if light { (243, 243, 243) } else { (32, 32, 32) };
        let mut x0 = half as u32 * col_w;
        for size in SIZES {
            for (row, (_, left, right, connected)) in states.iter().enumerate() {
                let y0 = row as u32 * row_h;
                for y in 0..row_h {
                    for x in 0..cell(size) {
                        put(&mut sheet, x0 + x, y0 + y, bg, 255);
                    }
                }
                let img = render_tray_icon(TrayIconSpec {
                    size,
                    left: *left,
                    right: *right,
                    connected: *connected,
                    light_taskbar: light,
                });
                for y in 0..size {
                    for x in 0..size {
                        let i = ((y * size + x) * 4) as usize;
                        let (r, g, b, a) = (
                            img.rgba[i],
                            img.rgba[i + 1],
                            img.rgba[i + 2],
                            img.rgba[i + 3],
                        );
                        for dy in 0..ZOOM {
                            for dx in 0..ZOOM {
                                let px = x0 + PAD + x * ZOOM + dx;
                                let py = y0 + (row_h - size * ZOOM) / 2 + y * ZOOM + dy;
                                blend(&mut sheet, px, py, (r, g, b), a);
                            }
                        }
                    }
                }
            }
            x0 += cell(size);
        }
    }
    let _ = Color::BLACK;
    sheet.save_png(&out).expect("write png");
    println!(
        "wrote {out} ({width}x{height}); rows: {:?}",
        states.iter().map(|s| s.0).collect::<Vec<_>>()
    );
}

fn put(p: &mut Pixmap, x: u32, y: u32, rgb: (u8, u8, u8), a: u8) {
    let i = ((y * p.width() + x) * 4) as usize;
    let d = p.data_mut();
    d[i..i + 4].copy_from_slice(&[rgb.0, rgb.1, rgb.2, a]);
}

/// Straight-alpha "over" onto an opaque pixel.
fn blend(p: &mut Pixmap, x: u32, y: u32, rgb: (u8, u8, u8), a: u8) {
    let i = ((y * p.width() + x) * 4) as usize;
    let d = p.data_mut();
    let k = f32::from(a) / 255.0;
    for (c, src) in [(0, rgb.0), (1, rgb.1), (2, rgb.2)] {
        d[i + c] = (f32::from(src) * k + f32::from(d[i + c]) * (1.0 - k)).round() as u8;
    }
}
