//! Drawing of the earbuds glyph and the battery ring for the tray icon, at any size and taskbar
//! colour. Pure software rendering with tiny-skia, so the
//! output is the same everywhere and can be unit-tested pixel by pixel.

use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke,
    Transform,
};

/// The glyph and ring are drawn in a single colour (the tray follows the taskbar: white on dark,
/// black on light) with transparency doing the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const WHITE: Rgb = Rgb(255, 255, 255);
    pub const BLACK: Rgb = Rgb(0, 0, 0);
}

/// An area of the pixmap, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub fn square(size: u32) -> Area {
        Area {
            x: 0.0,
            y: 0.0,
            w: size as f32,
            h: size as f32,
        }
    }
}

fn paint(color: Rgb, alpha: f32) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        color.0,
        color.1,
        color.2,
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    );
    paint.anti_alias = true;
    paint
}

/// Two upright earbuds, drawn like the app icon: an oval head with a straight stem below it, the
/// pair mirrored about the centre. The drawing is laid out on a 24-unit grid and scaled into `area`.
pub fn draw_glyph(pixmap: &mut Pixmap, area: Area, color: Rgb, alpha: f32) {
    // Drawn fully opaque into a layer and composited with `alpha`, so the places where a head and a
    // stem overlap don't add up to a brighter joint.
    let Some(mut layer) = Pixmap::new(pixmap.width(), pixmap.height()) else {
        return;
    };
    let unit = area.w.min(area.h) / 24.0;
    let t = Transform::from_scale(unit, unit).post_translate(area.x, area.y);
    let paint = paint(color, 1.0);
    let stroke = Stroke {
        width: 2.6,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };

    // Heads 3 units either side of x = 7 and 17, leaving 4 units (1.5 px at 16 px) between them.
    for x in [7.0_f32, 17.0] {
        if let Some(head) = Rect::from_xywh(x - 3.0, 5.2, 6.0, 7.4).and_then(|r| {
            let mut b = PathBuilder::new();
            b.push_oval(r);
            b.finish()
        }) {
            layer.fill_path(&head, &paint, FillRule::Winding, t, None);
        }
        let mut b = PathBuilder::new();
        b.move_to(x, 12.0);
        b.line_to(x, 18.0);
        if let Some(stem) = b.finish() {
            layer.stroke_path(&stem, &paint, &stroke, t, None);
        }
    }
    let composite = PixmapPaint {
        opacity: alpha.clamp(0.0, 1.0),
        ..PixmapPaint::default()
    };
    pixmap.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &composite,
        Transform::identity(),
        None,
    );
}

/// An arc of the circle at (`cx`, `cy`), from `from` to `to` degrees measured clockwise from
/// 12 o'clock (`from` < `to`).
fn arc_path(cx: f32, cy: f32, radius: f32, from: f32, to: f32) -> Option<Path> {
    let sweep = to - from;
    let steps = (sweep / 3.0).ceil().max(2.0) as u32;
    let mut b = PathBuilder::new();
    for i in 0..=steps {
        let angle = (from - 90.0 + sweep * i as f32 / steps as f32).to_radians();
        let (x, y) = (cx + radius * angle.cos(), cy + radius * angle.sin());
        if i == 0 {
            b.move_to(x, y);
        } else {
            b.line_to(x, y);
        }
    }
    b.finish()
}

/// A ring inside `area` cut into two halves: the left half is the left earbud's battery and the right
/// half the right earbud's, each filling up from the bottom. Each half has a faint track; a level of
/// `None` (or 0) draws the track only. `gap` is the clear space between the halves at 12 and at
/// 6 o'clock, in pixels, measured between the round caps.
#[allow(clippy::too_many_arguments)]
pub fn draw_split_ring(
    pixmap: &mut Pixmap,
    area: Area,
    left: Option<u8>,
    right: Option<u8>,
    color: Rgb,
    track_alpha: f32,
    line_width: f32,
    gap: f32,
) {
    let cx = area.x + area.w / 2.0;
    let cy = area.y + area.h / 2.0;
    let radius = (area.w.min(area.h) - line_width) / 2.0;
    let stroke = Stroke {
        width: line_width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    // Each half stops short of 12 and 6 o'clock by half the gap plus the reach of its round cap.
    let half_gap = ((gap + line_width) / 2.0 / radius).to_degrees();
    let span = 180.0 - 2.0 * half_gap;
    let mut draw = |from: f32, to: f32, alpha: f32| {
        if let Some(path) = arc_path(cx, cy, radius, from, to) {
            pixmap.stroke_path(
                &path,
                &paint(color, alpha),
                &stroke,
                Transform::identity(),
                None,
            );
        }
    };

    // Right half: 12 o'clock round to 6, filled from 6 o'clock upwards.
    // Left half: 6 o'clock round to 12, filled from 6 o'clock upwards.
    let (right_start, left_start) = (half_gap, 180.0 + half_gap);
    if track_alpha > 0.0 {
        draw(right_start, right_start + span, track_alpha);
        draw(left_start, left_start + span, track_alpha);
    }
    let fraction = |level: Option<u8>| {
        level
            .filter(|l| *l > 0)
            .map(|l| span * f32::from(l.min(100)) / 100.0)
    };
    if let Some(sweep) = fraction(right) {
        let end = right_start + span;
        draw(end - sweep, end, 1.0);
    }
    if let Some(sweep) = fraction(left) {
        draw(left_start, left_start + sweep, 1.0);
    }
}

/// What the tray icon should show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayIconSpec {
    /// Pixels per side (16 at 100% scaling, 24 at 150%, 32 at 200%).
    pub size: u32,
    /// Left earbud battery, if known.
    pub left: Option<u8>,
    /// Right earbud battery, if known.
    pub right: Option<u8>,
    /// The earbuds are connected; otherwise the icon is dimmed and shows no battery.
    pub connected: bool,
    /// The taskbar is light, so the icon is black instead of white.
    pub light_taskbar: bool,
}

