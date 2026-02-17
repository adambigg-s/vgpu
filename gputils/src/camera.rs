use crate::transform;

const FOV: f32 = 60.0f32.to_radians();
const ZFAR: f32 = 500.0;
const ZNEAR: f32 = 0.05;

#[derive(Default, bon::Builder)]
pub struct Camera {
    #[builder(default)]
    pub transform: transform::Transform,
    #[builder(default)]
    pub pitch: f32,
    #[builder(default)]
    pub yaw: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        self.transform.matrix().inverse()
    }

    pub fn proj_matrix(&self, ar: f32) -> glam::Mat4 {
        glam::Mat4::perspective_rh_gl(FOV, ar, ZNEAR, ZFAR)
    }
}
