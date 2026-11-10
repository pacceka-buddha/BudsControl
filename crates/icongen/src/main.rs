//! Renders the app icon (two earbuds inside a battery ring on a glassy blue squircle), after the
//! original `tools/MakeIcon.swift`. Drawn from scratch; no third-party artwork.
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

/// One earbud for the left ear; `mirror` flips it for the right ear.
fn earbud(pixmap: &mut Pixmap, center: (f32, f32), size: f32, mirror: bool) {
    let head_r = size * 0.25;
    let stem_w = size * 0.15;
    let stem_h = size * 0.62;
    let place = Transform::from_translate(center.0, center.1)
        .pre_scale(if mirror { -1.0 } else { 1.0 }, 1.0)
        .pre_rotate(-14.0);

    let stem = rounded_rect(
        head_r * 0.25 - stem_w / 2.0,
        0.0,
        stem_w,
        stem_h,
        stem_w / 2.0,
    );
    let head = ellipse(0.0, 0.0, head_r, head_r * 0.95);

    // Soft shadow: a few translucent offset copies.
    for step in 0..6 {
        let spread = step as f32 * size * 0.008;
        let paint = solid(rgba(0.0, 0.0, 0.0, 0.045));
        let shadow = place.pre_translate(0.0, size * 0.03 + spread * 0.5);
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

    // Body: white with a touch of blue towards the stem edge for volume.
    let body = gradient(
        (-head_r, 0.0),
        (head_r, 0.0),
        &[
            (0.0, rgba(1.0, 1.0, 1.0, 1.0)),
            (1.0, rgba(0.84, 0.88, 0.97, 1.0)),
        ],
    );
    for path in [&stem, &head] {
        pixmap.fill_path(path, &body, FillRule::Winding, place, None);
    }

    // Silicone ear tip on the inner side.
    let tip = ellipse(head_r * 0.85, head_r * 0.1, head_r * 0.3, head_r * 0.38);
    pixmap.fill_path(
        &tip,
        &solid(rgba(0.80, 0.84, 0.93, 1.0)),
        FillRule::Winding,
        place,
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

    // Blue to indigo, top-left to bottom-right.
    let fill = gradient(
        (bx + bw * 0.25, by),
        (bx + bw * 0.75, by + bw),
        &[
            (0.0, rgba(0.40, 0.70, 1.0, 1.0)),
            (0.5, rgba(0.16, 0.36, 0.95, 1.0)),
            (1.0, rgba(0.33, 0.18, 0.80, 1.0)),
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
            (0.0, rgba(1.0, 1.0, 1.0, 0.35)),
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
        &solid(rgba(1.0, 1.0, 1.0, 0.45)),
        &Stroke {
            width: (s * 0.006).max(1.0),
            ..Stroke::default()
        },
        Transform::identity(),
        None,
    );

    // Battery ring: a faint track and a three-quarter arc, like the tray icon.
    let (cx, cy, radius) = (s / 2.0, s / 2.0, s * 0.285);
    let ring_width = s * 0.035;
    if let Some(track) = PathBuilder::from_circle(cx, cy, radius) {
        pixmap.stroke_path(
            &track,
            &solid(rgba(1.0, 1.0, 1.0, 0.22)),
            &Stroke {
                width: ring_width,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }
    let mut arc = PathBuilder::new();
    // From 12 o'clock clockwise through 3 and 6 to 9 o'clock.
    for i in 0..=135 {
        let degrees = 90.0 - 270.0 * (i as f32 / 135.0);
        let radians = degrees.to_radians();
        let point = (cx + radius * radians.cos(), cy - radius * radians.sin());
        if i == 0 {
            arc.move_to(point.0, point.1);
        } else {
            arc.line_to(point.0, point.1);
        }
    }
    if let Some(arc) = arc.finish() {
        pixmap.stroke_path(
            &arc,
            &solid(rgba(1.0, 1.0, 1.0, 0.95)),
            &Stroke {
                width: ring_width,
                line_cap: LineCap::Round,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }

    // Earbuds are drawn with the y axis pointing down: the stem hangs below the head.
    earbud(&mut pixmap, (s * 0.385, s * 0.45), s * 0.3, false);
    earbud(&mut pixmap, (s * 0.615, s * 0.45), s * 0.3, true);

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
