#[derive(bon::Builder)]
pub struct Transform {
    #[builder(default)]
    pub scl: glam::Vec3,
    #[builder(default)]
    pub pos: glam::Vec3,
    #[builder(default)]
    pub rot: glam::Quat,
}

impl Transform {
    pub fn matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scl, self.rot, self.pos)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            scl: glam::Vec3::ONE,
            pos: glam::Vec3::ZERO,
            rot: glam::Quat::IDENTITY,
        }
    }
}

impl From<glam::Vec3> for Transform {
    fn from(pos: glam::Vec3) -> Self {
        Self { pos, ..Default::default() }
    }
}

impl From<glam::Quat> for Transform {
    fn from(rot: glam::Quat) -> Self {
        Self { rot, ..Default::default() }
    }
}
