pub mod cores {
    use glam::Vec3Swizzles;

    use crate::{interp, memory, vgpu::shader};

    pub type Vertex = glam::Vec3;

    #[derive(Default, Debug)]
    pub struct GeometryCore {
        pub vertices_in: [Vertex; 3],
        pub vertices_out: [Vertex; 3],
    }

    impl GeometryCore {
        pub fn process<S>(&mut self, program: &S)
        where
            S: shader::Shader,
        {
            self.vertices_out = self.vertices_in.map(|vertex| program.vertex(vertex));
        }
    }

    #[derive(Default, Debug)]
    pub struct RasterCore {
        pub vertices: [Vertex; 3],
    }

    impl RasterCore {
        pub fn process<S, P, D>(&mut self, program: &S, color: &mut P, depth: &mut D)
        where
            S: shader::Shader,
            P: memory::Raster<Item = u32>,
            D: memory::Raster<Item = f32>,
        {
            let [hw, hh] = color.size().map(|dim| dim as f32 / 2.0);
            self.vertices = self.vertices.map(|mut vertex| {
                vertex.x = vertex.x * hw + hw;
                vertex.y = -vertex.y * hh + hh;
                vertex
            });

            let [mut minx, mut miny, mut maxx, mut maxy] = self.bounding_box();
            [minx, miny, maxx, maxy] = [
                minx.max(0.0),
                miny.max(0.0),
                maxx.min(color.size()[0] as f32),
                maxy.min(color.size()[1] as f32),
            ];
            let interp = interp::BarycentricSystem::from_points(self.vertices.map(|vertex| vertex.xy()));
            for row in miny as i32..maxy as i32 {
                for col in minx as i32..maxx as i32 {
                    let point = glam::vec2(col as f32, row as f32);
                    let lambdas = interp.sample_point(point);
                    if !interp.surrounds(lambdas) {
                        continue;
                    }
                    let frag_vertex = interp::weighted_sum(self.vertices, lambdas.to_array());

                    let fragment = program.fragment(frag_vertex);
                    let pixel = program.pixel(fragment);

                    debug_assert!(
                        color.width() > col as usize && color.height() > row as usize,
                        "indices: {}, {}",
                        col,
                        row
                    );

                    *color.get(col as usize, row as usize) = pixel;
                    *depth.get(col as usize, row as usize) = fragment.z;
                }
            }
        }

        fn bounding_box(&self) -> [f32; 4] {
            let [mut minx, mut miny] = [f32::INFINITY, f32::INFINITY];
            let [mut maxx, mut maxy] = [f32::NEG_INFINITY, f32::NEG_INFINITY];
            self.vertices.iter().for_each(|vertex| {
                minx = minx.min(vertex.x);
                miny = miny.min(vertex.y);
                maxx = maxx.max(vertex.x);
                maxy = maxy.max(vertex.y);
            });
            [minx, miny, maxx, maxy]
        }
    }
}

#[allow(unused_imports)]
pub mod gpu {
    use std::clone;

    use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};

    use crate::{
        memory::{self, Raster, stack},
        vgpu::{
            cores,
            shader::{self},
        },
    };

    #[derive(Default, Debug)]
    pub struct VaoPointer {
        location: usize,
        size: usize,
        stride: usize,
        offset: usize,
    }

    #[derive(Default, Debug)]
    pub struct Scheduler {
        head: usize,
    }

    impl Scheduler {
        pub fn load_geometry_cores(
            &mut self,
            cores: &mut memory::Array<cores::GeometryCore>,
            data: &memory::Array<f32>,
        ) {
            const VSIZE: usize = 3;
            self.head = 0;

            #[allow(clippy::identity_op)]
            #[allow(clippy::erasing_op)]
            for i in 0..cores.len() {
                if data.len() < self.head + VSIZE * 3 {
                    break;
                }

                debug_assert!(
                    data.len() >= self.head + VSIZE * 3,
                    "data: {}\nhead: {}",
                    data.len(),
                    self.head
                );

                let v1 = glam::Vec3::from_slice(&data[self.head + VSIZE * 0..self.head + VSIZE * 1]);
                let v2 = glam::Vec3::from_slice(&data[self.head + VSIZE * 1..self.head + VSIZE * 2]);
                let v3 = glam::Vec3::from_slice(&data[self.head + VSIZE * 2..self.head + VSIZE * 3]);
                cores[i].vertices_in = [v1, v2, v3];

                self.head += VSIZE * 3;
            }

            debug_assert!(self.head == data.len());
        }

        pub fn load_raster_cores(
            &mut self,
            cores: &mut memory::Array<cores::RasterCore>,
            data: &memory::Array<cores::GeometryCore>,
        ) {
            debug_assert!(cores.len() == data.len());
            for i in 0..cores.len() {
                cores[i].vertices = data[i].vertices_out;
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
        P: Default + Raster<Item = u32> + Send + Sync,
        D: Default + Raster<Item = f32> + Send + Sync,
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

        pub fn cycle_vertex_cores<S>(&mut self, program: &S)
        where
            S: shader::Shader + Send + Sync,
        {
            // // Parallel
            // self.vertex_cores.par_iter_mut().for_each(|core| {
            //     core.process(program);
            // });

            // Sequential
            self.vertex_cores.iter_mut().for_each(|core| {
                core.process(program);
            });
        }

        pub fn cycle_raster_cores<S>(&mut self, program: &S)
        where
            S: shader::Shader + Send + Sync,
        {
            // // Parallel
            // let color = &self.color;
            // let depth = &self.depth;
            // #[allow(invalid_reference_casting)]
            // unsafe {
            //     self.raster_cores.par_iter_mut().for_each(|core| {
            //         let color = color as *const P as *mut P;
            //         let depth = depth as *const D as *mut D;
            //         core.process(program, &mut *color, &mut *depth);
            //     });
            // }

            // Sequential
            self.raster_cores.iter_mut().for_each(|core| {
                core.process(program, &mut self.color, &mut self.depth);
            });
        }
    }
}

#[allow(dead_code)]
#[allow(unused_variables)]
pub mod shader {
    use crate::{memory, vgpu::gpu};

    pub trait Shader {
        fn vertex(&self, vertex: glam::Vec3) -> glam::Vec3;

        fn fragment(&self, frag_vertex: glam::Vec3) -> glam::Vec3;

        fn pixel(&self, fragment: glam::Vec3) -> u32;

        /// !!! NO OVERRIDE !!!
        fn render<P, D>(&self, target: &mut gpu::Gpu<P, D>)
        where
            Self: Sized + Send + Sync,
            P: Default + memory::Raster<Item = u32> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
            debug_assert!(target.color.size() == target.depth.size(), "Buffer dimensions must match");
            target.scheduler.load_geometry_cores(&mut target.vertex_cores, &target.vao);
            target.cycle_vertex_cores(self);
            target.scheduler.load_raster_cores(&mut target.raster_cores, &target.vertex_cores);
            target.cycle_raster_cores(self);
        }
    }
}
