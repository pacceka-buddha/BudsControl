//! Renders the app icon: two upright earbuds inside a split battery ring (left half cyan, right half
//! violet) on a graphite glass squircle. Same idea as the tray icon in `buds-art`. Drawn from
//! scratch; no third-party artwork.
//!
//! Usage: `cargo run -p icongen -- app-icon.png [size]` (default 1024 px), then
//! `npx tauri icon app-icon.png` generates the platform icons in `src-tauri/icons`.

use std::path::PathBuf;
use std::process::ExitCode;

use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, Path, PathBuilder, Pixmap,
    Point, SpreadMode, Stroke, Transform,
};

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::from_rgba(r, g, b, a).expect("valid colour")
}

fn gradient(from: (f32, f32), to: (f32, f32), stops: &[(f32, Color)]) -> Paint<'static> {
    let stops: Vec<GradientStop> = stops
        .iter()
        .map(|(at, color)| GradientStop::new(*at, *color))
        .collect();
    let shader = LinearGradient::new(
        Point::from_xy(from.0, from.1),
        Point::from_xy(to.0, to.1),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    )
    .expect("valid gradient");
    Paint {
        shader,
        anti_alias: true,
        ..Paint::default()
    }
}

fn solid(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    paint
}

/// A rounded rectangle with circular corners.
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Path {
    const K: f32 = 0.552_284_8;
    let r = radius.min(w / 2.0).min(h / 2.0);
    let c = r * K;
    let (right, bottom) = (x + w, y + h);
    let mut p = PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(right - r, y);
    p.cubic_to(right - r + c, y, right, y + r - c, right, y + r);
    p.line_to(right, bottom - r);
    p.cubic_to(
        right,
        bottom - r + c,
        right - r + c,
        bottom,
        right - r,
        bottom,
    );
    p.line_to(x + r, bottom);
    p.cubic_to(x + r - c, bottom, x, bottom - r + c, x, bottom - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + r - c, x + r - c, y, x + r, y);
    p.close();
    p.finish().expect("rounded rectangle")
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
    let rect = tiny_skia::Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0).expect("ellipse");
    PathBuilder::from_oval(rect).expect("ellipse")
}

/// An arc from `from` to `to` degrees, clockwise from 12 o'clock.
fn arc(cx: f32, cy: f32, radius: f32, from: f32, to: f32) -> Option<Path> {
    let steps = ((to - from) / 1.5).ceil().max(2.0) as u32;
    let mut b = PathBuilder::new();
    for i in 0..=steps {
        let angle = (from - 90.0 + (to - from) * i as f32 / steps as f32).to_radians();
        let (x, y) = (cx + radius * angle.cos(), cy + radius * angle.sin());
        if i == 0 {
            b.move_to(x, y);
        } else {
            b.line_to(x, y);
        }
    }
    b.finish()
}

