use egui::{Pos2, Rect, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    TopLeft,
    TopCenter,
    TopRight,
    RightCenter,
    BottomRight,
    BottomCenter,
    BottomLeft,
    LeftCenter,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DragState {
    Idle,
    Creating { start: Pos2 },
    Moving { original_rect: Rect, grab_offset: Vec2 },
    Resizing { handle: ResizeHandle, original_rect: Rect, grab_pos: Pos2 },
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub min: Pos2,
    pub max: Pos2,
}

impl Selection {
    pub fn new(p1: Pos2, p2: Pos2) -> Self {
        let min_x = p1.x.min(p2.x);
        let max_x = p1.x.max(p2.x);
        let min_y = p1.y.min(p2.y);
        let max_y = p1.y.max(p2.y);

        Self {
            min: Pos2::new(min_x, min_y),
            max: Pos2::new(max_x, max_y),
        }
    }

    pub fn to_rect(&self) -> Rect {
        Rect::from_min_max(self.min, self.max)
    }

    pub fn width(&self) -> f32 {
        (self.max.x - self.min.x).abs()
    }

    pub fn height(&self) -> f32 {
        (self.max.y - self.min.y).abs()
    }

    pub fn is_valid(&self) -> bool {
        self.width() >= 4.0 && self.height() >= 4.0
    }

    /// Computes handle positions for resizing.
    pub fn get_handles(&self) -> [(ResizeHandle, Pos2); 8] {
        let rect = self.to_rect();
        [
            (ResizeHandle::TopLeft, rect.left_top()),
            (ResizeHandle::TopCenter, rect.center_top()),
            (ResizeHandle::TopRight, rect.right_top()),
            (ResizeHandle::RightCenter, rect.right_center()),
            (ResizeHandle::BottomRight, rect.right_bottom()),
            (ResizeHandle::BottomCenter, rect.center_bottom()),
            (ResizeHandle::BottomLeft, rect.left_bottom()),
            (ResizeHandle::LeftCenter, rect.left_center()),
        ]
    }

    /// Finds if mouse position hits any resize handle (within radius).
    pub fn hit_test_handle(&self, pos: Pos2, radius: f32) -> Option<ResizeHandle> {
        for (handle, handle_pos) in self.get_handles() {
            if handle_pos.distance(pos) <= radius {
                return Some(handle);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_normalization() {
        // Drag from bottom-right (200, 200) to top-left (100, 100)
        let sel = Selection::new(Pos2::new(200.0, 200.0), Pos2::new(100.0, 100.0));
        assert_eq!(sel.min, Pos2::new(100.0, 100.0));
        assert_eq!(sel.max, Pos2::new(200.0, 200.0));
        assert_eq!(sel.width(), 100.0);
        assert_eq!(sel.height(), 100.0);
        assert!(sel.is_valid());
    }

    #[test]
    fn test_selection_handles_hit_test() {
        let sel = Selection::new(Pos2::new(100.0, 100.0), Pos2::new(200.0, 200.0));
        // Hit top-left corner
        let hit = sel.hit_test_handle(Pos2::new(102.0, 99.0), 5.0);
        assert_eq!(hit, Some(ResizeHandle::TopLeft));

        // Hit bottom-right corner
        let hit = sel.hit_test_handle(Pos2::new(198.0, 201.0), 5.0);
        assert_eq!(hit, Some(ResizeHandle::BottomRight));

        // Center should not hit any handle
        let hit = sel.hit_test_handle(Pos2::new(150.0, 150.0), 5.0);
        assert_eq!(hit, None);
    }
}
