use crate::vec3x8::Vec3x8;

pub struct Rayx8 {
    pub origin: Vec3x8,
    pub direction: Vec3x8,
}

impl Rayx8 {
    pub fn new(origin: Vec3x8, direction: Vec3x8) -> Self {
        Rayx8 { origin, direction }
    }

    pub fn at(&self, t: wide::f32x8) -> Vec3x8 {
        self.origin + self.direction * t
    }
}
