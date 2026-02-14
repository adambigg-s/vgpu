pub const REGISTER_SIZE: usize = 12;
pub const VAO_SIZE: usize = 1;
pub const TILE_SIZE: usize = 64;

pub mod cores {
    use std::sync::atomic;

    use crate::{
        gpu, interp,
        memory::{self, transmute},
        shader,
        vgpu::{self, aabb},
    };

    pub type FloatRegister = interp::Vector<f32, { vgpu::REGISTER_SIZE }>;

    #[derive(Default, Debug)]
    pub struct VertexCore {}

    impl VertexCore {
        pub fn work<S>(
            &self,
            queue: &mut memory::Array<gpu::ProcessedVertex>,
            scheduler: &gpu::Scheduler,
            vao: &memory::Array<f32>,
            layout: &gpu::VaoPointer,
            program: &S,
        ) where
            S: shader::Shader,
        {
            debug_assert!(
                size_of::<S::Vertex>() < size_of::<FloatRegister>()
                    && size_of::<S::Interpolant>() < size_of::<FloatRegister>(),
                "Vertex attributes are too large for core buffers"
            );
            loop {
                let index = scheduler.head.fetch_add(1, atomic::Ordering::Relaxed);
                let head = index * layout.stride;
                let tail = head + layout.stride;
                if tail > vao.len() {
                    break;
                }

                let data = &vao[head..tail];
                let vertex = transmute::bit_interp::<&[f32], &S::Vertex>(&data);

                let mut position = glam::Vec4::default();
                let vs = program.vertex(vertex, &mut position);
                let attributes = transmute::bit_interp::<S::Interpolant, FloatRegister>(&vs);

                queue[index] = gpu::ProcessedVertex { attribs: attributes, pos: position }
            }
        }
    }

    #[derive(Default, Debug)]
    pub struct RasterCore {
        tile: Tile,
    }

    pub type Tile = aabb::AaBb<f32, 2>;
}

pub mod aabb {
    use std::cmp;

    #[derive(Debug)]
    pub struct AaBb<T, const N: usize> {
        low: [T; N],
        high: [T; N],
    }

    impl<T, const N: usize> AaBb<T, N> {
        pub fn new(low: [T; N], high: [T; N]) -> Self {
            Self { low, high }
        }

        pub fn overlaps(&self, other: &Self) -> bool
        where
            T: cmp::PartialOrd,
        {
            (0..N).all(|dim| self.low[dim] <= other.high[dim] && self.high[dim] >= other.low[dim])
        }
    }

    impl<T, const N: usize> Default for AaBb<T, N>
    where
        T: Default + Clone + Copy,
    {
        fn default() -> Self {
            Self::new([T::default(); N], [T::default(); N])
        }
    }
}

pub mod gpu {
    use std::{clone, sync::atomic, thread};

    use crate::{
        memory::{self, stack},
        shader,
        vgpu::{self, cores},
    };

    #[derive(Default, Debug)]
    pub struct VaoPointer {
        pub stride: usize,
    }

    #[derive(Default, Debug)]
    pub struct Scheduler {
        pub head: atomic::AtomicUsize,
    }

    impl Scheduler {
        pub fn reset(&self) {
            self.head.store(Default::default(), atomic::Ordering::Relaxed);
        }

        pub fn incomplete(&self, data: &memory::Array<f32>) -> bool {
            self.head.load(atomic::Ordering::Relaxed) < data.len()
        }

        pub fn vertex_stage<S>(
            &self,
            program: &S,
            cores: &mut memory::Array<cores::VertexCore>,
            queue: &mut memory::Array<ProcessedVertex>,
            vao: &memory::Array<f32>,
            layout: &VaoPointer,
        ) where
            S: shader::Shader + Send + Sync,
        {
            *queue = memory::Array::new([vao.len() / layout.stride]);
            self.reset();
            #[allow(invalid_reference_casting)]
            thread::scope(|scope| {
                for core in cores.iter() {
                    scope.spawn(|| unsafe {
                        let queue = queue as *const memory::Array<ProcessedVertex>
                            as *mut memory::Array<ProcessedVertex>;
                        core.work(&mut *queue, self, vao, layout, program);
                    });
                }
            })
        }

