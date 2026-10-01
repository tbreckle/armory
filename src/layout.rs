//! Placement of the items of a tab: tiles (buttons, tile labels) wrap into rows, full-width
//! items (dividers, labels) take a row of their own. Also the page-wise scrolling, which keeps
//! rows whole because a light gun can't use a scrollbar.

/// Size request of one item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Spans the whole width.
    Full { height: f32 },
    /// Fixed size, wraps with the other tiles.
    Tile { width: f32, height: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// One rectangle per shape, in the same order.
    pub rects: Vec<Rect>,
    /// Height of the content.
    pub height: f32,
}

impl Layout {
    /// Places `shapes` into `width`, with `spacing` between rows and between tiles.
    pub fn new(shapes: &[Shape], width: f32, spacing: f32) -> Self {
        let width = width.max(0.0);
        let mut rects = Vec::with_capacity(shapes.len());
        let (mut x, mut y, mut row_height) = (0.0_f32, 0.0_f32, 0.0_f32);
        let mut row_open = false;

        for shape in shapes {
            match *shape {
                Shape::Full { height } => {
                    if row_open {
                        y += row_height + spacing;
                        row_open = false;
                    }
                    rects.push(Rect {
                        x: 0.0,
                        y,
                        width,
                        height,
                    });
                    y += height + spacing;
                }
                Shape::Tile {
                    width: tile_width,
                    height,
                } => {
                    let tile_width = tile_width.min(width);
                    if row_open && x + tile_width > width {
                        y += row_height + spacing;
                        row_open = false;
                    }
                    if !row_open {
                        x = 0.0;
                        row_height = 0.0;
                        row_open = true;
                    }
                    rects.push(Rect {
                        x,
                        y,
                        width: tile_width,
                        height,
                    });
                    x += tile_width + spacing;
                    row_height = row_height.max(height);
                }
            }
        }

        let height = if row_open {
            y + row_height
        } else {
            (y - spacing).max(0.0)
        };
        Self { rects, height }
    }

    /// Largest useful scroll offset for a view of `view_height`.
    pub fn max_offset(&self, view_height: f32) -> f32 {
        (self.height - view_height).max(0.0)
    }

    pub fn clamp_offset(&self, offset: f32, view_height: f32) -> f32 {
        offset.clamp(0.0, self.max_offset(view_height))
    }

    /// Offset one page further down: the first item cut off at the bottom moves to the top.
    /// An item taller than the view scrolls by a full view height instead.
    pub fn page_down(&self, offset: f32, view_height: f32) -> f32 {
        let view_bottom = offset + view_height;
        let target = self
            .rects
            .iter()
            .filter(|r| r.bottom() > view_bottom + 0.5)
            .map(|r| if r.y > offset + 0.5 { r.y } else { view_bottom })
            .fold(f32::INFINITY, f32::min);
        let target = if target.is_finite() {
            target
        } else {
            view_bottom
        };
        self.clamp_offset(target, view_height)
    }

    /// Offset one page further up, snapped so that the item at the top is not cut off.
    pub fn page_up(&self, offset: f32, view_height: f32) -> f32 {
        let target = (offset - view_height).max(0.0);
        let snapped = self
            .rects
            .iter()
            .filter(|r| r.y < target && r.bottom() > target)
            .map(|r| r.y)
            .fold(target, f32::min);
        self.clamp_offset(snapped, view_height)
    }

    /// Offset one row further down (mouse wheel): the next row moves to the top.
    /// Moves at most a view height, so items taller than the view are passed in steps.
    pub fn row_down(&self, offset: f32, view_height: f32) -> f32 {
        let target = self
            .rects
            .iter()
            .map(|r| r.y)
            .filter(|&y| y > offset + 0.5)
            .fold(offset + view_height, f32::min);
        self.clamp_offset(target, view_height)
    }

