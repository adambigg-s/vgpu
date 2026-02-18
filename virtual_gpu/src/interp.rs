use std::ops;

pub struct BarycentricSystem {
    a: glam::Vec2,
    b: glam::Vec2,
    c: glam::Vec2,
    ba: glam::Vec2,
    cb: glam::Vec2,
    ac: glam::Vec2,
}

impl BarycentricSystem {
    #[inline(always)]
    pub fn from_points(points: [glam::Vec2; 3]) -> Self {
        let [a, b, c] = points;
        Self { a, b, c, ba: b - a, cb: c - b, ac: a - c }
    }

    #[inline(always)]
    pub fn sample_point(&self, point: glam::Vec2) -> glam::Vec3 {
        let [ap, bp, cp] = [point - self.a, point - self.b, point - self.c];
        let [apb, bpc, cpa] = [self.ba.perp_dot(ap), self.cb.perp_dot(bp), self.ac.perp_dot(cp)];
        glam::vec3(bpc, cpa, apb) / (bpc + cpa + apb)
    }

    #[inline(always)]
    pub fn surrounds(&self, lambdas: glam::Vec3) -> bool {
        lambdas.x >= 0.0 && lambdas.y >= 0.0 && lambdas.z >= 0.0
    }
}

#[inline(always)]
pub fn weighted_sum<V, W, T, D>(values: V, weights: W) -> T
where
    T: Default + ops::Add<T, Output = T> + ops::Mul<D, Output = T>,
    V: IntoIterator<Item = T>,
    W: IntoIterator<Item = D>,
{
    values
        .into_iter()
        .zip(weights)
        .fold(T::default(), |accumulator, (val, weight)| accumulator + val * weight)
}

