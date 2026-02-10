const THREADED: bool = true;

pub mod cores {
    use glam::Vec4Swizzles;

    use crate::{
        interp,
        memory::{self, transmute},
        vgpu::shader,
    };

    type FloatRegister = interp::Vector<f32, 12>;

    #[derive(Default, Debug)]
    pub struct GeometryCore {
        pub vertices_in: [FloatRegister; 3],
        pub attribs_out: [FloatRegister; 3],
        pub positions_out: [glam::Vec4; 3],
        pub enabled: bool,
    }

    impl GeometryCore {
        pub fn process<S, P>(&mut self, program: &S, viewport: &P)
        where
            S: shader::Shader,
            P: memory::Raster<Item = S::Pixel>,
        {
            debug_assert!(
                size_of::<S::Vertex>() < size_of::<FloatRegister>()
                    && size_of::<S::Interpolant>() < size_of::<FloatRegister>(),
                "Vertex attributes are too large for core buffers"
            );

            if !self.enabled {
                return;
            }

            for i in 0..3 {
                let vertex = transmute::bit_interp::<&FloatRegister, &S::Vertex>(&&self.vertices_in[i]);
                let mut pos = glam::Vec4::default();
                let attrib = program.vertex(vertex, &mut pos);
                let attrib = transmute::bit_interp::<S::Interpolant, FloatRegister>(&attrib);
                self.attribs_out[i] = attrib;
                self.positions_out[i] = pos;
            }
            let [hw, hh] = viewport.size().map(|dim| dim as f32 / 2.0);
            self.positions_out = self.positions_out.map(|mut vertex| {
                let inv_depth = vertex.w.recip();
                vertex *= inv_depth;
                vertex.w = inv_depth;
                vertex.x = vertex.x * hw + hw;
                vertex.y = -vertex.y * hh + hh;
                vertex
            });
        }
    }

    #[derive(Default, Debug)]
    pub struct RasterCore {
        pub attribs_in: [FloatRegister; 3],
        pub positions_in: [glam::Vec4; 3],
        pub enabled: bool,
    }

