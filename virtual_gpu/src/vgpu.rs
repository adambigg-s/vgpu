pub const REGISTER_SIZE: usize = 12;
pub const VAO_SIZE: usize = 1;
pub const TILE_SIZE: usize = 64;

pub mod cores {
    use std::sync::atomic;

    use glam::Vec4Swizzles;

    use crate::{
        gpu, interp,
        memory::{self, transmute},
        shader,
        vgpu::{self, aabb},
    };

    pub type FloatRegister = interp::Vector<f32, { vgpu::REGISTER_SIZE }>;

    pub type Tile = aabb::AaBb<f32, 2>;

    #[derive(Default, Debug)]
    pub struct VertexCore {}

    impl VertexCore {
        pub fn work<S, P>(
            &self,
            queue: &mut memory::Array<gpu::ProcessedVertex>,
            vao: &memory::Array<f32>,
            layout: &gpu::VaoPointer,
            scheduler: &gpu::Scheduler,
            viewport: &P,
            program: &S,
        ) where
            S: shader::Shader,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
        {
            debug_assert!(
                size_of::<S::Vertex>() < size_of::<FloatRegister>()
                    && size_of::<S::Interpolant>() < size_of::<FloatRegister>(),
                "Vertex attributes are too large: {}\nVertex size: {}\nInterpolant size: {}",
                vgpu::REGISTER_SIZE,
                size_of::<S::Vertex>(),
                size_of::<S::Interpolant>(),
            );

            let [hw, hh] = viewport.size().map(|dim| dim as f32 / 2.0);
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
                let vs_response = program.vertex(vertex, &mut position);
                let mut attributes = transmute::bit_interp::<S::Interpolant, FloatRegister>(&vs_response);

                position = {
                    let inv_depth = position.w.recip();
                    attributes = attributes * inv_depth;
                    position *= inv_depth;
                    position.w = inv_depth;
                    position.x = position.x * hw + hw;
                    position.y = -position.y * hh + hh;
                    position
                };

                queue[index] = gpu::ProcessedVertex { attribs: attributes, pos: position }
            }
        }
    }

    #[derive(Default, Debug)]
    pub struct RasterCore {
        pub attributes: [FloatRegister; 3],
        pub positions: [glam::Vec4; 3],
        pub tile: Tile,
    }

    impl RasterCore {
        pub fn work<S, P, D>(
            &mut self,
            color: &mut P,
            depth: &mut D,
            scheduler: &gpu::Scheduler,
            queue: &memory::Array<gpu::ProcessedVertex>,
            program: &S,
        ) where
            S: shader::Shader + Send + Sync,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
            loop {
                let head = scheduler.head.fetch_add(3, atomic::Ordering::Relaxed);
                let tail = head + 3;
                if tail > queue.len() {
                    break;
                }

                let data = &queue[head..tail];
                (0..3).for_each(|idx| {
                    self.attributes[idx] = data[idx].attribs;
                    self.positions[idx] = data[idx].pos;
                });

                let [mut minx, mut miny, mut maxx, mut maxy] = self.bounding_box();
                [minx, miny, maxx, maxy] = [
                    minx.max(0.0),
                    miny.max(0.0),
                    maxx.min((color.width() - 1) as f32),
                    maxy.min((color.height() - 1) as f32),
                ];
                let interp = interp::BarycentricSystem::from_points(self.positions.map(|vertex| vertex.xy()));

                for row in miny as i32..=maxy as i32 {
                    for col in minx as i32..=maxx as i32 {
                        let point = glam::vec2(col as f32 + 0.5, row as f32 + 0.5);
                        let lambdas = interp.sample_point(point);
                        if !interp.surrounds(lambdas) {
                            continue;
                        }

                        let mut pos = interp::weighted_sum(self.positions, lambdas.to_array());
                        let distance = pos.z;
                        if &distance > depth.peek(col as usize, row as usize) {
                            continue;
                        }

                        let inv_depth = pos.w.recip();
                        pos *= inv_depth;
                        let interp = interp::weighted_sum(
                            *interp::Vector::from(self.attributes.map(interp::Vector::from)),
                            lambdas.to_array(),
                        ) * inv_depth;
                        let fragment = program
                            .fragment(transmute::bit_interp::<&FloatRegister, &S::Interpolant>(&&interp));
                        let pixel = program.pixel(&fragment);

                        debug_assert!(
                            color.width() > col as usize && color.height() > row as usize,
                            "indices: {}, {}",
                            col,
                            row
                        );

                        *color.get(col as usize, row as usize) = pixel;
                        *depth.get(col as usize, row as usize) = distance;
                    }
                }
            }
        }

        fn bounding_box(&self) -> [f32; 4] {
            let [mut minx, mut miny] = [f32::INFINITY, f32::INFINITY];
            let [mut maxx, mut maxy] = [f32::NEG_INFINITY, f32::NEG_INFINITY];
            self.positions.iter().for_each(|vertex| {
                minx = minx.min(vertex.x);
                miny = miny.min(vertex.y);
                maxx = maxx.max(vertex.x);
                maxy = maxy.max(vertex.y);
            });
            [minx, miny, maxx, maxy]
        }
    }
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

        pub fn vertex_stage<S, P>(
            &self,
            cores: &mut memory::Array<cores::VertexCore>,
            queue: &mut memory::Array<ProcessedVertex>,
            vao: &memory::Array<f32>,
            layout: &VaoPointer,
            viewport: &P,
            program: &S,
        ) where
            S: shader::Shader + Send + Sync,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
        {
            self.reset();
            thread::scope(|scope| {
                cores.iter_mut().for_each(|core| {
                    scope.spawn(|| {
                        let queue = queue as *const memory::Array<ProcessedVertex>
                            as *mut memory::Array<ProcessedVertex>;
                        unsafe { core.work(&mut *queue, vao, layout, self, viewport, program) }
                    });
                });
            })
        }

        pub fn raster_stage<S, P, D>(
            &self,
            cores: &mut memory::Array<cores::RasterCore>,
            color: &mut P,
            depth: &mut D,
            queue: &memory::Array<ProcessedVertex>,
            program: &S,
        ) where
            S: shader::Shader + Send + Sync,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
            self.reset();
            thread::scope(|scope| {
                cores.iter_mut().for_each(|core| {
                    scope.spawn(|| {
                        let depth = depth as *const D as *mut D;
                        let color = color as *const P as *mut P;
                        unsafe {
                            core.work(&mut *color, &mut *depth, self, queue, program);
                        }
                    });
                });
            })
        }
    }

    #[derive(Default, Debug)]
    pub struct ProcessedVertex {
        pub attribs: cores::FloatRegister,
        pub pos: glam::Vec4,
    }

    #[derive(Default, Debug, bon::Builder)]
    pub struct Gpu<P, D> {
        #[builder(with = |num: usize| memory::Array::new([num]))]
        pub vertex_cores: memory::Array<cores::VertexCore>,
        #[builder(with = |num: usize| memory::Array::new([num]))]
        pub raster_cores: memory::Array<cores::RasterCore>,
        #[builder(default)]
        pub vscheduler: Scheduler,
        #[builder(default)]
        pub rscheduler: Scheduler,
        #[builder(default)]
        pub tiles: memory::Array<cores::Tile>,

        #[builder(default)]
        pub vao_layout: stack::Vec<VaoPointer, { vgpu::VAO_SIZE }>,
        #[builder(default)]
        pub vao_raw: memory::Array<f32>,
        #[builder(default)]
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
            debug_assert!(
                self.color.size() == self.depth.size()
                    || self.color.size() == [0, 0] && self.depth.size() != [0, 0]
                    || self.color.size() != [0, 0] && self.depth.size() == [0, 0],
                "Buffer dimensions must match, or ONE buffer must be zero-sized"
            );
            self.vao_queue = memory::Array::new([self.vao_raw.len() / self.vao_layout.peek().stride]);
            self.vscheduler.vertex_stage(
                &mut self.vertex_cores,
                &mut self.vao_queue,
                &self.vao_raw,
                self.vao_layout.peek(),
                &self.color,
                program,
            );
            self.rscheduler.raster_stage(
                &mut self.raster_cores,
                &mut self.color,
                &mut self.depth,
                &self.vao_queue,
                program,
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