    /// Offset one row further up (mouse wheel): the row above, or a cut-off row, moves to the
    /// top. Moves at most a view height.
    pub fn row_up(&self, offset: f32, view_height: f32) -> f32 {
        let target = self
            .rects
            .iter()
            .map(|r| r.y)
            .filter(|&y| y < offset - 0.5)
            .fold(offset - view_height, f32::max);
        self.clamp_offset(target, view_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Shape = Shape::Tile {
        width: 100.0,
        height: 50.0,
    };
    const FULL: Shape = Shape::Full { height: 20.0 };

    fn ys(layout: &Layout) -> Vec<f32> {
        layout.rects.iter().map(|r| r.y).collect()
    }

    #[test]
    fn tiles_wrap() {
        // 3 tiles of 100 + 10 spacing fit into 320.
        let layout = Layout::new(&[TILE; 4], 320.0, 10.0);
        let xs: Vec<f32> = layout.rects.iter().map(|r| r.x).collect();
        assert_eq!(xs, [0.0, 110.0, 220.0, 0.0]);
        assert_eq!(ys(&layout), [0.0, 0.0, 0.0, 60.0]);
        assert_eq!(layout.height, 110.0);
    }

    #[test]
    fn full_items_break_rows() {
        let layout = Layout::new(&[FULL, TILE, TILE, FULL, TILE], 1000.0, 10.0);
        assert_eq!(ys(&layout), [0.0, 30.0, 30.0, 90.0, 120.0]);
        assert_eq!(layout.rects[0].width, 1000.0);
        assert_eq!(layout.height, 170.0);
    }

    #[test]
    fn trailing_full_item_has_no_spacing() {
        let layout = Layout::new(&[TILE, FULL], 1000.0, 10.0);
        assert_eq!(layout.height, 80.0);
        assert_eq!(Layout::new(&[], 1000.0, 10.0).height, 0.0);
    }

    #[test]
    fn tiles_wider_than_the_view_shrink() {
        let layout = Layout::new(&[TILE, TILE], 60.0, 10.0);
        assert_eq!(layout.rects[0].width, 60.0);
        assert_eq!(ys(&layout), [0.0, 60.0]);
    }

    #[test]
    fn paging_keeps_rows_whole() {
        // One tile per row: rows at 0, 60, 120, ..., 540; height 590.
        let layout = Layout::new(&[TILE; 10], 100.0, 10.0);
        // A view of 150 shows rows 0 and 60 fully, row 120 is cut.
        assert_eq!(layout.page_down(0.0, 150.0), 120.0);
        assert_eq!(layout.page_down(120.0, 150.0), 240.0);
        // The last page is clamped to the end.
        assert_eq!(layout.page_down(420.0, 150.0), 440.0);
        assert_eq!(layout.page_down(440.0, 150.0), 440.0);
        // Up: 240 - 150 = 90 cuts row 60, so snap to 60.
        assert_eq!(layout.page_up(240.0, 150.0), 60.0);
        assert_eq!(layout.page_up(60.0, 150.0), 0.0);
    }

    #[test]
    fn wheel_scrolls_row_by_row() {
        // Rows at 0, 30 (two tiles), 90, 120; height 170.
        let layout = Layout::new(&[FULL, TILE, TILE, FULL, TILE], 1000.0, 10.0);
        assert_eq!(layout.row_down(0.0, 100.0), 30.0);
        assert_eq!(layout.row_down(30.0, 100.0), 70.0);
        // Clamped to the end, which cuts the row at 30 at the top.
        assert_eq!(layout.row_down(70.0, 100.0), 70.0);
        assert_eq!(layout.row_up(70.0, 100.0), 30.0);
        assert_eq!(layout.row_up(30.0, 100.0), 0.0);
        assert_eq!(layout.row_up(0.0, 100.0), 0.0);
    }

    #[test]
    fn wheel_steps_through_items_taller_than_the_view() {
        let layout = Layout::new(
            &[
                TILE,
                Shape::Tile {
                    width: 100.0,
                    height: 500.0,
                },
                TILE,
            ],
            100.0,
            10.0,
        );
        // Rows at 0, 60, 570; height 620.
        assert_eq!(layout.row_down(0.0, 200.0), 60.0);
        assert_eq!(layout.row_down(60.0, 200.0), 260.0);
        assert_eq!(layout.row_down(260.0, 200.0), 420.0);
        assert_eq!(layout.row_up(420.0, 200.0), 220.0);
        assert_eq!(layout.row_up(220.0, 200.0), 60.0);
    }

    #[test]
    fn paging_progresses_past_items_taller_than_the_view() {
        let layout = Layout::new(
            &[
                Shape::Tile {
                    width: 100.0,
                    height: 500.0,
                },
                TILE,
            ],
            100.0,
            10.0,
        );
        assert_eq!(layout.page_down(0.0, 200.0), 200.0);
    }

    #[test]
    fn no_scrolling_when_everything_fits() {
        let layout = Layout::new(&[TILE; 2], 100.0, 10.0);
        assert_eq!(layout.max_offset(500.0), 0.0);
        assert_eq!(layout.page_down(0.0, 500.0), 0.0);
    }
}
