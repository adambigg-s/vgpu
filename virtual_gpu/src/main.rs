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
    -0.5, -0.5, 0.0,
    0.5 , -0.5, 0.0,
    0.0 , 0.5 , 0.0,
];

struct Pipeline;
impl shader::Shader for Pipeline {
    fn vertex(&self, vertex: glam::Vec3) -> glam::Vec3 {
        vertex
    }

    fn fragment(&self, frag_vertex: glam::Vec3) -> glam::Vec3 {
        frag_vertex
    }

    fn pixel(&self, fragment: glam::Vec3) -> u32 {
        let red = (fragment.x * 255.9999) as u8 as u32;
        let gre = (fragment.y * 255.9999) as u8 as u32;
        let blu = (fragment.z * 255.9999) as u8 as u32;
        0xff_u32 << 24 | red << 16 | gre << 8 | blu
    }
}

fn main() {
    let mut gpu = gpu::Vgpu::new(1, 1);
    gpu.color = memory::Raster::new([100, 100]);
    gpu.depth = memory::Raster::new([100, 100]);

    let pipeline = Pipeline;
    gpu.bind_data(&TRIANGLE.to_vec());

    let mut window = minifb::Window::new("Virtual GPU", 100, 100, Default::default()).unwrap();

    while !window.is_key_down(minifb::Key::Escape) {
        pipeline.render(&mut gpu);
        window.update_with_buffer(&gpu.color, gpu.color.size()[0], gpu.color.size()[1]).unwrap();
    }
}