/// One upright earbud: an oval head at (`x`, `y`) with a stem hanging straight down, and the
/// silicone ear tip on the side facing the other bud (`inward` is +1 for the left bud, -1 for the
/// right). `u` is the size of one unit of the tray glyph's 24-unit grid.
fn earbud(pixmap: &mut Pixmap, x: f32, y: f32, u: f32, inward: f32) {
    let (rx, ry) = (3.0 * u, 3.7 * u);
    let stem_w = 2.6 * u;
    let head = ellipse(x, y, rx, ry);
    let stem = rounded_rect(
        x - stem_w / 2.0,
        y,
        stem_w,
        6.0 * u + stem_w / 2.0,
        stem_w / 2.0,
    );

    // Soft shadow: a few translucent copies, growing and dropping a little.
    for step in 0..6 {
        let spread = step as f32 * u * 0.12;
        let paint = solid(rgba(0.0, 0.0, 0.0, 0.06));
        let shadow = Transform::from_translate(0.0, u * 0.5 + spread * 0.5);
        for path in [&stem, &head] {
            if let Some(grown) = path.clone().stroke(
                &Stroke {
                    width: spread * 2.0,
                    ..Stroke::default()
                },
                1.0,
            ) {
                pixmap.fill_path(&grown, &paint, FillRule::Winding, shadow, None);
            }
            pixmap.fill_path(path, &paint, FillRule::Winding, shadow, None);
        }
    }

    // Body: white, shading to a cool grey on the outer side for volume.
    let body = gradient(
        (x + rx * inward, y),
        (x - rx * inward, y),
        &[
            (0.0, rgba(1.0, 1.0, 1.0, 1.0)),
            (1.0, rgba(0.80, 0.83, 0.90, 1.0)),
        ],
    );
    for path in [&stem, &head] {
        pixmap.fill_path(path, &body, FillRule::Winding, Transform::identity(), None);
    }

    // Ear tip on the inner side.
    let tip = ellipse(x + rx * 0.62 * inward, y + ry * 0.05, rx * 0.36, ry * 0.36);
    pixmap.fill_path(
        &tip,
        &solid(rgba(0.74, 0.78, 0.88, 1.0)),
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn render(px: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(px, px).expect("pixmap");
    let s = px as f32;

    // The squircle fills most of the canvas (Windows icons have no macOS-style margin).
    let margin = s * 0.04;
    let (bx, by, bw) = (margin, margin, s - margin * 2.0);
    let shape = rounded_rect(bx, by, bw, bw, bw * 0.225);

    // Drop shadow under the squircle.
    for step in 0..8 {
        let grow = step as f32 * s * 0.0025;
        let shadow = rounded_rect(
            bx - grow,
            by + s * 0.012 - grow,
            bw + grow * 2.0,
            bw + grow * 2.0,
            bw * 0.225 + grow,
        );
        pixmap.fill_path(
            &shadow,
            &solid(rgba(0.0, 0.0, 0.0, 0.035)),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // Graphite, lighter at the top left.
    let fill = gradient(
        (bx + bw * 0.2, by),
        (bx + bw * 0.8, by + bw),
        &[
            (0.0, rgba(0.235, 0.255, 0.305, 1.0)),
            (0.5, rgba(0.105, 0.115, 0.145, 1.0)),
            (1.0, rgba(0.047, 0.055, 0.071, 1.0)),
        ],
    );
    pixmap.fill_path(
        &shape,
        &fill,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    // Glass sheen on the upper half, and a thin bright rim.
    let sheen = gradient(
        (0.0, by),
        (0.0, by + bw / 2.0),
        &[
            (0.0, rgba(1.0, 1.0, 1.0, 0.14)),
            (1.0, rgba(1.0, 1.0, 1.0, 0.0)),
        ],
    );
    pixmap.fill_path(
        &shape,
        &sheen,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    let rim = rounded_rect(
        bx + s * 0.004,
        by + s * 0.004,
        bw - s * 0.008,
        bw - s * 0.008,
        bw * 0.222,
    );
    pixmap.stroke_path(
        &rim,
        &solid(rgba(1.0, 1.0, 1.0, 0.22)),
        &Stroke {
            width: (s * 0.006).max(1.0),
            ..Stroke::default()
        },
        Transform::identity(),
        None,
    );

    // Split battery ring: the left half is the left bud (cyan, 80%), the right half the right bud
    // (violet, 55%). Each half has a faint track and fills up from the bottom.
    let (cx, cy, radius) = (s / 2.0, s / 2.0, s * 0.30);
    let ring_width = s * 0.04;
    let half_gap = ((s * 0.03 + ring_width) / 2.0 / radius).to_degrees();
    let span = 180.0 - 2.0 * half_gap;
    let stroke = Stroke {
        width: ring_width,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    let mut ring = |from: f32, to: f32, color: Color| {
        if let Some(path) = arc(cx, cy, radius, from, to) {
            pixmap.stroke_path(&path, &solid(color), &stroke, Transform::identity(), None);
        }
    };
    let track = rgba(1.0, 1.0, 1.0, 0.13);
    let (right_start, left_start) = (half_gap, 180.0 + half_gap);
    ring(right_start, right_start + span, track);
    ring(left_start, left_start + span, track);
    ring(
        left_start,
        left_start + span * 0.80,
        rgba(0.36, 0.80, 1.0, 1.0),
    );
    let right_end = right_start + span;
    ring(
        right_end - span * 0.55,
        right_end,
        rgba(0.65, 0.55, 1.0, 1.0),
    );

    // Earbuds on the tray glyph's grid, scaled up: heads at x = 7 and 17 of 24, y = 8.9.
    let u = s * 0.0215;
    let head_y = cy - 3.1 * u;
    earbud(&mut pixmap, cx - 5.0 * u, head_y, u, 1.0);
    earbud(&mut pixmap, cx + 5.0 * u, head_y, u, -1.0);

    pixmap
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next().map(PathBuf::from) else {
        eprintln!("usage: icongen <output.png> [size]");
        return ExitCode::from(2);
    };
    let size: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(1024);
    match render(size).save_png(&out) {
        Ok(()) => {
            println!("wrote {} ({size} px)", out.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("icongen: cannot write {}: {error}", out.display());
            ExitCode::FAILURE
        }
    }
}