    impl RasterCore {
        pub fn process<S, P, D>(&mut self, program: &S, color: &mut P, depth: &mut D)
        where
            S: shader::Shader,
            P: memory::Raster<Item = S::Pixel>,
            D: memory::Raster<Item = f32>,
        {
            if !self.enabled {
                return;
            }

            let [mut minx, mut miny, mut maxx, mut maxy] = self.bounding_box();
            [minx, miny, maxx, maxy] = [
                minx.max(0.0),
                miny.max(0.0),
                maxx.min((color.width() - 1) as f32),
                maxy.min((color.height() - 1) as f32),
            ];
            let interp = interp::BarycentricSystem::from_points(self.positions_in.map(|vertex| vertex.xy()));

            for row in miny as i32..=maxy as i32 {
                for col in minx as i32..=maxx as i32 {
                    let point = glam::vec2(col as f32 + 0.5, row as f32 + 0.5);
                    let lambdas = interp.sample_point(point);
                    if !interp.surrounds(lambdas) {
                        continue;
                    }

                    let mut pos = interp::weighted_sum(self.positions_in, lambdas.to_array());
                    let distance = pos.z;
                    if &distance < depth.peek(col as usize, row as usize) {
                        continue;
                    }

                    let inv_depth = distance.recip();
                    pos *= inv_depth;

                    let interp = interp::weighted_sum(
                        *interp::Vector::from(self.attribs_in.map(interp::Vector::from)),
                        lambdas.to_array(),
                    );
                    let fragment =
                        program.fragment(transmute::bit_interp::<&FloatRegister, &S::Interpolant>(&&interp));
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

        fn bounding_box(&self) -> [f32; 4] {
            let [mut minx, mut miny] = [f32::INFINITY, f32::INFINITY];
            let [mut maxx, mut maxy] = [f32::NEG_INFINITY, f32::NEG_INFINITY];
            self.positions_in.iter().for_each(|vertex| {
                minx = minx.min(vertex.x);
                miny = miny.min(vertex.y);
                maxx = maxx.max(vertex.x);
                maxy = maxy.max(vertex.y);
            });
            [minx, miny, maxx, maxy]
        }
    }
}

pub mod gpu {
    use std::clone;

    use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};

    use crate::{
        memory::{self, stack},
        vgpu::{
            self, cores,
            shader::{self},
        },
    };

    #[derive(Default, Debug)]
    pub struct VaoPointer {
        stride: usize,
    }

    #[derive(Default, Debug)]
    pub struct Scheduler {
        head: usize,
    }

    impl Scheduler {
        pub fn reset(&mut self) {
            *self = Default::default()
        }

        pub fn incomplete(&self, data: &memory::Array<f32>) -> bool {
            self.head < data.len()
        }

        pub fn load_geometry_cores(
            &mut self,
            cores: &mut memory::Array<cores::GeometryCore>,
            data: &memory::Array<f32>,
            ptr: &stack::Vec<VaoPointer, 1>,
        ) {
            let vsize = ptr.into_iter().map(|ptr| ptr.stride).max().unwrap_or(3);
            debug_assert!(vsize > 0);

            #[allow(clippy::identity_op)]
            #[allow(clippy::erasing_op)]
            for i in 0..cores.len() {
                let core = &mut cores[i];
                core.enabled = false;

                if data.len() < self.head + vsize * 3 {
                    break;
                }

                let data = &data[self.head..self.head + vsize * 3];
                core.vertices_in[0][..vsize].copy_from_slice(&data[vsize * 0..vsize * 1]);
                core.vertices_in[1][..vsize].copy_from_slice(&data[vsize * 1..vsize * 2]);
                core.vertices_in[2][..vsize].copy_from_slice(&data[vsize * 2..vsize * 3]);
                core.enabled = true;

                self.head += vsize * 3;
            }
        }

        pub fn load_raster_cores(
            &mut self,
            cores: &mut memory::Array<cores::RasterCore>,
            data: &memory::Array<cores::GeometryCore>,
        ) {
            debug_assert!(cores.len() == data.len());
            for i in 0..cores.len() {
                cores[i].enabled = false;
                if !data[i].enabled {
                    continue;
                }
                cores[i].attribs_in = data[i].attribs_out;
                cores[i].positions_in = data[i].positions_out;
                cores[i].enabled = true;
            }
        }
    }

    #[derive(Default, Debug)]
    pub struct Gpu<P, D> {
        pub vertex_cores: memory::Array<cores::GeometryCore>,
        pub raster_cores: memory::Array<cores::RasterCore>,
        pub scheduler: Scheduler,

        pub color: P,
        pub depth: D,

        pub vao: memory::Array<f32>,
        pub vao_layout: stack::Vec<VaoPointer, 1>,
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
            self.vao = memory::Array::from_parts([data.len()], clone::Clone::clone(data));
        }

        pub fn set_vattrib_ptr(&mut self, stride: usize) {
            if !self.vao_layout.is_empty() {
                self.vao_layout.pop();
            }
            debug_assert!(self.vao_layout.len() < self.vao_layout.capacity());
            self.vao_layout.push(VaoPointer { stride });
        }

        pub fn cycle_vertex_cores<S>(&mut self, program: &S)
        where
            S: shader::Shader + Send + Sync,
            P: memory::Raster<Item = S::Pixel>,
        {
            if vgpu::THREADED {
                self.vertex_cores.par_iter_mut().for_each(|core| {
                    core.process(program, &self.color);
                });
            }
            else {
                self.vertex_cores.iter_mut().for_each(|core| {
                    core.process(program, &self.color);
                });
            }
        }

        pub fn cycle_raster_cores<S>(&mut self, program: &S)
        where
            S: shader::Shader + Send + Sync,
            P: memory::Raster<Item = S::Pixel>,
            D: memory::Raster<Item = f32>,
        {
            if vgpu::THREADED {
                let color = &self.color;
                let depth = &self.depth;
                #[allow(invalid_reference_casting)]
                unsafe {
                    self.raster_cores.par_iter_mut().for_each(|core| {
                        let color = color as *const P as *mut P;
                        let depth = depth as *const D as *mut D;
                        core.process(program, &mut *color, &mut *depth);
                    });
                }
            }
            else {
                self.raster_cores.iter_mut().for_each(|core| {
                    core.process(program, &mut self.color, &mut self.depth);
                });
            }
        }
    }
}

pub mod shader {
    use crate::{
        memory,
        vgpu::gpu::{self},
    };

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

        /**
        !!! NO OVERRIDE !!!

        This method has a generalized implementation and shouldn't ever be touched
        */
        fn render<P, D>(&self, target: &mut gpu::Gpu<P, D>)
        where
            Self: Sized + Send + Sync,
            P: Default + memory::Raster<Item = Self::Pixel> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
            debug_assert!(target.color.size() == target.depth.size(), "Buffer dimensions must match");
            target.scheduler.reset();
            loop {
                target.scheduler.load_geometry_cores(
                    &mut target.vertex_cores,
                    &target.vao,
                    &target.vao_layout,
                );
                target.cycle_vertex_cores(self);
                target.scheduler.load_raster_cores(&mut target.raster_cores, &target.vertex_cores);
                target.cycle_raster_cores(self);

                if !target.scheduler.incomplete(&target.vao) {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        memory::transmute,
        vgpu::shader,
    };

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
    fn controlled_ub() {
        fn generic_pipe_fn<S>(_: S, val: &[f32; 8])
        where
            S: shader::Shader,
        {
            let shader_in = transmute::bit_interp::<&[f32; 8], &S::Vertex>(&val);
            let shader_out = transmute::bit_interp::<&S::Vertex, &S::Fragment>(&shader_in);
            let val = transmute::bit_interp::<&S::Fragment, &[f32; 3]>(&shader_out);
            assert!(val == &[1.0, 2.0, 3.0]);
        }
        let shader = TestingPipeline;
        generic_pipe_fn(shader, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }
}
