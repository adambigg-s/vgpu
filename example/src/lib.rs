use gputils::transform;

const MOVE_SPEED: f32 = 0.01;
const ROT_SPEED: f32 = 0.01;

pub fn model_translation(model_matrix: &mut transform::Transform, screen: &minifb::Window) {
    if screen.is_key_down(minifb::Key::Down) {
        model_matrix.pos -= glam::vec3(0.0, MOVE_SPEED, 0.0);
    }
    if screen.is_key_down(minifb::Key::Up) {
        model_matrix.pos += glam::vec3(0.0, MOVE_SPEED, 0.0);
    }
    if screen.is_key_down(minifb::Key::Left) {
        model_matrix.pos -= glam::vec3(MOVE_SPEED, 0.0, 0.0);
    }
    if screen.is_key_down(minifb::Key::Right) {
        model_matrix.pos += glam::vec3(MOVE_SPEED, 0.0, 0.0);
    }
    if screen.is_key_down(minifb::Key::R) {
        model_matrix.pos -= glam::vec3(0.0, 0.0, MOVE_SPEED);
    }
    if screen.is_key_down(minifb::Key::F) {
        model_matrix.pos += glam::vec3(0.0, 0.0, MOVE_SPEED);
    }
}

pub fn model_rotation(model_matrix: &mut transform::Transform, screen: &minifb::Window) {
    if screen.is_key_down(minifb::Key::W) {
        model_matrix.rot = glam::Quat::from_rotation_x(-ROT_SPEED) * model_matrix.rot;
    }
    if screen.is_key_down(minifb::Key::S) {
        model_matrix.rot = glam::Quat::from_rotation_x(ROT_SPEED) * model_matrix.rot;
    }
    if screen.is_key_down(minifb::Key::A) {
        model_matrix.rot = glam::Quat::from_rotation_y(-ROT_SPEED) * model_matrix.rot;
    }
    if screen.is_key_down(minifb::Key::D) {
        model_matrix.rot = glam::Quat::from_rotation_y(ROT_SPEED) * model_matrix.rot;
    }
    if screen.is_key_down(minifb::Key::Q) {
        model_matrix.rot = glam::Quat::from_rotation_z(ROT_SPEED) * model_matrix.rot;
    }
    if screen.is_key_down(minifb::Key::E) {
        model_matrix.rot = glam::Quat::from_rotation_z(-ROT_SPEED) * model_matrix.rot;
    }
}
