pub mod gpu {
    use std::clone;

    use glam::Vec3Swizzles;

    use crate::{
        interp, memory,
        vgpu::shader::{self},
    };

    pub type Vertex = glam::Vec3;

    #[derive(Default, Debug)]
    pub struct GeometryCore {
        vertices_in: [Vertex; 3],
        vertices_out: [Vertex; 3],
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
        vertices_in: [Vertex; 3],
    }

    impl RasterCore {
        pub fn process<S>(
            &mut self,
            program: &S,
            color: &mut memory::Raster<u32>,
            depth: &mut memory::Raster<f32>,
        ) where
            S: shader::Shader,
        {
            let [hw, hh] = color.size().map(|dim| dim as f32 / 2.0);
            self.vertices_in = self.vertices_in.map(|mut vertex| {
                vertex.x = vertex.x * hw + hw;
                vertex.y = -vertex.y * hh + hh;
                vertex
            });

            let [minx, miny, maxx, maxy] = self.bounding_box();
            let bary = interp::BarycentricSystem::from_points(self.vertices_in.map(|vertex| vertex.xy()));
            for row in (miny as i32).max(0)..=(maxy as i32).min(color.size()[1] as i32) {
                for col in (minx as i32).max(0)..=(maxx as i32).min(color.size()[0] as i32) {
                    let point = glam::vec2(col as f32, row as f32);
                    let lambdas = bary.sample_point(point);
                    if !bary.surrounds(lambdas) {
                        continue;
                    }

                    *color.get_mut([col as usize, row as usize]) = 0xff00ffff;
                }
            }
        }

        fn bounding_box(&self) -> [f32; 4] {
            let [mut minx, mut miny] = [f32::INFINITY, f32::NEG_INFINITY];
            let [mut maxx, mut maxy] = [f32::INFINITY, f32::NEG_INFINITY];
            self.vertices_in.iter().for_each(|vertex| {
                minx = minx.min(vertex.x);
                maxx = maxx.max(vertex.x);
                miny = miny.min(vertex.y);
                maxy = maxy.max(vertex.y);
            });
            [minx, miny, maxx, maxy]
        }
    }

    #[derive(Default, Debug)]
    pub struct VaoPointer {}

    #[derive(Default, Debug)]
    pub struct Scheduler {
        head: usize,
    }

    impl Scheduler {
        pub fn load_geometry_cores(
            &mut self,
            cores: &mut memory::Array<GeometryCore>,
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
            cores: &mut memory::Array<RasterCore>,
            data: &memory::Array<GeometryCore>,
        ) {
            debug_assert!(cores.len() == data.len());
            for i in 0..cores.len() {
                cores[i].vertices_in = data[i].vertices_out;
            }
        }
    }

    #[derive(Default, Debug)]
    pub struct Vgpu {
        pub vertex_cores: memory::Array<GeometryCore>,
        pub raster_cores: memory::Array<RasterCore>,
        pub scheduler: Scheduler,

        pub color: memory::Raster<u32>,
        pub depth: memory::Raster<f32>,

        pub vao: memory::Array<f32>,
        pub vao_layout: VaoPointer,
    }

    impl Vgpu {
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
            S: shader::Shader,
        {
            for core in self.vertex_cores.iter_mut() {
                core.process(program);
            }
        }

        pub fn cycle_raster_cores<S>(&mut self, program: &S)
        where
            S: shader::Shader,
        {
            for core in self.raster_cores.iter_mut() {
                core.process(program, &mut self.color, &mut self.depth);
            }
        }
    }
}

#[allow(dead_code)]
#[allow(unused_variables)]
pub mod shader {
    use crate::vgpu::gpu;

    pub trait Shader {
        fn vertex(&self, vertex: glam::Vec3) -> glam::Vec3;

        fn fragment(&self, frag_vertex: glam::Vec3) -> glam::Vec3;

        fn pixel(&self, fragment: glam::Vec3) -> u32;

        /// !!! NO OVERRIDE !!!
        fn render(&self, target: &mut gpu::Vgpu)
        where
            Self: Sized,
        {
            debug_assert!(target.color.size() == target.depth.size(), "Buffer dimensions must match");
            target.scheduler.load_geometry_cores(&mut target.vertex_cores, &target.vao);
            target.cycle_vertex_cores(self);
            target.scheduler.load_raster_cores(&mut target.raster_cores, &target.vertex_cores);
            target.cycle_raster_cores(self);
        }
    }
}
