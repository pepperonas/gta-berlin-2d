//! GRS80 transverse Mercator / Krüger series matching projection.js.
use crate::format::Meta;
use glam::DVec2;
pub struct Projection {
    lon0: f64,
    y0: f64,
    a: f64,
    e: f64,
    alpha: [f64; 3],
}
impl Projection {
    pub fn new(lat0: f64, lon0: f64) -> Self {
        let f = 1.0 / 298.257222101;
        let n = f / (2.0 - f);
        let n2 = n * n;
        let n3 = n2 * n;
        let mut p = Self {
            lon0,
            y0: 0.0,
            a: 6378137.0 / (1.0 + n) * (1.0 + n2 / 4.0 + n2 * n2 / 64.0),
            e: (f * (2.0 - f)).sqrt(),
            alpha: [
                n / 2.0 - 2.0 * n2 / 3.0 + 5.0 * n3 / 16.0,
                13.0 * n2 / 48.0 - 3.0 * n3 / 5.0,
                61.0 * n3 / 240.0,
            ],
        };
        p.y0 = p.raw(lat0, lon0).y;
        p
    }
    fn raw(&self, lat: f64, lon: f64) -> DVec2 {
        let phi = lat.to_radians();
        let lam = (lon - self.lon0).to_radians();
        let t = (phi.sin().atanh() - self.e * (self.e * phi.sin()).atanh()).sinh();
        let xi = t.atan2(lam.cos());
        let eta = (lam.sin() / (1.0 + t * t).sqrt()).atanh();
        let mut v = DVec2::new(eta, xi);
        for (i, &a) in self.alpha.iter().enumerate() {
            let j = 2.0 * (i + 1) as f64;
            v.x += a * (j * xi).cos() * (j * eta).sinh();
            v.y += a * (j * xi).sin() * (j * eta).cosh();
        }
        v * self.a
    }
    pub fn meters(&self, lat: f64, lon: f64) -> DVec2 {
        self.raw(lat, lon) - DVec2::new(0.0, self.y0)
    }
}
pub fn geo_to_px(meta: &Meta, lat: f64, lon: f64) -> DVec2 {
    let p = Projection::new(meta.origin.lat0, meta.origin.lon0);
    let [s, w, n, e] = meta.origin.bbox;
    let corners = [
        p.meters(s, w),
        p.meters(s, e),
        p.meters(n, w),
        p.meters(n, e),
    ];
    let min_x = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let v = p.meters(lat, lon);
    // JS Math.round uses floor(x + .5), including negative halves.
    DVec2::new(
        ((v.x - min_x) * meta.scale as f64 + 0.5).floor(),
        ((max_y - v.y) * meta.scale as f64 + 0.5).floor(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_and_js_reference() {
        let p = Projection::new(52.506876635, 13.424753365);
        assert!(p.meters(52.506876635, 13.424753365).length() < 1e-8);
        let reference = DVec2::new(-9730.444360619662, -19004.760987591);
        assert!(p.meters(52.336, 13.282).distance(reference) < 0.001);
    }
}
