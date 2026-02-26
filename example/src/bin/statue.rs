use std::mem;

use glam::{Vec3Swizzles, Vec4Swizzles};
use gputils::{
    camera,
    model::{self, texture},
    transform,
};
use virtual_gpu::{gpu, memory, shader};

const SWIDTH: usize = 1920;
const SHEIGHT: usize = 1080;
const SSCALE: minifb::Scale = minifb::Scale::X1;
const SFILL: u32 = 0xffu32 << 24 | 220u32 << 16 | 220u32 << 8 | 200u32;
const STITLE: &str = "Statue PBR Example";

#[derive(Default)]
struct Object<S>
where
    S: shader::Shader,
{
    mesh: Vec<f32>,
    material: PbrMaterial,
    transform: transform::Transform,
    shader: S,
}

#[derive(Default, Clone, Copy)]
pub struct PbrMaterial {
    albedo_tint: glam::Vec3,
    emissive: glam::Vec3,
    rough_factor: f32,
    metal_factor: f32,
    ao_factor: f32,
    reflect_factor: f32,
    normal_factor: f32,
}

#[derive(Default)]
struct PbrTextures {
    diffuse: texture::Texture,
    normals: texture::Texture,
    ao_r_ms: texture::Texture,
}

#[derive(Default)]
struct ShadowPipeline {
    vp_matrix: glam::Mat4,
    m_matrix: glam::Mat4,
}

impl shader::Shader for ShadowPipeline {
    type Vertex = model::Vertex;

    type Interpolant = f32;

    type Fragment = f32;

    type Pixel = u32;

    #[inline(always)]
    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        *position_out = self.vp_matrix * self.m_matrix * vertex_in.pos.to_homogeneous();
        position_out.z / position_out.w
    }

    #[inline(always)]
    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment {
        *frag_vertex_in
    }

    #[inline(always)]
    fn pixel(&self, frag: &Self::Fragment) -> Self::Pixel {
        debug_assert!(&-1.1 < frag && frag < &1.1, "NDC frag depth: {}", frag);

        let normalized_light = (*frag + 1.0) * 0.5;
        let attenuation = (normalized_light * 255.9999) as u8 as u32;
        0xffu32 << 24 | attenuation << 16 | attenuation << 8 | attenuation
    }

    #[inline(always)]
    fn cull_mode() -> shader::CullMode {
        shader::CullMode::None
    }
}

#[derive(Default)]
struct PbrRenderPipeline<'r> {
    model_matrix: glam::Mat4,
    mvp_matrix: glam::Mat4,
    normal_matrix: glam::Mat3,

    textures: PbrTextures,
    material: PbrMaterial,

    light_vp: glam::Mat4,
    light_depth: texture::TextureRef<'r, f32>,
    light_intensity: f32,
    light: glam::Vec3,
    camera: glam::Vec3,
}

#[derive(Default)]
#[repr(C, packed)]
struct Interpolant {
    pos: glam::Vec3,
    nor: glam::Vec3,
    light_view_pos: glam::Vec3,
    uv: glam::Vec2,
}

impl<'r> shader::Shader for PbrRenderPipeline<'r> {
    type Vertex = model::Vertex;

    type Interpolant = Interpolant;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    #[inline(always)]
    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant {
        *position_out = self.mvp_matrix * vertex_in.pos.to_homogeneous();

        let world_pos = self.model_matrix * vertex_in.pos.to_homogeneous();
        let light_view_pos = self.light_vp * world_pos;

        Interpolant {
            pos: (self.model_matrix * vertex_in.pos.to_homogeneous()).xyz(),
            nor: self.normal_matrix * vertex_in.nor,
            light_view_pos: light_view_pos.xyz() / light_view_pos.w,
            uv: vertex_in.uv,
        }
    }

