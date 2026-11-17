//! 二维几何：设备坐标与路径长度。

use std::ops::{Add, Sub};

/// 二维点/向量，单位米。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// 欧氏距离（米）。
    pub fn distance(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// 限制在矩形场地 [0, max_x] × [0, max_y] 内。
    pub fn clamp(self, max_x: f64, max_y: f64) -> Self {
        Self {
            x: self.x.clamp(0.0, max_x),
            y: self.y.clamp(0.0, max_y),
        }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_basic() {
        assert!((Vec2::new(0.0, 0.0).distance(Vec2::new(3.0, 4.0)) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn triangle_inequality_models_relay_detour() {
        // 车、钥匙相距 8m，中继放在两者侧面：转发路径必然比直连长。
        let vehicle = Vec2::new(0.0, 0.0);
        let key = Vec2::new(8.0, 0.0);
        let relay = Vec2::new(4.0, 3.0);
        let direct = vehicle.distance(key);
        let relayed = vehicle.distance(relay) + relay.distance(key);
        assert!(relayed > direct);
        assert!((relayed - 10.0).abs() < 1e-12);
    }
}
