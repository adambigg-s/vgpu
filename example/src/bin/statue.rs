use glam::Vec4Swizzles;
use gputils::{
    camera,
    model::{self, texture},
    transform,
};
use virtual_gpu::{gpu, memory, shader};

const SWIDTH: usize = 256 * 4;
const SHEIGHT: usize = 196 * 4;
// const SWIDTH: usize = 1920;
// const SHEIGHT: usize = 1080;
const SSCALE: minifb::Scale = minifb::Scale::X1;
const SFILL: u32 = 0xffu32 << 24 | 220u32 << 16 | 220u32 << 8 | 200u32;
const STITLE: &str = "Statue PBR Example";

#[derive(Default)]
struct ShadowPipeline {
    mvp_matrix: glam::Mat4,
}

impl shader::Shader for ShadowPipeline {
    type Vertex = model::Vertex;

    type Interpolant = u32;

    type Fragment = u32;

    type Pixel = u32;

    #[inline(always)]
    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        *position_out = self.mvp_matrix * vertex_in.pos.to_homogeneous();
        0
    }

    #[inline(always)]
    fn fragment(&self, _: &Self::Interpolant) -> Self::Fragment {
        0
    }

    #[inline(always)]
    fn pixel(&self, _: &Self::Fragment) -> Self::Pixel {
        0
    }

    #[inline(always)]
    fn pixel_write() -> bool {
        false
    }
}

#[derive(Default)]
struct ObjectPipeline {
    model_matrix: glam::Mat4,
    mvp_matrix: glam::Mat4,
    normal_matrix: glam::Mat3,

    diffuse: texture::Texture,
    normal: texture::Texture,
    metal: texture::Texture,

    light: glam::Vec3,
    camera: glam::Vec3,
}

impl shader::Shader for ObjectPipeline {
    type Vertex = model::Vertex;

    type Interpolant = model::Vertex;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    #[inline(always)]
    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        *position_out = self.mvp_matrix * vertex_in.pos.to_homogeneous();

        let mut vertex_out = *vertex_in;
        vertex_out.pos = (self.model_matrix * vertex_in.pos.to_homogeneous()).xyz();
        vertex_out.nor = self.normal_matrix * vertex_out.nor;
        vertex_out
    }

    #[inline(always)]
    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment {
        let diffuse_map = self.diffuse.sample_bilinear(frag_vertex_in.uv.x, frag_vertex_in.uv.y);
        let normal_map = self.normal.sample_bilinear(frag_vertex_in.uv.x, frag_vertex_in.uv.y);
        let metallic_map = self.metal.sample_bilinear(frag_vertex_in.uv.x, frag_vertex_in.uv.y).x;

        let tangent_normal = normal_map * 2.0 - glam::Vec3::ONE;
        let normal = frag_vertex_in.nor.normalize();
        let tangent = if normal.y.abs() < 0.9999 {
            normal.cross(glam::Vec3::Y).normalize()
        }
        else {
            normal.cross(glam::Vec3::X).normalize()
        };
        let bitangent = normal.cross(tangent);
        let world_normal =
            (tangent * tangent_normal.x + bitangent * tangent_normal.y + normal * tangent_normal.z)
                .normalize();

        let light_dir = (self.light - frag_vertex_in.pos).normalize();
        let view_dir = (self.camera - frag_vertex_in.pos).normalize();
        let half_dir = (light_dir + view_dir).normalize();

        let ndl = world_normal.dot(light_dir).max(0.0);
        let ndh = world_normal.dot(half_dir).max(0.0);
        let ndv = world_normal.dot(view_dir).max(0.0);

        let shine = 64.0 + metallic_map * 96.0;
        let specular = ndh.powf(shine);
        let fresnel = (1.0 - ndv).powf(3.0);

        let diffuse_color = diffuse_map * (1.0 - metallic_map);
        let specular_color = glam::Vec3::splat(1.0 - metallic_map * 0.5) + diffuse_map * metallic_map;
        let ambient_color = glam::Vec3::splat(0.025);

        let diffuse_final = diffuse_color * ndl;
        let spec_final = specular_color * specular * (metallic_map * 0.5 + 0.5);
        let rim_final = specular_color * fresnel * 0.15 * metallic_map;

        (ambient_color + diffuse_final + spec_final + rim_final) * 1.15
    }

    #[inline(always)]
    fn pixel(&self, fragment_in: &Self::Fragment) -> Self::Pixel {
        let r = (fragment_in.x * 255.9999) as u8 as u32;
        let g = (fragment_in.y * 255.9999) as u8 as u32;
        let b = (fragment_in.z * 255.9999) as u8 as u32;
        0xffu32 << 24 | r << 16 | g << 8 | b
    }

    #[inline(always)]
    fn cull_mode() -> shader::CullMode {
        shader::CullMode::Back
    }
}

fn main() {
    let mut gpu = gpu::Gpu::builder()
        .vertex_cores(4)
        .raster_cores(12)
        .color(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .depth(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .build();

    // let model = model::Mesh::new("vendor/statue/lion_head_1k.obj").unwrap();
    // let mut shader = ObjectPipeline {
    //     diffuse: "vendor/statue/lion_head_diff_1k.jpg".into(),
    //     normal: "vendor/statue/lion_head_nor_gl_1k.jpg".into(),
    //     metal: "vendor/statue/lion_head_rough_1k.jpg".into(),
    //     light: glam::vec3(10.0, 25.0, 15.0),
    //     ..Default::default()
    // };

    let model = model::Mesh::new("vendor/table/ClassicConsole_01_1k.obj").unwrap();
    let mut shader = ObjectPipeline {
        diffuse: "vendor/table/ClassicConsole_01_diff_1k.jpg".into(),
        // diffuse: texture::Texture::debug_fallback(),
        normal: "vendor/table/ClassicConsole_01_nor_gl_1k.jpg".into(),
        // metal: texture::Texture::debug_fallback(),
        metal: "vendor/table/ClassicConsole_01_roughness_1k.jpg".into(),
        light: glam::vec3(10.0, 25.0, 15.0),
        ..Default::default()
    };

    let depth_shader = ShadowPipeline { ..Default::default() };
    gpu.bind_data(&model.to_flat_vertices());
    gpu.set_vattrib_ptr(8);

    let camera = camera::Camera::builder().transform(glam::vec3(0.0, 0.0, 1.0).into()).build();
    let mut model_matrix = transform::Transform::default();

    let mut screen = minifb::Window::new(
        STITLE,
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();

    let mut timer = std::time::Instant::now();
    let mut average_fps = 0.0;
    loop {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::INFINITY);

        shader.model_matrix = model_matrix.matrix();
        shader.mvp_matrix =
            camera.proj_matrix(SWIDTH as f32 / SHEIGHT as f32) * camera.view_matrix() * model_matrix.matrix();
        shader.normal_matrix = glam::Mat3::from_mat4(model_matrix.matrix()).inverse().transpose();
        shader.camera = camera.transform.pos;

        gpu.render(&depth_shader);
        gpu.render(&shader);
        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        example::model_rotation(&mut model_matrix, &screen);
        example::model_translation(&mut model_matrix, &screen);
        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }

        let elapsed = timer.elapsed().as_secs_f64().recip();
        average_fps += elapsed;
        average_fps /= 2.0;
        println!("approx fps: {:.2}", elapsed);
        timer = std::time::Instant::now();
    }

    println!("averaged fps: {:?}", average_fps);
}