    #[inline(always)]
    fn fragment(&self, frag: &Self::Interpolant) -> Self::Fragment {
        const PI: f32 = std::f32::consts::PI;
        const GAMMA_MOD: f32 = 2.2;

        let diffuse_map = self.textures.diffuse.sample_bilinear(frag.uv.x, frag.uv.y);
        let arm_map = self.textures.ao_r_ms.sample_bilinear(frag.uv.x, frag.uv.y);
        let normal_map = self.textures.normals.sample_bilinear(frag.uv.x, frag.uv.y);

        let albedo = diffuse_map.powf(GAMMA_MOD) * self.material.albedo_tint;
        let ao = (arm_map.x * self.material.ao_factor).clamp(0.0, 1.0);
        let rough = (arm_map.y * self.material.rough_factor).clamp(0.05, 1.0);
        let metal = (arm_map.z * self.material.metal_factor).clamp(0.0, 1.0);

        let light_uv = frag.light_view_pos.xy() * glam::Vec2::new(1.0, -1.0) * 0.5 + 0.5;
        let frag_depth = frag.light_view_pos.z;
        let shadow_depth = self.light_depth.sample_bilinear(light_uv.x, light_uv.y);
        let shadow = if frag_depth > shadow_depth + 0.0015 { 0.0 } else { 1.0 };

        let tn = normal_map * 2.0 - glam::Vec3::ONE;
        let tan_norm =
            glam::vec3(tn.x * self.material.normal_factor, tn.y * self.material.normal_factor, tn.z)
                .normalize();
        let norm = frag.nor.normalize();
        let tan = if norm.y.abs() < 0.9999 {
            norm.cross(glam::Vec3::Y).normalize()
        }
        else {
            norm.cross(glam::Vec3::X).normalize()
        };
        let bi_tan = norm.cross(tan);
        let n = (tan * tan_norm.x + bi_tan * tan_norm.y + norm * tan_norm.z).normalize();

        let l = (self.light - frag.pos).normalize();
        let v = (self.camera - frag.pos).normalize();
        let h = (l + v).normalize();

        let ndl = n.dot(l).max(0.0);
        let ndv = n.dot(v).max(0.0);
        let ndh = n.dot(h).max(0.0);
        let hdv = h.dot(v).max(0.0);

        let f0 = glam::Vec3::splat(self.material.reflect_factor).lerp(albedo, metal);
        let alpha = rough * rough;
        let alpha2 = alpha * alpha;

        let demon_d = ndh * ndh * (alpha2 - 1.0) + 1.0;
        let d = alpha2 / (PI * demon_d * demon_d + 1e-6);

        let k = (rough + 1.0).powi(2) / 8.0;
        let g = ndv / (ndv * (1.0 - k) + k) * (ndl / (ndl * (1.0 - k) + k));

        let f = f0 + (glam::Vec3::ONE - f0) * (1.0 - hdv).powf(5.0);

        let specular = (d * g * f) / (4.0 * ndv * ndl + 1e-6);

        let kd = (glam::Vec3::ONE - f0) * (1.0 - metal);
        let diffuse = kd * albedo / PI;

        let distance = (self.light - frag.pos).length();
        let attenutation = (distance * distance).recip();
        let radiance = glam::Vec3::splat(self.light_intensity) * attenutation;

        let direct = (diffuse + specular) * radiance * ndl * shadow;
        let ambient = glam::Vec3::splat(0.06) * albedo * ao;

        ambient + direct + self.material.emissive
    }

    #[inline(always)]
    fn pixel(&self, fragment_in: &Self::Fragment) -> Self::Pixel {
        const GAMMA: f32 = 1.0 / 2.2;
        let color = (*fragment_in / (*fragment_in + glam::Vec3::ONE)).powf(GAMMA);
        let r = (color.x * 255.9999) as u8 as u32;
        let g = (color.y * 255.9999) as u8 as u32;
        let b = (color.z * 255.9999) as u8 as u32;
        0xff_u32 << 24 | r << 16 | g << 8 | b
    }

    #[inline(always)]
    fn cull_mode() -> shader::CullMode {
        shader::CullMode::Back
    }
}

