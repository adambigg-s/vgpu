use crate::vgpu::{
    gpu,
    shader::{self, Shader},
};

mod interp;
mod memory;
mod model;
mod vgpu;

#[rustfmt::skip]
const TRIANGLE: [f32; 18] = [
    -0.6, -0.4, 3.0, 1.0, 0.4, 0.0,
    0.5, -0.6, 1.0, 0.0, 1.0, 0.4,
    0.1, 0.5, 0.2, 0.4, 0.0, 1.0,
];

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pos: glam::Vec3,
    col: glam::Vec3,
}

#[repr(C, packed)]
pub struct Fragment {
    col: glam::Vec3,
}

#[repr(C, packed)]
pub struct Pipeline {
    mvp: glam::Mat4,
}

impl shader::Shader for Pipeline {
    type Vertex = Vertex;

    type Interpolant = Fragment;

    type Fragment = glam::Vec3;

    type Pixel = u32;

    #[inline]
    fn vertex(&self, vertex: &Self::Vertex, pos_out: &mut glam::Vec4) -> Self::Interpolant {
        *pos_out = self.mvp * vertex.pos.to_homogeneous();
        Fragment { col: vertex.col }
    }

    #[inline]
    fn fragment(&self, frag_vertex: &Self::Interpolant) -> Self::Fragment {
        frag_vertex.col
    }

    #[inline]
    fn pixel(&self, fragment: &Self::Fragment) -> Self::Pixel {
        let red = (fragment.x * 255.9999) as u8 as u32;
        let gre = (fragment.y * 255.9999) as u8 as u32;
        let blu = (fragment.z * 255.9999) as u8 as u32;
        0xffu32 << 24 | red << 16 | gre << 8 | blu
    }
}

const SWIDTH: usize = 400;
const SHEIGHT: usize = 300;
const SCALE: minifb::Scale = minifb::Scale::X4;
const SFILL: u32 = 0xffu32 << 24 | 25u32 << 16 | 25u32 << 8 | 40u32;

fn main() {
    let mut gpu = gpu::Gpu::new(1, 1);
    gpu.color = memory::RenderTarget::new([SWIDTH, SHEIGHT]);
    gpu.depth = memory::RenderTarget::new([SWIDTH, SHEIGHT]);

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

    let model = model::Model::new("../vendor/teapot.obj").unwrap();
    let mut model_mat = glam::Mat4::from_translation(glam::vec3(0.0, 0.0, 4.0));

    while !window.is_key_down(minifb::Key::Escape) {
        gpu.color.fill(SFILL);
        gpu.depth.fill(f32::MIN);
        let pipeline = Pipeline {
            mvp: glam::Mat4::perspective_rh_gl(90.0f32, SWIDTH as f32 / SHEIGHT as f32, 0.01, 100.0)
                * glam::Mat4::IDENTITY
                * model_mat,
        };

        for mesh in &model.meshes {
            let mut floats = Vec::new();
            mesh.vertices().for_each(|vertex| {
                let as_floats = memory::transmute::bit_interp::<Vertex, [f32; 6]>(&vertex);
                for float in as_floats {
                    floats.push(float);
                }
            });
            gpu.bind_data(&floats);
            gpu.set_vattrib_ptr(6);
            pipeline.render(&mut gpu);
        }

        if window.is_key_down(minifb::Key::A) {
            model_mat *= glam::Mat4::from_rotation_y(0.003);
        }
        if window.is_key_down(minifb::Key::D) {
            model_mat *= glam::Mat4::from_rotation_y(-0.003);
        }
        if window.is_key_down(minifb::Key::W) {
            model_mat *= glam::Mat4::from_rotation_z(0.001);
        }
        if window.is_key_down(minifb::Key::S) {
            model_mat *= glam::Mat4::from_rotation_z(-0.001);
        }

        window.update_with_buffer(&gpu.color, gpu.color.size()[0], gpu.color.size()[1]).unwrap();
    }
}
