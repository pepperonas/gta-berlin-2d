use glam::Vec2;

/// Map coordinates retain the Canvas convention: x east, y south, height up.
#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec2,
    pub zoom: f32,
    pub scale: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 2.0,
            scale: 1.0,
        }
    }
}
impl Camera {
    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = (self.zoom * factor).clamp(0.72, 2.6);
        }
    }
    pub fn world_to_screen(&self, point: Vec2, height: f32, viewport: Vec2) -> Vec2 {
        let delta = point - self.position;
        // render.js: h=max(18,height*0.5), dx=delta.x*h*0.0005,
        // dy=-h*0.5+delta.y*h*0.00025. Ground (height=0) has no offset.
        let h = if height > 0.0 {
            (height * 0.5).max(18.0)
        } else {
            0.0
        };
        let offset = Vec2::new(delta.x * h * 0.0005, -h * 0.5 + delta.y * h * 0.00025);
        viewport * 0.5 + (delta + offset) * self.zoom * self.scale
    }
    pub fn screen_to_ground(&self, screen: Vec2, viewport: Vec2) -> Vec2 {
        self.position + (screen - viewport * 0.5) / (self.zoom * self.scale)
    }
    pub(crate) fn uniform(&self, viewport: Vec2) -> [f32; 8] {
        [
            self.position.x,
            self.position.y,
            self.zoom * self.scale,
            0.0,
            viewport.x,
            viewport.y,
            0.0,
            0.0,
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ground_round_trip_and_center() {
        let cam = Camera {
            position: Vec2::new(1000.0, 500.0),
            ..Default::default()
        };
        let viewport = Vec2::new(1280.0, 720.0);
        assert_eq!(
            cam.world_to_screen(cam.position, 0.0, viewport),
            viewport * 0.5
        );
        let p = Vec2::new(1200.0, 420.0);
        assert!(
            cam.screen_to_ground(cam.world_to_screen(p, 0.0, viewport), viewport)
                .distance(p)
                < 0.001
        );
    }
    #[test]
    fn roof_matches_canvas_and_zoom_is_bounded() {
        let mut cam = Camera::default();
        assert_eq!(
            cam.world_to_screen(Vec2::ZERO, 40.0, Vec2::splat(100.0)),
            Vec2::new(50.0, 30.0)
        );
        cam.zoom_by(100.0);
        assert_eq!(cam.zoom, 2.6);
        cam.zoom_by(0.001);
        assert_eq!(cam.zoom, 0.72);
        cam.zoom_by(f32::NAN);
        assert_eq!(cam.zoom, 0.72);
    }
}