fn main() {
    let mut gpu = gpu::Gpu::builder()
        .vertex_cores(4)
        .raster_cores(16)
        .color(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .depth(memory::RenderTarget::new([SWIDTH, SHEIGHT]))
        .build();

    let mut shadow_depth = memory::RenderTarget::new([SWIDTH, SHEIGHT]);
    let mut depth_shader = ShadowPipeline { ..Default::default() };

    let mut statue = Object {
        mesh: model::Mesh::new("vendor/statue/lion_head_1k.obj").unwrap().to_flat_vertices(),
        material: PbrMaterial {
            albedo_tint: glam::Vec3::ONE,
            emissive: glam::Vec3::ZERO,
            rough_factor: 0.7,
            metal_factor: 1.3,
            ao_factor: 0.9,
            reflect_factor: 0.05,
            normal_factor: 1.0,
        },
        transform: transform::Transform::builder()
            .pos(glam::vec3(0.11999998, -0.29999998, 0.08999999))
            .rot(glam::quat(0.0, 0.119329154, 0.0, 0.9896322))
            .scl(glam::Vec3::splat(1.67))
            .build(),
        shader: PbrRenderPipeline {
            textures: PbrTextures {
                diffuse: "vendor/statue/lion_head_diff_1k.jpg".into(),
                normals: "vendor/statue/lion_head_nor_gl_1k.jpg".into(),
                ao_r_ms: "vendor/statue/lion_head_arm_1k.jpg".into(),
            },
            ..Default::default()
        },
    };

    let mut table = Object {
        mesh: model::Mesh::new("vendor/table/ClassicConsole_01_1k.obj").unwrap().to_flat_vertices(),
        material: PbrMaterial {
            albedo_tint: glam::Vec3::splat(0.95),
            emissive: glam::Vec3::ZERO,
            rough_factor: 1.0,
            metal_factor: 0.5,
            ao_factor: 0.7,
            reflect_factor: 0.02,
            normal_factor: 1.4,
        },
        transform: transform::Transform::builder()
            .pos(glam::vec3(0.08999999, -1.2499992, 0.07999999))
            .rot(glam::quat(0.0, 0.09485673, 0.0, 0.99548596))
            .scl(glam::Vec3::ONE)
            .build(),
        shader: PbrRenderPipeline {
            textures: PbrTextures {
                diffuse: "vendor/table/ClassicConsole_01_diff_1k.jpg".into(),
                normals: "vendor/table/ClassicConsole_01_nor_gl_1k.jpg".into(),
                ao_r_ms: "vendor/table/ClassicConsole_01_arm_1k.jpg".into(),
            },
            ..Default::default()
        },
    };

    gpu.set_vattrib_ptr(8);

    let camera = camera::Camera::builder().transform(glam::vec3(0.0, 0.0, 1.0).into()).build();
    let light = camera::Camera::builder().transform(glam::vec3(25.0, 25.0, 10.0).into()).build();
    let brightness = 10000.0;

    let mut screen = minifb::Window::new(
        STITLE,
        SWIDTH,
        SHEIGHT,
        minifb::WindowOptions { scale: SSCALE, ..Default::default() },
    )
    .unwrap();

    loop {
        mem::swap(&mut shadow_depth, &mut gpu.depth);
        gpu.depth.fill(f32::INFINITY);

        depth_shader.vp_matrix = light.ortho_matrix()
            * glam::Mat4::look_at_rh(light.transform.pos.normalize(), glam::Vec3::ZERO, glam::Vec3::Y);

        depth_shader.m_matrix = table.transform.matrix();
        gpu.bind_data(&table.mesh);
        gpu.render(&depth_shader);
        depth_shader.m_matrix = statue.transform.matrix();
        gpu.bind_data(&statue.mesh);
        gpu.render(&depth_shader);

        table.shader.model_matrix = table.transform.matrix();
        table.shader.mvp_matrix = camera.proj_matrix(SWIDTH as f32 / SHEIGHT as f32)
            * camera.view_matrix()
            * table.shader.model_matrix;
        table.shader.normal_matrix = glam::Mat3::from_mat4(table.shader.model_matrix).inverse().transpose();
        table.shader.camera = camera.transform.pos;
        table.shader.light = light.transform.pos;
        table.shader.light_intensity = brightness;
        table.shader.light_depth = unsafe {
            let depth = &shadow_depth as *const memory::RenderTarget<f32>;
            (&*depth).into()
        };
        table.shader.light_vp = depth_shader.vp_matrix;
        table.shader.material = table.material;

        statue.shader.model_matrix = statue.transform.matrix();
        statue.shader.mvp_matrix = camera.proj_matrix(SWIDTH as f32 / SHEIGHT as f32)
            * camera.view_matrix()
            * statue.shader.model_matrix;
        statue.shader.normal_matrix = glam::Mat3::from_mat4(statue.shader.model_matrix).inverse().transpose();
        statue.shader.camera = camera.transform.pos;
        statue.shader.light = light.transform.pos;
        statue.shader.light_intensity = brightness;
        statue.shader.light_depth = unsafe {
            let depth = &shadow_depth as *const memory::RenderTarget<f32>;
            (&*depth).into()
        };
        statue.shader.light_vp = depth_shader.vp_matrix;
        statue.shader.material = statue.material;

        mem::swap(&mut shadow_depth, &mut gpu.depth);
        gpu.depth.fill(f32::INFINITY);
        gpu.color.fill(SFILL);

        gpu.bind_data(&table.mesh);
        gpu.render(&table.shader);
        gpu.bind_data(&statue.mesh);
        gpu.render(&statue.shader);

        screen.update_with_buffer(&gpu.color, SWIDTH, SHEIGHT).unwrap();

        example::model_rotation(&mut statue.transform, &screen);
        example::model_translation(&mut statue.transform, &screen);
        if screen.is_key_down(minifb::Key::Escape) {
            break;
        }
    }
}
