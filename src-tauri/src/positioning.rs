//! Where the flyout goes: next to the tray icon, on the taskbar's side of the screen, kept inside the
//! monitor's work area. Pure integer geometry in physical pixels, so it is unit-tested.

/// A rectangle in physical (device) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

/// Which screen edge the taskbar (and so the tray icon) is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

/// The taskbar edge: the side of the work area the icon lies outside of. An icon inside the work area
/// (for example in the tray overflow popup) is treated as a bottom taskbar.
pub fn taskbar_edge(icon: Rect, work: Rect) -> Edge {
    let (cx, cy) = icon.center();
    if cy >= work.bottom() {
        Edge::Bottom
    } else if cy < work.y {
        Edge::Top
    } else if cx < work.x {
        Edge::Left
    } else if cx >= work.right() {
        Edge::Right
    } else {
        Edge::Bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub edge: Edge,
}

fn clamp_span(value: i32, lo: i32, hi: i32) -> i32 {
    // If the window is bigger than the space, stick to the low edge.
    if hi < lo { lo } else { value.clamp(lo, hi) }
}

/// Top-left position for a window of `size` (w, h) with `margin` to the work-area edges. With no icon
/// (unknown tray position) the bottom-right corner is used, which is where the tray usually is.
pub fn place(icon: Option<Rect>, work: Rect, size: (i32, i32), margin: i32) -> Placement {
    let (w, h) = size;
    let min_x = work.x + margin;
    let max_x = work.right() - w - margin;
    let min_y = work.y + margin;
    let max_y = work.bottom() - h - margin;
    let Some(icon) = icon else {
        return Placement {
            x: clamp_span(max_x, min_x, max_x),
            y: clamp_span(max_y, min_y, max_y),
            edge: Edge::Bottom,
        };
    };
    let edge = taskbar_edge(icon, work);
    let (cx, cy) = icon.center();
    let (x, y) = match edge {
        Edge::Bottom => (
            clamp_span(cx - w / 2, min_x, max_x),
            clamp_span(max_y, min_y, max_y),
        ),
        Edge::Top => (clamp_span(cx - w / 2, min_x, max_x), min_y),
        Edge::Left => (min_x, clamp_span(cy - h / 2, min_y, max_y)),
        Edge::Right => (
            clamp_span(max_x, min_x, max_x),
            clamp_span(cy - h / 2, min_y, max_y),
        ),
    };
    Placement { x, y, edge }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 1920x1080 monitor with a 48 px taskbar at the bottom.
    const WORK_BOTTOM: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1032,
    };

    #[test]
    fn bottom_taskbar_centres_above_the_icon() {
        let icon = Rect::new(1700, 1040, 24, 24);
        let p = place(Some(icon), WORK_BOTTOM, (300, 400), 12);
        assert_eq!(p.edge, Edge::Bottom);
        assert_eq!(p.x, 1712 - 150); // icon centre minus half the width
        assert_eq!(p.y, 1032 - 400 - 12); // bottom edge sits 12 px above the taskbar
    }

    #[test]
    fn clamps_inside_the_work_area() {
        // Icon at the far right: the window can't stick out past the margin.
        let icon = Rect::new(1890, 1040, 24, 24);
        let p = place(Some(icon), WORK_BOTTOM, (300, 400), 12);
        assert_eq!(p.x, 1920 - 300 - 12);
        // Icon at the far left.
        let p = place(
            Some(Rect::new(2, 1040, 24, 24)),
            WORK_BOTTOM,
            (300, 400),
            12,
        );
        assert_eq!(p.x, 12);
    }

    #[test]
    fn top_taskbar_hangs_below_it() {
        let work = Rect::new(0, 48, 1920, 1032);
        let icon = Rect::new(1700, 8, 24, 24);
        let p = place(Some(icon), work, (300, 400), 12);
        assert_eq!(p.edge, Edge::Top);
        assert_eq!(p.y, 48 + 12);
    }

    #[test]
    fn left_and_right_taskbars() {
        let work = Rect::new(60, 0, 1860, 1080);
        // Icon mid-screen: the window is centred on it.
        let p = place(Some(Rect::new(18, 500, 24, 24)), work, (300, 400), 12);
        assert_eq!(p.edge, Edge::Left);
        assert_eq!(p.x, 60 + 12);
        assert_eq!(p.y, 512 - 200);
        // Icon near the bottom: clamped so the window stays inside the work area.
        let p = place(Some(Rect::new(18, 900, 24, 24)), work, (300, 400), 12);
        assert_eq!(p.y, 1080 - 400 - 12);

        let work = Rect::new(0, 0, 1860, 1080);
        let p = place(Some(Rect::new(1878, 900, 24, 24)), work, (300, 400), 12);
        assert_eq!(p.edge, Edge::Right);
        assert_eq!(p.x, 1860 - 300 - 12);
    }

    #[test]
    fn secondary_monitor_with_negative_coordinates() {
        // A monitor to the left of the primary one.
        let work = Rect::new(-1920, 0, 1920, 1032);
        let icon = Rect::new(-300, 1040, 24, 24);
        let p = place(Some(icon), work, (300, 400), 12);
        assert_eq!(p.x, -288 - 150);
        assert!(p.x >= work.x + 12 && p.x + 300 <= work.right() - 12);
    }

    #[test]
    fn no_icon_uses_the_bottom_right_corner() {
        let p = place(None, WORK_BOTTOM, (300, 400), 12);
        assert_eq!((p.x, p.y), (1920 - 300 - 12, 1032 - 400 - 12));
    }

    #[test]
    fn growing_keeps_the_anchored_edge_fixed() {
        let icon = Rect::new(1700, 1040, 24, 24);
        let small = place(Some(icon), WORK_BOTTOM, (300, 200), 12);
        let big = place(Some(icon), WORK_BOTTOM, (300, 500), 12);
        assert_eq!(
            small.y + 200,
            big.y + 500,
            "bottom edge stays put, the window grows upward"
        );
        assert_eq!(small.x, big.x);
        let top = Rect::new(0, 48, 1920, 1032);
        let a = place(Some(Rect::new(1700, 8, 24, 24)), top, (300, 200), 12);
        let b = place(Some(Rect::new(1700, 8, 24, 24)), top, (300, 500), 12);
        assert_eq!(a.y, b.y, "top edge stays put, the window grows downward");
    }

    #[test]
    fn a_window_taller_than_the_screen_sticks_to_the_top() {
        let p = place(
            Some(Rect::new(1700, 1040, 24, 24)),
            WORK_BOTTOM,
            (300, 2000),
            12,
        );
        assert_eq!(p.y, 12);
    }

    #[test]
    fn overflow_popup_icons_count_as_bottom() {
        // The icon lives inside the work area (the hidden-icons popup).
        let icon = Rect::new(1650, 900, 24, 24);
        assert_eq!(taskbar_edge(icon, WORK_BOTTOM), Edge::Bottom);
    }
}
