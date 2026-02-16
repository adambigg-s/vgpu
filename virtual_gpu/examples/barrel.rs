use glam::Vec4Swizzles;
use virtual_gpu::{gpu, memory, shader};

use crate::utils::{
    camera,
    model::{self, texture},
    transform::{self},
};

mod utils;

const SWIDTH: usize = 256 * 4;
const SHEIGHT: usize = 196 * 4;
const SSCALE: minifb::Scale = minifb::Scale::X1;
const SFILL: u32 = 0xffu32 << 24 | 25u32 << 16 | 25u32 << 8 | 40u32;
const STITLE: &str = "Barrel Example";

#[derive(Default)]
struct Pipeline {
    model_mat: glam::Mat4,
    mvp_mat: glam::Mat4,
    nor_mat: glam::Mat3,
    tex: texture::Texture,
    nor: texture::Texture,
    met: texture::Texture,
    light_direction: glam::Vec3,
}

impl shader::Shader for Pipeline {
    type Vertex = model::Vertex;

    type Interpolant = model::Vertex;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    #[inline(always)]
    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        *position_out = self.mvp_mat * vertex_in.pos.to_homogeneous();

        let mut vertex_out = *vertex_in;
        vertex_out.pos = (self.model_mat * vertex_in.pos.to_homogeneous()).xyz();
        vertex_out.nor = self.nor_mat * vertex_out.nor;
        vertex_out
    }

    #[inline(always)]
    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment {
        let albedo = self.tex.sample_bilinear(frag_vertex_in.uv.x, frag_vertex_in.uv.y);
        let normal = (self.nor.sample(frag_vertex_in.uv.x, frag_vertex_in.uv.y) * 0.25 + frag_vertex_in.nor)
            .normalize();
        let reflec = self.met.sample(frag_vertex_in.uv.x, frag_vertex_in.uv.y);
        let relative = (self.light_direction - frag_vertex_in.pos).normalize();
        let light = relative.dot(normal).max(0.05);
        albedo * (light + reflec * 0.25)
    }

    #[inline(always)]
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
    let model = model::Model::new("../vendor/barrel/barrel.obj").unwrap();
    let mut shader = Pipeline {
        tex: "../vendor/barrel/texture.jpg".into(),
        nor: "../vendor/barrel/normal.jpg".into(),
        met: "../vendor/barrel/metallic.jpg".into(),
        light_direction: glam::vec3(10.0, 25.0, 15.0),
        ..Default::default()
    };
    gpu.bind_data(&model.model.to_flat_vertices());
    gpu.set_vattrib_ptr(8);

    let camera = camera::Camera::builder().transform(glam::vec3(0.0, 0.0, 6.0).into()).build();
    let mut model_matrix = transform::Transform::default();

    let mut screen = minifb::Window::new(
        STITLE,
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();
    screen.set_target_fps(9999);

    let mut starting = std::time::Instant::now();
    loop {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::INFINITY);
        shader.model_mat = model_matrix.matrix();
        shader.mvp_mat =
            camera.proj_matrix(SWIDTH as f32 / SHEIGHT as f32) * camera.view_matrix() * model_matrix.matrix();
        shader.nor_mat = glam::Mat3::from_mat4(model_matrix.matrix()).inverse().transpose();
        gpu.render(&shader);
        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }
        if screen.is_key_down(minifb::Key::W) {
            model_matrix.rot *= glam::Quat::from_rotation_x(-0.01);
        }
        if screen.is_key_down(minifb::Key::S) {
            model_matrix.rot *= glam::Quat::from_rotation_x(0.01);
        }
        if screen.is_key_down(minifb::Key::A) {
            model_matrix.rot *= glam::Quat::from_rotation_y(0.01);
        }
        if screen.is_key_down(minifb::Key::D) {
            model_matrix.rot *= glam::Quat::from_rotation_y(-0.01);
        }
        if screen.is_key_down(minifb::Key::Q) {
            model_matrix.rot *= glam::Quat::from_rotation_z(0.01);
        }
        if screen.is_key_down(minifb::Key::E) {
            model_matrix.rot *= glam::Quat::from_rotation_z(-0.01);
        }
        if screen.is_key_down(minifb::Key::Down) {
            model_matrix.pos -= glam::vec3(0.0, 0.01, 0.0);
        }
        if screen.is_key_down(minifb::Key::Up) {
            model_matrix.pos += glam::vec3(0.0, 0.01, 0.0);
        }
        if screen.is_key_down(minifb::Key::Down) {
            model_matrix.pos -= glam::vec3(0.0, 0.01, 0.0);
        }
        if screen.is_key_down(minifb::Key::Up) {
            model_matrix.pos += glam::vec3(0.0, 0.01, 0.0);
        }

        println!("fps: {:.2}", starting.elapsed().as_secs_f64().recip());
        starting = std::time::Instant::now();
    }
}
