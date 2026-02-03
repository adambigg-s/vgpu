#![allow(dead_code)]

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
    pub fn from_points(points: [glam::Vec2; 3]) -> Self {
        let [a, b, c] = points;
        Self { a, b, c, ba: b - a, cb: c - b, ac: a - c }
    }

    pub fn sample_point(&self, point: glam::Vec2) -> glam::Vec3 {
        let [ap, bp, cp] = [point - self.a, point - self.b, point - self.c];
        let [apb, bpc, cpa] = [self.ba.perp_dot(ap), self.cb.perp_dot(bp), self.ac.perp_dot(cp)];
        glam::vec3(bpc, cpa, apb) / (bpc + cpa + apb)
    }

    pub fn surrounds(&self, lambdas: glam::Vec3) -> bool {
        lambdas.x >= 0.0 && lambdas.y >= 0.0 && lambdas.z >= 0.0
    }
}

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

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vector<T, const N: usize> {
    items: [T; N],
}

impl<T, const N: usize> Vector<T, N> {
    pub fn to_array(self) -> [T; N] {
        self.items
    }
}

impl<T, const N: usize> Default for Vector<T, N>
where
    T: Default + Clone + Copy,
{
    fn default() -> Self {
        Self { items: [T::default(); N] }
    }
}

impl<T, const N: usize> From<[T; N]> for Vector<T, N> {
    fn from(items: [T; N]) -> Self {
        Self { items }
    }
}

impl<T, const N: usize> From<Vector<T, N>> for [T; N] {
    fn from(value: Vector<T, N>) -> Self {
        value.items
    }
}

impl<T, const N: usize> ops::Add for Vector<T, N>
where
    T: Clone + Copy + ops::Add<Output = T>,
{
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        (0..N).for_each(|i| {
            self.items[i] = self.items[i] + rhs.items[i];
        });
        self
    }
}

impl<T, D, const N: usize> ops::Mul<D> for Vector<T, N>
where
    T: Clone + Copy + ops::Mul<D, Output = T>,
    D: Clone + Copy,
{
    type Output = Self;

    fn mul(mut self, rhs: D) -> Self::Output {
        for i in 0..N {
            self.items[i] = self.items[i] * rhs;
        }
        self
    }
}

impl<T, const N: usize> ops::Deref for Vector<T, N> {
    type Target = [T; N];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl<T, const N: usize> ops::DerefMut for Vector<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}

#[cfg(test)]
mod tests {
    use crate::interp::{self, weighted_sum};

    #[test]
    fn weighted_sum_scalar() {
        let values = [1, 2, 3];
        let weights = [1, 2, 3];
        assert!(weighted_sum(values, weights) == 14);
    }

    #[test]
    fn weighted_sum_vector() {
        let values = vec![
            glam::vec3(1.0, 1.0, 1.0),
            glam::vec3(1.0, 1.0, 1.0),
            glam::vec3(1.0, 1.0, 1.0),
        ];
        let weights = [1.0, 1.0, 1.0];
        assert!(weighted_sum(values, weights) == glam::vec3(3.0, 3.0, 3.0,));
    }

    #[test]
    fn vector_add() {
        let v1 = [1, 2, 3];
        let v2 = [1, 2, 3];
        assert!((interp::Vector::from(v1) + interp::Vector::from(v2)) == [2, 4, 6].into());
    }

    #[test]
    fn vector_mul() {
        let v = [1, 2, 3];
        let s = 10;
        assert!(interp::Vector::from(v) * s == [10, 20, 30].into());
    }

    #[test]
    fn vector_transparency() {
        let v = [1, 2, 3, 4, 5];
        assert!(size_of_val(&v) == size_of_val(&interp::Vector::from(v)));
    }
}