/// An RGBA image (straight, not premultiplied, as the tray expects).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Track opacity of the ring and glyph opacity while disconnected.
const TRACK_ALPHA: f32 = 0.28;
const DIMMED_ALPHA: f32 = 0.45;

/// Renders the tray icon: the earbuds inside a ring split into the left and right battery.
pub fn render_tray_icon(spec: TrayIconSpec) -> Image {
    let size = spec.size.clamp(8, 256);
    let mut pixmap = Pixmap::new(size, size).expect("size is non-zero");
    let color = if spec.light_taskbar {
        Rgb::BLACK
    } else {
        Rgb::WHITE
    };

    let line_width = (size as f32 * 0.11).max(1.6);
    let ring = Area::square(size);
    let (left, right) = if spec.connected {
        (spec.left, spec.right)
    } else {
        (None, None)
    };
    draw_split_ring(
        &mut pixmap,
        ring,
        left,
        right,
        color,
        TRACK_ALPHA,
        line_width,
        // Two whole pixels at 16 px, so anti-aliasing can't fill the gap in.
        (size as f32 * 0.08).max(2.0),
    );

    // The glyph sits inside the ring with a little air around it.
    let glyph_size = size as f32 * 0.56;
    let offset = (size as f32 - glyph_size) / 2.0;
    let glyph = Area {
        x: offset,
        y: offset,
        w: glyph_size,
        h: glyph_size,
    };
    draw_glyph(
        &mut pixmap,
        glyph,
        color,
        if spec.connected { 1.0 } else { DIMMED_ALPHA },
    );

    // tiny-skia stores premultiplied colour; the tray wants straight alpha.
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for pixel in pixmap.pixels() {
        let c = pixel.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Image {
        width: size,
        height: size,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(left: Option<u8>, right: Option<u8>, connected: bool, light: bool, size: u32) -> Image {
        render_tray_icon(TrayIconSpec {
            size,
            left,
            right,
            connected,
            light_taskbar: light,
        })
    }

    fn both(level: Option<u8>, connected: bool, light: bool, size: u32) -> Image {
        icon(level, level, connected, light, size)
    }

    fn alpha(img: &Image, x: u32, y: u32) -> u8 {
        img.rgba[((y * img.width + x) * 4 + 3) as usize]
    }

    fn rgb(img: &Image, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * img.width + x) * 4) as usize;
        (img.rgba[i], img.rgba[i + 1], img.rgba[i + 2])
    }

    /// The pixel on the ring at `degrees` clockwise from 12 o'clock.
    fn ring_alpha(img: &Image, degrees: f32) -> u8 {
        let c = img.width as f32 / 2.0;
        let r = (img.width as f32 - (img.width as f32 * 0.11).max(1.6)) / 2.0;
        let a = (degrees - 90.0).to_radians();
        alpha(img, (c + r * a.cos()) as u32, (c + r * a.sin()) as u32)
    }

    fn covered(img: &Image, degrees: f32) -> bool {
        ring_alpha(img, degrees) > 230
    }

    fn track_only(img: &Image, degrees: f32) -> bool {
        (40..=110).contains(&ring_alpha(img, degrees))
    }

    #[test]
    fn sizes_match_the_request() {
        for size in [16, 20, 24, 32, 48] {
            let img = both(Some(50), true, false, size);
            assert_eq!((img.width, img.height), (size, size));
            assert_eq!(img.rgba.len(), (size * size * 4) as usize);
        }
    }

    #[test]
    fn the_left_half_fills_up_from_the_bottom_with_the_left_level() {
        // The left half runs from 192 to 348 degrees; at 50% it covers 192..270.
        let img = icon(Some(50), None, true, false, 32);
        for deg in [200.0, 230.0, 262.0] {
            assert!(covered(&img, deg), "left covered at {deg}");
        }
        for deg in [290.0, 330.0] {
            assert!(
                track_only(&img, deg),
                "left track at {deg}: {}",
                ring_alpha(&img, deg)
            );
        }
        // The right half has no level: track only.
        for deg in [30.0, 90.0, 150.0] {
            assert!(
                track_only(&img, deg),
                "right track at {deg}: {}",
                ring_alpha(&img, deg)
            );
        }
    }

    #[test]
    fn the_right_half_fills_up_from_the_bottom_with_the_right_level() {
        // The right half runs from 12 to 168 degrees; at 50% it covers 90..168.
        let img = icon(None, Some(50), true, false, 32);
        for deg in [98.0, 130.0, 160.0] {
            assert!(covered(&img, deg), "right covered at {deg}");
        }
        for deg in [30.0, 70.0] {
            assert!(
                track_only(&img, deg),
                "right track at {deg}: {}",
                ring_alpha(&img, deg)
            );
        }
        for deg in [210.0, 270.0, 330.0] {
            assert!(
                track_only(&img, deg),
                "left track at {deg}: {}",
                ring_alpha(&img, deg)
            );
        }
    }

    #[test]
    fn the_halves_are_apart_at_the_top_and_bottom() {
        let img = both(Some(100), true, false, 32);
        for deg in [0.0, 180.0] {
            assert!(
                ring_alpha(&img, deg) < 60,
                "gap at {deg}: {}",
                ring_alpha(&img, deg)
            );
        }
        for deg in [30.0, 90.0, 150.0, 210.0, 270.0, 330.0] {
            assert!(covered(&img, deg), "full at {deg}");
        }
        // At 16 px the gap is still clear: the two middle pixels of the top and bottom rows.
        let small = both(Some(100), true, false, 16);
        for (x, y) in [(7, 0), (8, 0), (7, 15), (8, 15)] {
            let a = alpha(&small, x, y);
            assert!(a < 40, "16 px gap at {x},{y}: {a}");
        }
    }

    #[test]
    fn empty_and_unknown_levels_show_the_track_only() {
        for img in [both(None, true, false, 32), both(Some(0), true, false, 32)] {
            for deg in [90.0, 270.0] {
                assert!(
                    track_only(&img, deg),
                    "track at {deg}: {}",
                    ring_alpha(&img, deg)
                );
            }
        }
    }

    #[test]
    fn disconnected_is_dimmed_and_shows_no_battery() {
        let on = both(Some(80), true, false, 32);
        let off = both(Some(80), false, false, 32);
        assert!(covered(&on, 90.0));
        assert!(ring_alpha(&off, 90.0) < 120, "no arc while disconnected");
        // The glyph (centre area) is fainter.
        let glyph_alpha = |img: &Image| {
            (10..22)
                .flat_map(|y| (10..22).map(move |x| (x, y)))
                .map(|(x, y)| u32::from(alpha(img, x, y)))
                .max()
                .unwrap()
        };
        assert!(glyph_alpha(&on) > 240);
        assert!(
            (90..=130).contains(&glyph_alpha(&off)),
            "glyph at ~45%: {}",
            glyph_alpha(&off)
        );
    }

    #[test]
    fn the_colour_follows_the_taskbar() {
        let dark = both(Some(100), true, false, 32);
        let light = both(Some(100), true, true, 32);
        let find_opaque = |img: &Image| {
            (0..32)
                .flat_map(|y| (0..32).map(move |x| (x, y)))
                .find(|&(x, y)| alpha(img, x, y) == 255)
                .expect("an opaque pixel")
        };
        let (x, y) = find_opaque(&dark);
        assert_eq!(rgb(&dark, x, y), (255, 255, 255));
        let (x, y) = find_opaque(&light);
        assert_eq!(rgb(&light, x, y), (0, 0, 0));
    }

    #[test]
    fn the_corners_stay_transparent_and_the_glyph_is_centred() {
        let img = both(Some(100), true, false, 32);
        for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert_eq!(alpha(&img, x, y), 0, "corner {x},{y}");
        }
        // The two buds are mirror images: the glyph's pixels balance left and right of the centre.
        let mass = |range: std::ops::Range<u32>| -> u32 {
            (8..24)
                .flat_map(|y| range.clone().map(move |x| (x, y)))
                .map(|(x, y)| u32::from(alpha(&img, x, y)))
                .sum()
        };
        let (left, right) = (mass(8..16), mass(16..24));
        let diff = left.abs_diff(right) as f32 / left.max(right) as f32;
        assert!(diff < 0.05, "left {left} right {right}");
    }

    #[test]
    fn straight_alpha_is_demultiplied() {
        // A half-transparent pixel keeps its full colour value (it isn't darkened by the alpha).
        let img = both(None, true, false, 32);
        let (x, y) = (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .find(|&(x, y)| (100..200).contains(&alpha(&img, x, y)))
            .expect("an edge pixel");
        assert_eq!(rgb(&img, x, y), (255, 255, 255));
    }
}
