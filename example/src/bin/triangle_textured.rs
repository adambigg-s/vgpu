use gputils::model::texture;
use virtual_gpu::{gpu, memory};

const SWIDTH: usize = 256 * 2;
const SHEIGHT: usize = 196 * 2;
const SSCALE: minifb::Scale = minifb::Scale::X2;
const SFILL: u32 = 0xffu32 << 24 | 220u32 << 16 | 220u32 << 8 | 200u32;
const STITLE: &str = "Textured Example";

#[rustfmt::skip]
const TRIANGLE: [f32; 15] = [
    -0.5, -0.5, 0.0,      0.0,  0.0,
     0.5, -0.5, 0.0,      1.0,  0.0,
     0.0,  0.5, 0.0,      0.0,  1.0,
];

#[derive(Default)]
struct Pipeline<'r> {
    texture: texture::TextureRef<'r, glam::Vec3>,
}

impl<'r> virtual_gpu::Shader for Pipeline<'r> {
    type Vertex = (glam::Vec3, glam::Vec2);

    type Interpolant = glam::Vec2;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        let (pos, tex_coords) = *vertex_in;
        *position_out = pos.to_homogeneous();
        tex_coords
    }

    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment {
        self.texture.sample(frag_vertex_in.x, frag_vertex_in.y)
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
        .vertex_cores(2)
        .raster_cores(2)
        .color(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .depth(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .build();
    gpu.bind_data(&TRIANGLE.to_vec());
    gpu.set_vattrib_ptr(5);

    let mut screen = minifb::Window::new(
        STITLE,
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();
    let mut pipeline = Pipeline { ..Default::default() };
    let texture = texture::Texture::import("vendor/textures/brick.jpg").unwrap();

    loop {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::INFINITY);
        pipeline.texture = texture.reference();

        gpu.render(&pipeline);
        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }
    }
}
