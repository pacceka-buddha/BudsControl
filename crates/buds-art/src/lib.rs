//! Drawing of the earbuds glyph and the battery ring: the tray icon (this crate renders it for any
//! size and taskbar colour) and, later, the app icon. Pure software rendering with tiny-skia, so the
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

/// Two earbuds, drawn like the app icon: an oval head and a rounded stem, each tilted 14 degrees,
/// the right one mirrored. The drawing is laid out on a 24-unit grid and scaled into `area`.
pub fn draw_glyph(pixmap: &mut Pixmap, area: Area, color: Rgb, alpha: f32) {
    // Drawn fully opaque into a layer and composited with `alpha`, so the places where a head and a
    // stem overlap don't add up to a brighter joint.
    let Some(mut layer) = Pixmap::new(pixmap.width(), pixmap.height()) else {
        return;
    };
    let unit = area.w.min(area.h) / 24.0;
    let base = Transform::from_scale(unit, unit).post_translate(area.x, area.y);
    let paint = paint(color, 1.0);

    for (x, mirror) in [(7.4_f32, false), (16.6, true)] {
        // Head centre at (x, 8.4); the stem hangs from it. Rotation and mirroring are about the head.
        let mut t = base.pre_translate(x, 8.4);
        if mirror {
            t = t.pre_scale(-1.0, 1.0);
        }
        t = t.pre_rotate(14.0);

        let head = Rect::from_xywh(-3.3, -3.7, 6.6, 7.4).and_then(|r| {
            let mut b = PathBuilder::new();
            b.push_oval(r);
            b.finish()
        });
        if let Some(head) = head {
            layer.fill_path(&head, &paint, FillRule::Winding, t, None);
        }

        // Stem: a thick line with round caps.
        let mut b = PathBuilder::new();
        b.move_to(0.7, 1.0);
        b.line_to(0.7, 10.2);
        if let Some(stem) = b.finish() {
            let stroke = Stroke {
                width: 2.3,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
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

/// A ring inside `area`: a faint full circle (the track) and, on top, an arc from 12 o'clock
/// clockwise covering `level` percent. `level` of `None` draws the track only.
pub fn draw_ring(
    pixmap: &mut Pixmap,
    area: Area,
    level: Option<u8>,
    color: Rgb,
    track_alpha: f32,
    line_width: f32,
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

    if track_alpha > 0.0 {
        let mut b = PathBuilder::new();
        b.push_circle(cx, cy, radius);
        if let Some(track) = b.finish() {
            pixmap.stroke_path(
                &track,
                &paint(color, track_alpha),
                &stroke,
                Transform::identity(),
                None,
            );
        }
    }

    let Some(level) = level.map(|l| l.min(100)).filter(|l| *l > 0) else {
        return;
    };
    let arc: Option<Path> = {
        let mut b = PathBuilder::new();
        if level >= 100 {
            b.push_circle(cx, cy, radius);
        } else {
            let sweep = 360.0 * f32::from(level) / 100.0;
            let steps = (sweep / 3.0).ceil().max(2.0) as u32;
            for i in 0..=steps {
                let angle = (-90.0 + sweep * i as f32 / steps as f32).to_radians();
                let (x, y) = (cx + radius * angle.cos(), cy + radius * angle.sin());
                if i == 0 {
                    b.move_to(x, y);
                } else {
                    b.line_to(x, y);
                }
            }
        }
        b.finish()
    };
    if let Some(arc) = arc {
        pixmap.stroke_path(
            &arc,
            &paint(color, 1.0),
            &stroke,
            Transform::identity(),
            None,
        );
    }
}

/// What the tray icon should show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayIconSpec {
    /// Pixels per side (16 at 100% scaling, 24 at 150%, 32 at 200%).
    pub size: u32,
    /// Lowest earbud battery, if known.
    pub level: Option<u8>,
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

/// Renders the tray icon: the earbuds inside a battery ring.
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
    let ring_level = if spec.connected { spec.level } else { None };
    draw_ring(
        &mut pixmap,
        ring,
        ring_level,
        color,
        TRACK_ALPHA,
        line_width,
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

    fn icon(level: Option<u8>, connected: bool, light: bool, size: u32) -> Image {
        render_tray_icon(TrayIconSpec {
            size,
            level,
            connected,
            light_taskbar: light,
        })
    }

    fn alpha(img: &Image, x: u32, y: u32) -> u8 {
        img.rgba[((y * img.width + x) * 4 + 3) as usize]
    }

    fn rgb(img: &Image, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * img.width + x) * 4) as usize;
        (img.rgba[i], img.rgba[i + 1], img.rgba[i + 2])
    }

    /// The pixel on the ring at `degrees` clockwise from 12 o'clock (32 px icon: radius ~14).
    fn ring_alpha(img: &Image, degrees: f32) -> u8 {
        let c = img.width as f32 / 2.0;
        let r = (img.width as f32 - (img.width as f32 * 0.11).max(1.6)) / 2.0;
        let a = (degrees - 90.0).to_radians();
        alpha(img, (c + r * a.cos()) as u32, (c + r * a.sin()) as u32)
    }

    #[test]
    fn sizes_match_the_request() {
        for size in [16, 20, 24, 32, 48] {
            let img = icon(Some(50), true, false, size);
            assert_eq!((img.width, img.height), (size, size));
            assert_eq!(img.rgba.len(), (size * size * 4) as usize);
        }
    }

    #[test]
    fn the_arc_covers_the_battery_level_clockwise_from_the_top() {
        let img = icon(Some(50), true, false, 32);
        // Covered: 12 o'clock (just past), 3 o'clock, just short of 6 o'clock.
        assert!(ring_alpha(&img, 8.0) > 230, "top");
        assert!(ring_alpha(&img, 90.0) > 230, "right");
        assert!(ring_alpha(&img, 170.0) > 230, "bottom, just before the end");
        // Not covered: only the faint track (28%) is there.
        for deg in [200.0, 270.0, 340.0] {
            let a = ring_alpha(&img, deg);
            assert!(
                (40..=110).contains(&a),
                "track only at {deg} degrees, alpha {a}"
            );
        }
    }

    #[test]
    fn full_empty_and_unknown_levels() {
        let full = icon(Some(100), true, false, 32);
        for deg in [0.0, 90.0, 180.0, 270.0] {
            assert!(ring_alpha(&full, deg) > 230, "full ring at {deg}");
        }
        let none = icon(None, true, false, 32);
        let zero = icon(Some(0), true, false, 32);
        for deg in [0.0, 90.0, 180.0, 270.0] {
            assert!(
                ring_alpha(&none, deg) < 120,
                "track only when the level is unknown"
            );
            assert!(ring_alpha(&zero, deg) < 120, "track only at 0%");
        }
    }

    #[test]
    fn disconnected_is_dimmed_and_shows_no_battery() {
        let on = icon(Some(80), true, false, 32);
        let off = icon(Some(80), false, false, 32);
        assert!(ring_alpha(&on, 90.0) > 230);
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
        let dark = icon(Some(100), true, false, 32);
        let light = icon(Some(100), true, true, 32);
        let c = 16;
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
        let _ = c;
    }

    #[test]
    fn the_corners_stay_transparent_and_the_glyph_is_centred() {
        let img = icon(Some(100), true, false, 32);
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
        let img = icon(None, true, false, 32);
        let (x, y) = (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .find(|&(x, y)| (100..200).contains(&alpha(&img, x, y)))
            .expect("an edge pixel");
        assert_eq!(rgb(&img, x, y), (255, 255, 255));
    }
}
