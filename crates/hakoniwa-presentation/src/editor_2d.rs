use hakoniwa_domain::{GridPosition, Piece, Plane};

pub(crate) const PLATE_SIZE: i32 = 29;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GridPoint2d {
    pub(crate) u: i32,
    pub(crate) v: i32,
}

impl GridPoint2d {
    pub(crate) const fn new(u: i32, v: i32) -> Self {
        Self { u, v }
    }
}

pub(crate) const fn project_position(plane: Plane, position: GridPosition) -> GridPoint2d {
    match plane {
        Plane::Xy { .. } => GridPoint2d::new(position.x, position.y),
        Plane::Xz { .. } => GridPoint2d::new(position.x, position.z),
        Plane::Yz { .. } => GridPoint2d::new(position.y, position.z),
    }
}

pub(crate) const fn unproject_position(plane: Plane, point: GridPoint2d) -> GridPosition {
    match plane {
        Plane::Xy { z } => GridPosition::new(point.u, point.v, z),
        Plane::Xz { y } => GridPosition::new(point.u, y, point.v),
        Plane::Yz { x } => GridPosition::new(x, point.u, point.v),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlateLayout {
    pub(crate) min_u: i32,
    pub(crate) min_v: i32,
    pub(crate) columns: u32,
    pub(crate) rows: u32,
}

impl PlateLayout {
    pub(crate) const fn empty_piece() -> Self {
        Self {
            min_u: -14,
            min_v: -14,
            columns: 1,
            rows: 1,
        }
    }

    pub(crate) fn for_piece(piece: &Piece) -> Self {
        let mut points = piece
            .beads
            .keys()
            .map(|position| project_position(piece.plane, *position));
        let Some(first) = points.next() else {
            return Self::empty_piece();
        };

        let (mut min_u, mut max_u) = (first.u, first.u);
        let (mut min_v, mut max_v) = (first.v, first.v);
        for point in points {
            min_u = min_u.min(point.u);
            max_u = max_u.max(point.u);
            min_v = min_v.min(point.v);
            max_v = max_v.max(point.v);
        }

        Self {
            min_u,
            min_v,
            columns: plate_count(min_u, max_u),
            rows: plate_count(min_v, max_v),
        }
    }

    pub(crate) fn max_u(self) -> i32 {
        self.min_u + i32::try_from(self.columns).unwrap_or(i32::MAX) * PLATE_SIZE - 1
    }

    pub(crate) fn max_v(self) -> i32 {
        self.min_v + i32::try_from(self.rows).unwrap_or(i32::MAX) * PLATE_SIZE - 1
    }

    pub(crate) fn center(self) -> (f32, f32) {
        (
            midpoint(self.min_u, self.max_u()),
            midpoint(self.min_v, self.max_v()),
        )
    }

    /// Expands by complete plates and returns how far the lower-left origin moved.
    pub(crate) fn expand_to_include(&mut self, point: GridPoint2d) -> GridPoint2d {
        let old_min = GridPoint2d::new(self.min_u, self.min_v);

        if point.u < self.min_u {
            let added = div_ceil_positive(self.min_u.saturating_sub(point.u), PLATE_SIZE);
            self.min_u = self.min_u.saturating_sub(added.saturating_mul(PLATE_SIZE));
            self.columns = self.columns.saturating_add(added as u32);
        } else if point.u > self.max_u() {
            let added = div_ceil_positive(point.u.saturating_sub(self.max_u()), PLATE_SIZE);
            self.columns = self.columns.saturating_add(added as u32);
        }

        if point.v < self.min_v {
            let added = div_ceil_positive(self.min_v.saturating_sub(point.v), PLATE_SIZE);
            self.min_v = self.min_v.saturating_sub(added.saturating_mul(PLATE_SIZE));
            self.rows = self.rows.saturating_add(added as u32);
        } else if point.v > self.max_v() {
            let added = div_ceil_positive(point.v.saturating_sub(self.max_v()), PLATE_SIZE);
            self.rows = self.rows.saturating_add(added as u32);
        }

        GridPoint2d::new(self.min_u - old_min.u, self.min_v - old_min.v)
    }
}

fn plate_count(minimum: i32, maximum: i32) -> u32 {
    let span = i64::from(maximum) - i64::from(minimum) + 1;
    u32::try_from((span + i64::from(PLATE_SIZE) - 1) / i64::from(PLATE_SIZE))
        .unwrap_or(u32::MAX)
        .max(1)
}

fn div_ceil_positive(value: i32, divisor: i32) -> i32 {
    value / divisor + i32::from(value % divisor != 0)
}

fn midpoint(minimum: i32, maximum: i32) -> f32 {
    (minimum as f32 + maximum as f32) * 0.5
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Viewport2d {
    pub(crate) center_u: f32,
    pub(crate) center_v: f32,
    pub(crate) pixels_per_cell: f32,
}

impl Viewport2d {
    const MIN_PIXELS_PER_CELL: f32 = 2.0;
    const MAX_PIXELS_PER_CELL: f32 = 80.0;

    pub(crate) fn for_layout(
        layout: PlateLayout,
        available_width: f32,
        available_height: f32,
    ) -> Self {
        let (center_u, center_v) = layout.center();
        let width_in_cells = layout.columns as f32 * PLATE_SIZE as f32;
        let height_in_cells = layout.rows as f32 * PLATE_SIZE as f32;
        let pixels_per_cell =
            ((available_width / width_in_cells).min(available_height / height_in_cells) * 0.9)
                .clamp(Self::MIN_PIXELS_PER_CELL, Self::MAX_PIXELS_PER_CELL);
        Self {
            center_u,
            center_v,
            pixels_per_cell,
        }
    }

    pub(crate) fn grid_to_screen(self, point: (f32, f32), screen_center: (f32, f32)) -> (f32, f32) {
        (
            screen_center.0 + (point.0 - self.center_u) * self.pixels_per_cell,
            screen_center.1 - (point.1 - self.center_v) * self.pixels_per_cell,
        )
    }

    pub(crate) fn screen_to_grid(self, point: (f32, f32), screen_center: (f32, f32)) -> (f32, f32) {
        (
            self.center_u + (point.0 - screen_center.0) / self.pixels_per_cell,
            self.center_v - (point.1 - screen_center.1) / self.pixels_per_cell,
        )
    }

    pub(crate) fn screen_to_cell(
        self,
        point: (f32, f32),
        screen_center: (f32, f32),
    ) -> GridPoint2d {
        let grid = self.screen_to_grid(point, screen_center);
        GridPoint2d::new(grid.0.round() as i32, grid.1.round() as i32)
    }

    pub(crate) fn pan_pixels(&mut self, delta_x: f32, delta_y: f32) {
        self.center_u -= delta_x / self.pixels_per_cell;
        self.center_v += delta_y / self.pixels_per_cell;
    }

    pub(crate) fn zoom_at(&mut self, factor: f32, pointer: (f32, f32), screen_center: (f32, f32)) {
        let anchor = self.screen_to_grid(pointer, screen_center);
        self.pixels_per_cell = (self.pixels_per_cell * factor)
            .clamp(Self::MIN_PIXELS_PER_CELL, Self::MAX_PIXELS_PER_CELL);
        let after = self.screen_to_grid(pointer, screen_center);
        self.center_u += anchor.0 - after.0;
        self.center_v += anchor.1 - after.1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hakoniwa_domain::{Bead, Color};

    fn piece(plane: Plane, positions: impl IntoIterator<Item = GridPosition>) -> Piece {
        Piece::new(
            1,
            "test",
            plane,
            positions.into_iter().map(|position| Bead {
                position,
                color: Color::RED,
            }),
        )
        .unwrap()
    }

    #[test]
    fn projection_round_trips_on_every_piece_plane() {
        for (plane, position) in [
            (Plane::Xy { z: 7 }, GridPosition::new(-3, 5, 7)),
            (Plane::Xz { y: -2 }, GridPosition::new(-3, -2, 5)),
            (Plane::Yz { x: 9 }, GridPosition::new(9, -3, 5)),
        ] {
            let projected = project_position(plane, position);
            assert_eq!(unproject_position(plane, projected), position);
        }
    }

    #[test]
    fn empty_piece_uses_one_plate_centered_on_the_origin() {
        let layout = PlateLayout::for_piece(&piece(Plane::Xy { z: 0 }, []));
        assert_eq!(layout, PlateLayout::empty_piece());
        assert_eq!((layout.min_u, layout.max_u()), (-14, 14));
        assert_eq!((layout.min_v, layout.max_v()), (-14, 14));
    }

    #[test]
    fn existing_piece_uses_the_minimum_number_of_plates() {
        let layout = PlateLayout::for_piece(&piece(
            Plane::Xy { z: 0 },
            [GridPosition::new(14, 2, 0), GridPosition::new(42, 31, 0)],
        ));
        assert_eq!(layout.min_u, 14);
        assert_eq!(layout.min_v, 2);
        assert_eq!(layout.columns, 1);
        assert_eq!(layout.rows, 2);
        assert_eq!(layout.max_u(), 42);
        assert_eq!(layout.max_v(), 59);
    }

    #[test]
    fn expansion_adds_connected_complete_plates_in_every_direction() {
        let mut layout = PlateLayout::empty_piece();
        let moved = layout.expand_to_include(GridPoint2d::new(-44, 73));
        assert_eq!(moved, GridPoint2d::new(-58, 0));
        assert_eq!((layout.min_u, layout.max_u()), (-72, 14));
        assert_eq!((layout.min_v, layout.max_v()), (-14, 101));
        assert_eq!((layout.columns, layout.rows), (3, 4));
    }

    #[test]
    fn viewport_round_trips_cells_and_keeps_zoom_anchor_fixed() {
        let layout = PlateLayout::empty_piece();
        let mut viewport = Viewport2d::for_layout(layout, 580.0, 580.0);
        let screen_center = (300.0, 250.0);
        let point = GridPoint2d::new(8, -11);
        let screen = viewport.grid_to_screen((point.u as f32, point.v as f32), screen_center);
        assert_eq!(viewport.screen_to_cell(screen, screen_center), point);

        let pointer = (420.0, 190.0);
        let before = viewport.screen_to_grid(pointer, screen_center);
        viewport.zoom_at(1.75, pointer, screen_center);
        let after = viewport.screen_to_grid(pointer, screen_center);
        assert!((before.0 - after.0).abs() < 0.000_1);
        assert!((before.1 - after.1).abs() < 0.000_1);
    }

    #[test]
    fn panning_moves_the_view_without_changing_scale() {
        let mut viewport = Viewport2d::for_layout(PlateLayout::empty_piece(), 580.0, 580.0);
        let scale = viewport.pixels_per_cell;
        viewport.pan_pixels(scale * 3.0, scale * -2.0);
        assert!((viewport.center_u + 3.0).abs() < 0.000_1);
        assert!((viewport.center_v + 2.0).abs() < 0.000_1);
        assert_eq!(viewport.pixels_per_cell, scale);
    }
}
