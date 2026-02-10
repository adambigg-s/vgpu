use virtual_gpu::shader::Shader;
use virtual_gpu::{gpu, memory, shader};

mod utils;

const SWIDTH: usize = 400;
const SHEIGHT: usize = 300;

#[rustfmt::skip]
const TRIANGLE: [f32; 18] = [
    -0.5, -0.5, 0.0, 1.0, 0.7, 0.0,
    0.5, -0.5, 0.0, 0.0, 1.0, 0.7,
    0.0, 0.5, 0.0, 0.7, 0.0, 1.0,
];

struct Vertex {
    pos: glam::Vec3,
    col: glam::Vec3,
}

struct Fragment {
    col: glam::Vec3,
}

struct Pipeline;
impl shader::Shader for Pipeline {
    type Vertex = Vertex;

    type Interpolant = Fragment;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        *position_out = glam::vec3(vertex_in.pos.x, vertex_in.pos.y, vertex_in.pos.z).to_homogeneous();
        Self::Interpolant { col: vertex_in.col }
    }

    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment {
        frag_vertex_in.col
    }

    fn pixel(&self, fragment_in: &Self::Fragment) -> Self::Pixel {
        let r = (fragment_in.x * 255.9999) as u8 as u32;
        let g = (fragment_in.y * 255.9999) as u8 as u32;
        let b = (fragment_in.z * 255.9999) as u8 as u32;
        0xffu32 << 24 | r << 16 | g << 8 | b
    }
}

fn main() {
    let mut gpu = gpu::Gpu::new(32, 32);
    let shader = Pipeline;
    gpu.color = memory::RenderTarget::new([SWIDTH, SHEIGHT]);
    gpu.depth = memory::RenderTarget::new([SWIDTH, SHEIGHT]);
    gpu.bind_data(&TRIANGLE.to_vec());
    gpu.set_vattrib_ptr(6);

    let mut screen = minifb::Window::new(
        "Triangle",
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: minifb::Scale::X2, ..Default::default() },
    )
    .unwrap();

    loop {
        shader.render(&mut gpu);
        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }
    }
}
