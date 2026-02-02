#![allow(dead_code)]

pub type Raster<T> = buffer::Buffer<T, 2>;

impl<T> Default for Raster<T> {
    fn default() -> Self {
        Self::new([0, 0])
    }
}

unsafe impl<T> Send for Raster<T> {}

unsafe impl<T> Sync for Raster<T> {}

pub type Array<T> = buffer::Buffer<T, 1>;

impl<T> Default for Array<T> {
    fn default() -> Self {
        Self::new([0])
    }
}

pub mod buffer {
    use std::{convert, mem, ops};

    #[derive(Debug)]
    pub struct Buffer<T, const N: usize> {
        size: [usize; N],
        items: Box<[T]>,
    }

    impl<T, const N: usize> Buffer<T, N> {
        pub fn new(size: [usize; N]) -> Self {
            Self {
                size,
                items: unsafe {
                    mem::transmute::<std::boxed::Box<[std::mem::MaybeUninit<T>]>, std::boxed::Box<[T]>>(
                        Box::<[T]>::new_uninit_slice(size.iter().product()),
                    )
                },
            }
        }

        pub fn from_parts<S>(size: [usize; N], items: S) -> Self
        where
            Box<[T]>: convert::From<S>,
            S: AsRef<[T]>,
        {
            debug_assert!(size.iter().product::<usize>() == items.as_ref().len());
            Self { size, items: items.into() }
        }

        pub fn size(&self) -> [usize; N] {
            self.size
        }

        pub fn fill(&mut self, fill: T)
        where
            T: Copy,
        {
            self.items.iter_mut().for_each(|item| *item = fill);
        }

        pub fn try_get(&self, indices: [usize; N]) -> Option<&T> {
            if !self.surrounds(indices) {
                return None;
            }
            Some(self.get(indices))
        }

        pub fn get(&self, indices: [usize; N]) -> &T {
            let idx = self.linearlize(indices);
            &self.items[idx]
        }

        pub fn try_get_mut(&mut self, indices: [usize; N]) -> Option<&mut T> {
            if !self.surrounds(indices) {
                return None;
            }
            Some(self.get_mut(indices))
        }

        pub fn get_mut(&mut self, indices: [usize; N]) -> &mut T {
            let idx = self.linearlize(indices);
            &mut self.items[idx]
        }

        pub fn linearlize(&self, indices: [usize; N]) -> usize {
            debug_assert!(self.surrounds(indices));
            let mut index = 0;
            let mut stride = 1;
            (0..N).for_each(|dim| {
                index += indices[dim] * stride;
                stride *= self.size[dim];
            });
            index
        }

        pub fn surrounds(&self, indices: [usize; N]) -> bool {
            (0..N).all(|idx| indices[idx] < self.size[idx])
        }
    }

    impl<T, const N: usize> ops::Deref for Buffer<T, N> {
        type Target = Box<[T]>;

        fn deref(&self) -> &Self::Target {
            &self.items
        }
    }

    impl<T, const N: usize> ops::DerefMut for Buffer<T, N> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.items
        }
    }
}

pub mod stack {
    use std::{mem, ops};

    pub struct Vec<T, const N: usize> {
        len: usize,
        items: [mem::MaybeUninit<T>; N],
    }

    impl<T, const N: usize> Vec<T, N> {
        const ITEM: mem::MaybeUninit<T> = mem::MaybeUninit::uninit();
        const ITEMS: [mem::MaybeUninit<T>; N] = [Self::ITEM; N];

        pub fn new() -> Self {
            Self { len: Default::default(), items: Self::ITEMS }
        }

        pub fn len(&self) -> usize {
            self.len
        }

        pub fn push(&mut self, item: T) {
            debug_assert!(self.len != N);
            self.items[self.len] = mem::MaybeUninit::new(item);
            self.len += 1;
        }

        pub fn pop(&mut self) -> T {
            debug_assert!(self.len != 0);
            self.len -= 1;
            unsafe { self.items[self.len].assume_init_read() }
        }
    }

    impl<T, const N: usize> ops::Index<usize> for Vec<T, N> {
        type Output = T;

        fn index(&self, index: usize) -> &Self::Output {
            unsafe { self.items[index].assume_init_ref() }
        }
    }

    impl<T, const N: usize> ops::IndexMut<usize> for Vec<T, N> {
        fn index_mut(&mut self, index: usize) -> &mut Self::Output {
            unsafe { self.items[index].assume_init_mut() }
        }
    }

    impl<'d, T, const N: usize> IntoIterator for &'d Vec<T, N> {
        type Item = &'d T;

        type IntoIter = VecIter<'d, T, N>;

        fn into_iter(self) -> Self::IntoIter {
            VecIter { inner: self, idx: Default::default() }
        }
    }

    impl<T, const N: usize> FromIterator<T> for Vec<T, N> {
        fn from_iter<A>(iter: A) -> Self
        where
            A: IntoIterator<Item = T>,
        {
            let mut out = Self::new();
            iter.into_iter().for_each(|item| {
                out.push(item);
            });
            out
        }
    }

    pub struct VecIter<'d, T, const N: usize> {
        inner: &'d Vec<T, N>,
        idx: usize,
    }

    impl<'d, T, const N: usize> Iterator for VecIter<'d, T, N> {
        type Item = &'d T;

        fn next(&mut self) -> Option<Self::Item> {
            if self.idx < self.inner.len() {
                let item = unsafe { self.inner.items[self.idx].assume_init_ref() };
                self.idx += 1;
                return Some(item);
            }

            None
        }
    }
}
