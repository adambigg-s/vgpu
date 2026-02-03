use crate::vgpu::{
    gpu,
    shader::{self, Shader},
};

mod interp;
mod memory;
mod vgpu;

#[rustfmt::skip]
#[allow(dead_code)]
const TRIANGLE: [f32; 9] = [
    -0.6, -0.4, 0.0,
    0.5 , -0.6, 0.0,
    0.1 , 0.5 , 0.0,
];

struct Pipeline;
impl shader::Shader for Pipeline {
    type Vertex = [f32; 6];

    type VertexAttribs = [f32; 6];

    type Fragment = [f32; 3];

    type Pixel = u32;

    fn vertex(&self, vertex: &Self::Vertex) -> (glam::Vec3, Self::VertexAttribs) {
        (glam::Vec3::from_slice(&vertex[0..3]), *vertex)
    }

    fn fragment(&self, _: &Self::VertexAttribs) -> Self::Fragment {
        [1.0, 0.2, 0.4]
    }

    fn pixel(&self, fragment: &Self::Fragment) -> Self::Pixel {
        let red = (fragment[0] * 255.9999) as u8 as u32;
        let gre = (fragment[1] * 255.9999) as u8 as u32;
        let blu = (fragment[2] * 255.9999) as u8 as u32;
        0xffu32 << 24 | red << 16 | gre << 8 | blu
    }
}

const SWIDTH: usize = 48;
const SHEIGHT: usize = 48;
const SCALE: minifb::Scale = minifb::Scale::X16;
const SFILL: u32 = 0xffu32 << 24 | 25u32 << 16 | 25u32 << 8 | 40u32;

fn main() {
    let mut gpu = gpu::Gpu::new(2, 2);
    // gpu.color = memory::RenderTarget::new([0, 0]);
    // gpu.depth = memory::RenderTarget::new([0, 0]);
    gpu.color = memory::RenderTarget::new([SWIDTH, SHEIGHT]);
    gpu.depth = memory::RenderTarget::new([SWIDTH, SHEIGHT]);

    let pipeline = Pipeline;
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
        // gpu.color.fill(SFILL);
        // gpu.depth.fill(f32::INFINITY);
        pipeline.render(&mut gpu);

        // window.update();
        window.update_with_buffer(&gpu.color, gpu.color.size()[0], gpu.color.size()[1]).unwrap();
    }

    // dbg!(&gpu);
}
