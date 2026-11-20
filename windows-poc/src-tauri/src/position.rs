//! Pure geometry for the Windows screen-selection prototype.
//! All rectangles, cursor coordinates, margins and sizes are physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    pub bounds: Rect,
    pub work: Rect,
}

/// Selects the screen at the cursor, or the first screen as a fallback.
/// Returns its index and the top-left position of a bottom-right aligned window.
/// The caller must supply an appropriate margin and window size for that screen's DPI.
pub fn position(screens: &[Screen], cursor: (i32, i32), size: (u32, u32), margin: u32) -> Option<(usize, (i32, i32))> {
    let index = screens.iter().position(|s| cursor.0 >= s.bounds.left && cursor.0 < s.bounds.right && cursor.1 >= s.bounds.top && cursor.1 < s.bounds.bottom).unwrap_or(0);
    let screen = screens.get(index)?;
    let r = screen.work;
    if r.right <= r.left || r.bottom <= r.top || size.0 == 0 || size.1 == 0 { return None; }
    let width = i64::from(r.right) - i64::from(r.left);
    let height = i64::from(r.bottom) - i64::from(r.top);
    let x = (i64::from(r.right) - i64::from(size.0) - i64::from(margin)).max(i64::from(r.left));
    let y = (i64::from(r.bottom) - i64::from(size.1) - i64::from(margin)).max(i64::from(r.top));
    if i64::from(size.0) > width || i64::from(size.1) > height { return None; }
    Some((index, (i32::try_from(x).ok()?, i32::try_from(y).ok()?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect { Rect { left, top, right, bottom } }
    #[test]
    fn selects_screen_and_respects_taskbar() {
        let s = [Screen { bounds: rect(0, 0, 1920, 1080), work: rect(0, 0, 1920, 1040) }, Screen { bounds: rect(-1280, 0, 0, 1024), work: rect(-1280, 0, 0, 984) }];
        assert_eq!(position(&s, (200, 200), (290, 298), 16), Some((0, (1614, 726))));
        assert_eq!(position(&s, (-500, 200), (290, 298), 16), Some((1, (-306, 670))));
        assert_eq!(position(&s, (5000, 0), (290, 298), 16), Some((0, (1614, 726))));
    }
    #[test]
    fn handles_upper_screen_and_scaled_size() {
        let s = [Screen { bounds: rect(0, -1440, 2560, 0), work: rect(0, -1400, 2560, 0) }];
        assert_eq!(position(&s, (500, -500), (580, 596), 32), Some((0, (1948, -628))));
    }
    #[test]
    fn rejects_missing_or_invalid_geometry() {
        let s = [Screen { bounds: rect(0, 0, 100, 100), work: rect(0, 0, 100, 100) }];
        assert_eq!(position(&[], (0, 0), (1, 1), 0), None);
        assert_eq!(position(&s, (0, 0), (101, 1), 0), None);
        assert_eq!(position(&s, (0, 0), (0, 1), 0), None);
        assert_eq!(position(&[Screen { bounds: s[0].bounds, work: rect(0, 0, 0, 1) }], (0, 0), (1, 1), 0), None);
    }
    #[test]
    fn clamps_large_margin_and_accepts_boundary() {
        let s = [Screen { bounds: rect(-100, -100, 0, 0), work: rect(-100, -100, 0, 0) }];
        assert_eq!(position(&s, (-100, -100), (20, 20), u32::MAX), Some((0, (-100, -100))));
    }
}