        pub fn raster_stage<S, P, D>(
            &self,
            program: &S,
            cores: &mut memory::Array<cores::RasterCore>,
            color: &mut P,
            depth: &mut D,
            queue: &memory::Array<ProcessedVertex>,
        ) where
            S: shader::Shader + Send + Sync,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
        }
    }

    #[derive(Default, Debug)]
    pub struct ProcessedVertex {
        pub attribs: cores::FloatRegister,
        pub pos: glam::Vec4,
    }

    #[derive(Default, Debug)]
    pub struct Gpu<P, D> {
        pub vertex_cores: memory::Array<cores::VertexCore>,
        pub raster_cores: memory::Array<cores::RasterCore>,
        pub vscheduler: Scheduler,
        pub rscheduler: Scheduler,

        pub vao_layout: stack::Vec<VaoPointer, { vgpu::VAO_SIZE }>,
        pub vao_raw: memory::Array<f32>,
        pub vao_queue: memory::Array<ProcessedVertex>,

        pub color: P,
        pub depth: D,
    }

    impl<P, D> Gpu<P, D>
    where
        P: Default + Send + Sync,
        D: Default + Send + Sync,
    {
        pub fn new(vcores: usize, rcores: usize) -> Self {
            Self {
                vertex_cores: memory::Array::new([vcores]),
                raster_cores: memory::Array::new([rcores]),
                ..Default::default()
            }
        }

        pub fn bind_data(&mut self, data: &Vec<f32>) {
            self.vao_raw = memory::Array::from_parts([data.len()], clone::Clone::clone(data));
        }

        pub fn set_vattrib_ptr(&mut self, stride: usize) {
            if !self.vao_layout.is_empty() {
                self.vao_layout.pop();
            }
            debug_assert!(self.vao_layout.len() < self.vao_layout.capacity());
            self.vao_layout.push(VaoPointer { stride });
        }

        pub fn render<S>(&mut self, program: &S)
        where
            S: shader::Shader + Send + Sync,
            P: memory::Raster<Item = S::Pixel>,
            D: memory::Raster<Item = f32>,
        {
            debug_assert!(self.color.size() == self.depth.size(), "Buffer dimensions must match");
            self.vscheduler.vertex_stage(
                program,
                &mut self.vertex_cores,
                &mut self.vao_queue,
                &self.vao_raw,
                self.vao_layout.peek(),
            );
        }
    }
}

pub mod shader {
    pub trait Shader {
        /// Vertex shader input
        type Vertex;

        /// Attributes to be interpolated during rasterization
        type Interpolant;

        /// Fragment shader output
        type Fragment;

        /// Pixel type on the screen buffer
        type Pixel;

        /// Vertex shader stage
        ///
        /// Returns an 'Interpolant' to be rasterized
        /// It is required to assign a value to <position_out> in NDC coordinates
        fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant;

        /// Fragment shader stage
        ///
        /// Returns an intermediate 'Fragment' without writing to any buffers
        fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment;

        /// Converts 'Fragment' to buffer's 'Pixel' representation and writes to memeory
        fn pixel(&self, fragment_in: &Self::Fragment) -> Self::Pixel;
    }
}

#[cfg(test)]
mod tests {
    use std::hint;

    use crate::{memory::transmute, vgpu::shader};

    struct TestingPipeline;
    impl shader::Shader for TestingPipeline {
        type Vertex = [f32; 6];
        type Interpolant = [f32; 6];
        type Fragment = [f32; 3];
        type Pixel = u32;
        fn vertex(&self, _: &Self::Vertex, _: &mut glam::Vec4) -> Self::Interpolant {
            todo!()
        }
        fn fragment(&self, _: &Self::Interpolant) -> Self::Fragment {
            todo!()
        }
        fn pixel(&self, _: &Self::Fragment) -> Self::Pixel {
            todo!()
        }
    }

    #[test]
    fn ridiculous_transmute() {
        fn generic_pipe_fn<S>(_: S, v: &[f32; 32])
        where
            S: shader::Shader,
        {
            unsafe {
                assert!(size_of_val(v) == 32 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Pixel)) == size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Vertex)) == 6 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Fragment)) == 3 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Interpolant)) == 6 * size_of::<f32>());
            }
        }

        let shader = TestingPipeline;
        generic_pipe_fn(shader, &[Default::default(); 32]);
    }

    #[test]
    #[unsafe(no_mangle)]
    fn controlled_ub() {
        fn generic_pipe_fn<S>(_: S, val: &[f32; 8])
        where
            S: shader::Shader,
        {
            let shader_in = transmute::bit_interp::<&[f32; 8], &S::Vertex>(&val);
            let shader_out = transmute::bit_interp::<&S::Vertex, &S::Fragment>(&shader_in);
            let good_val = transmute::bit_interp::<&S::Fragment, &[f32; 3]>(&shader_out);
            let bad_val = transmute::bit_interp::<&S::Fragment, &[f32; 8]>(&shader_out);
            hint::black_box(&shader_in);
            hint::black_box(&shader_out);
            hint::black_box(&good_val);
            hint::black_box(&bad_val);
            assert!(size_of_val(good_val) == 3 * size_of::<f32>());
            assert!(size_of_val(bad_val) == 8 * size_of::<f32>());
            assert!(good_val == &val[0..3]);
            assert!(bad_val[3..] == val[3..]);
        }
        let shader = TestingPipeline;
        generic_pipe_fn(shader, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }
}
