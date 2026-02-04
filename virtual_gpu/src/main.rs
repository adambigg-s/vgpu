use crate::vgpu::{
    gpu,
    shader::{self, Shader},
};

mod interp;
mod memory;
mod vgpu;

#[rustfmt::skip]
const TRIANGLE: [f32; 18] = [
    -0.6, -0.4, 0.0, 1.0, 0.4, 0.0,
    0.5, -0.6, 0.0, 0.0, 1.0, 0.4,
    0.1, 0.5, 0.0, 0.4, 0.0, 1.0,
];

#[repr(C, packed)]
struct Vertex {
    pos: glam::Vec3,
    col: glam::Vec3,
}

#[repr(C, packed)]
struct Fragment {
    col: glam::Vec3,
}

struct Pipeline;
impl shader::Shader for Pipeline {
    type Vertex = Vertex;

    type Interpolant = Fragment;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    #[inline]
    fn vertex(&self, vertex: &Self::Vertex, pos_out: &mut glam::Vec3) -> Self::Interpolant {
        *pos_out = vertex.pos;
        Fragment { col: vertex.col }
    }

    #[inline]
    fn fragment(&self, frag_vertex: &Self::Interpolant) -> Self::Fragment {
        frag_vertex.col
    }

    #[inline]
    fn pixel(&self, fragment: &Self::Fragment) -> Self::Pixel {
        let red = (fragment[0] * 255.9999) as u8 as u32;
        let gre = (fragment[1] * 255.9999) as u8 as u32;
        let blu = (fragment[2] * 255.9999) as u8 as u32;
        0xffu32 << 24 | red << 16 | gre << 8 | blu
    }
}

const SWIDTH: usize = 400;
const SHEIGHT: usize = 300;
const SCALE: minifb::Scale = minifb::Scale::X4;
const SFILL: u32 = 0xffu32 << 24 | 25u32 << 16 | 25u32 << 8 | 40u32;

fn main() {
    let mut gpu = gpu::Gpu::new(4, 4);
    gpu.color = memory::RenderTarget::new([SWIDTH, SHEIGHT]);
    gpu.depth = memory::RenderTarget::new([SWIDTH, SHEIGHT]);

    let pipeline = Pipeline;
    gpu.set_vattrib_ptr(6);
    gpu.bind_data(&TRIANGLE.to_vec());

    let mut window = minifb::Window::new(
        "Virtual GPU",
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SCALE, ..Default::default() },
    )
    .unwrap();
    window.set_target_fps(999);

    while !window.is_key_down(minifb::Key::Escape) {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::MAX);
        pipeline.render(&mut gpu);

        window.update_with_buffer(&gpu.color, gpu.color.size()[0], gpu.color.size()[1]).unwrap();
    }
}
