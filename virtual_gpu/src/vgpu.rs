pub mod gpu {
    use crate::memory;

    #[derive(Default)]
    struct GeoCore {}

    #[derive(Default)]
    struct RasterCore {}

    pub struct Vgpu {
        vertex_cores: memory::Array<GeoCore>,
        raster_cores: memory::Array<RasterCore>,
    }

    impl Vgpu {
        pub fn new(vcores: usize, rcores: usize) -> Self {
            Self {
                vertex_cores: memory::Array::new([vcores]),
                raster_cores: memory::Array::new([rcores]),
            }
        }
    }
}

#[allow(dead_code)]
pub mod shader {
    use crate::vgpu::gpu::Vgpu;

    pub trait Shader {
        type Vertex;
        type FragVertex;
        type Fragment;
        type Pixel;

        fn vertex(&self, vertex: Self::Vertex) -> Self::FragVertex;

        fn fragment(&self, frag_vertex: Self::FragVertex) -> Self::Fragment;

        fn pixel(&self, fragment: Self::Fragment) -> Self::Pixel;

        fn render(&self, target: Vgpu);
    }
}
