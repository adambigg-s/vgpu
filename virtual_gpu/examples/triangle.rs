use virtual_gpu::{gpu, memory, shader};

mod utils;

const SWIDTH: usize = 128;
const SHEIGHT: usize = 96;
const SSCALE: minifb::Scale = minifb::Scale::X8;
const SFILL: u32 = 0xffu32 << 24 | 25u32 << 16 | 25u32 << 8 | 40u32;

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
    let mut gpu = gpu::Gpu::builder()
        .vertex_cores(4)
        .raster_cores(8)
        .color(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .depth(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .build();
    gpu.bind_data(&TRIANGLE.to_vec());
    gpu.set_vattrib_ptr(6);

    let mut screen = minifb::Window::new(
        "Triangle",
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();

    loop {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::MIN);
        gpu.render(&Pipeline);
        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }
    }
}
