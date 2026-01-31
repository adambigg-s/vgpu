#[allow(dead_code)]
#[allow(unused_variables)]
pub mod gpu {
    use std::clone;

    use crate::memory;

    #[derive(Default, Debug)]
    pub struct GeometryCore {}

    #[derive(Default, Debug)]
    pub struct RasterCore {}

    #[derive(Default, Debug)]
    pub struct VaoPointer {}

    #[derive(Default, Debug)]
    pub struct Scheduler {}

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
        fn render(&self, target: &mut gpu::Vgpu) {}
    }
}
