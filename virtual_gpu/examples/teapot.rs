use virtual_gpu::{gpu, memory, shader};

use crate::utils::model;

mod utils;

const SWIDTH: usize = 256;
const SHEIGHT: usize = 256;
const SSCALE: minifb::Scale = minifb::Scale::X2;

struct Pipeline;
impl shader::Shader for Pipeline {
    type Vertex = model::Vertex;

    type Interpolant = model::Vertex;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        todo!()
    }

    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment {
        todo!()
    }

    fn pixel(&self, fragment_in: &Self::Fragment) -> Self::Pixel {
        todo!()
    }
}

fn main() {
    let mut gpu = gpu::Gpu::new(3, 3);
    let shader = Pipeline;
    gpu.color = memory::RenderTarget::<f32>::new([SWIDTH, SHEIGHT]);
    gpu.depth = memory::RenderTarget::<f32>::new([SWIDTH, SHEIGHT]);
    gpu.set_vattrib_ptr(size_of::<model::Vertex>() / size_of::<f32>());

    let mut screen = minifb::Window::new(
        "Triangle",
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();
    screen.set_target_fps(999);
}
