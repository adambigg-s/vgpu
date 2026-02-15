use virtual_gpu::{gpu, memory, shader};

use crate::utils::{
    camera, model,
    transform::{self, Transform},
};

mod utils;

const SWIDTH: usize = 256;
const SHEIGHT: usize = 196;
const SSCALE: minifb::Scale = minifb::Scale::X4;
const SFILL: u32 = 0xffu32 << 24 | 25u32 << 16 | 25u32 << 8 | 40u32;
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
        *position_out =
            self.mvp * glam::vec3(vertex_in.pos.x, vertex_in.pos.y, vertex_in.pos.z).to_homogeneous();
        *vertex_in
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
    let mut gpu = gpu::Gpu::new(2, 8);
    gpu.color = memory::RenderTarget::<u32>::new([SWIDTH, SHEIGHT]);
    gpu.depth = memory::RenderTarget::<f32>::new([SWIDTH, SHEIGHT]);

    let camera = camera::Camera::builder().transform(glam::vec3(0.0, 0.0, 5.0).into()).build();

    let model = model::Model::new("../vendor/teapot.obj").unwrap();
    let mut model_matrix = transform::Transform::default();
    gpu.bind_data(&model.model.to_flat_vertices());
    gpu.set_vattrib_ptr(8);

    let mut screen = minifb::Window::new(
        STITLE,
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();

    loop {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::MIN);

        let shader = Pipeline {
            mvp: camera.proj_matrix(SWIDTH as f32 / SHEIGHT as f32)
                * camera.view_matrix()
                * model_matrix.matrix(),
        };

        gpu.render(&shader);
        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }
        if screen.is_key_down(minifb::Key::W) {
            model_matrix.rot *= glam::Quat::from_rotation_x(0.01);
        }
        if screen.is_key_down(minifb::Key::S) {
            model_matrix.rot *= glam::Quat::from_rotation_x(-0.01);
        }
        if screen.is_key_down(minifb::Key::A) {
            model_matrix.rot *= glam::Quat::from_rotation_z(0.01);
        }
        if screen.is_key_down(minifb::Key::D) {
            model_matrix.rot *= glam::Quat::from_rotation_z(-0.01);
        }
    }
}
