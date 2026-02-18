use gputils::{camera, model, transform};
use virtual_gpu::{gpu, memory, shader};

const SWIDTH: usize = 256;
const SHEIGHT: usize = 196;
const SSCALE: minifb::Scale = minifb::Scale::X4;
// const SFILL: u32 = 0xffu32 << 24 | 25u32 << 16 | 25u32 << 8 | 40u32;
const SFILL: u32 = 0xffu32 << 24 | 15u32 << 16 | 15u32 << 8 | 25u32;
const STITLE: &str = "Teapot Example";

struct Pipeline {
    mvp: glam::Mat4,
}

impl shader::Shader for Pipeline {
    type Vertex = model::Vertex;

    type Interpolant = model::Vertex;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        *position_out = self.mvp * vertex_in.pos.to_homogeneous();
        *vertex_in
    }

    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment {
        frag_vertex_in.nor
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
        .raster_cores(12)
        .color(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .depth(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .build();
    let model = model::Mesh::new("vendor/teapot/teapot.obj").unwrap();
    gpu.bind_data(&model.to_flat_vertices());
    gpu.set_vattrib_ptr(8);

    let camera = camera::Camera::builder().transform(glam::vec3(0.0, 0.0, 8.0).into()).build();
    let mut model_matrix = transform::Transform::default();

    let mut screen = minifb::Window::new(
        STITLE,
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();
    screen.set_target_fps(9999);

    loop {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::INFINITY);
        let shader = Pipeline {
            mvp: camera.proj_matrix(SWIDTH as f32 / SHEIGHT as f32)
                * camera.view_matrix()
                * model_matrix.matrix(),
        };
        gpu.render(&shader);
        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        example::model_translation(&mut model_matrix, &screen);
        example::model_rotation(&mut model_matrix, &screen);
        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }
    }
}
