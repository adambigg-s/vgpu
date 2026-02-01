#[allow(dead_code)]
#[allow(unused_variables)]
pub mod gpu {
    use std::clone;

    use crate::{
        memory,
        vgpu::shader::{self},
    };

    pub type Vertex = glam::Vec3;

    #[derive(Default, Debug)]
    pub struct GeometryCore {
        vertices_in: [Vertex; 3],
        vertices_out: [Vertex; 3],
    }

    impl GeometryCore {
        pub fn process<S>(&mut self, program: S)
        where
            S: shader::Shader,
        {
            self.vertices_out = self.vertices_in.map(|vertex| program.vertex(vertex));
        }
    }

    #[derive(Default, Debug)]
    pub struct RasterCore {
        vertices_in: [Vertex; 3],
        vertices_out: [Vertex; 3],
    }

    impl RasterCore {
        pub fn process<S>(&mut self, program: S)
        where
            S: shader::Shader,
        {
            self.vertices_out = self.vertices_in.map(|vertex| program.fragment(vertex));
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
            let num_cores = cores.len();

            #[allow(clippy::identity_op)]
            for i in 0..num_cores {
                debug_assert!(data.len() >= self.head + 9, "data: {}\nhead: {}", data.len(), self.head);

                let v1 = glam::Vec3::from_slice(&data[self.head + 0..self.head + 3]);
                let v2 = glam::Vec3::from_slice(&data[self.head + 3..self.head + 6]);
                let v3 = glam::Vec3::from_slice(&data[self.head + 6..self.head + 9]);
                cores[i].vertices_in = [v1, v2, v3];

                self.head += 9;
            }
        }

        pub fn load_raster_cores(
            &mut self,
            cores: &mut memory::Array<RasterCore>,
            data: &mut memory::Array<f32>,
        ) {
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
        fn render(&self, target: &mut gpu::Vgpu) {
            target.scheduler.load_geometry_cores(&mut target.vertex_cores, &target.vao);
        }
    }
}
