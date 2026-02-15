#[derive(Default, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Vertex {
    pub pos: glam::Vec3,
    pub col: glam::Vec3,
    pub uv: glam::Vec2,
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    pub fn vertices(&self) -> impl Iterator<Item = Vertex> {
        self.indices.iter().map(|&index| self.vertices[index as usize])
    }

    pub fn to_flat_vertices(&self) -> Vec<f32> {
        self.vertices()
            .flat_map(|vertex| {
                [
                    vertex.pos.x,
                    vertex.pos.y,
                    vertex.pos.z,
                    vertex.col.x,
                    vertex.col.y,
                    vertex.col.z,
                    vertex.uv.x,
                    vertex.uv.y,
                ]
            })
            .collect()
    }
}

pub struct Model {
    pub model: Mesh,
}

impl Model {
    pub fn new(model_path: &'static str) -> Result<Self, String> {
        let (models, ..) = tobj::load_obj(
            model_path,
            &tobj::LoadOptions {
                single_index: true,
                triangulate: true,
                ..Default::default()
            },
        )
        .map_err(|err| err.to_string())?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        models.into_iter().for_each(|model| {
            let mut vertices_sp = Vec::new();
            #[allow(clippy::identity_op)]
            (0..(&model.mesh.positions.len() / 3)).for_each(|i| {
                vertices_sp.push(Vertex {
                    pos: glam::vec3(
                        (&model.mesh.positions)[i * 3 + 0],
                        (&model.mesh.positions)[i * 3 + 1],
                        (&model.mesh.positions)[i * 3 + 2],
                    ),
                    col: match !model.mesh.normals.is_empty() {
                        | true => glam::vec3(
                            (&model.mesh.positions)[i * 3 + 0],
                            (&model.mesh.positions)[i * 3 + 1],
                            (&model.mesh.positions)[i * 3 + 2],
                        ),
                        | false => Default::default(),
                    },
                    uv: match !model.mesh.texcoords.is_empty() {
                        | true => {
                            glam::vec2((&model.mesh.texcoords)[i * 2 + 0], (&model.mesh.texcoords)[i * 2 + 1])
                        }
                        | false => Default::default(),
                    },
                });
            });
            vertices.extend_from_slice(&vertices_sp);
            indices.extend_from_slice(&model.mesh.indices);
        });

        Ok(Model { model: Mesh { vertices, indices } })
    }
}
