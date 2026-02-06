#![allow(dead_code)]

use crate::Vertex;

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    pub fn vertices(&self) -> impl Iterator<Item = Vertex> {
        self.indices.iter().map(|&index| self.vertices[index as usize])
    }
}

pub struct Model {
    pub meshes: Vec<Mesh>,
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

        let mut meshes = Vec::new();
        models.into_iter().for_each(|model| {
            let mut vertices = Vec::new();
            #[allow(clippy::identity_op)]
            (0..(&model.mesh.positions.len() / 3)).for_each(|i| {
                vertices.push(Vertex {
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
                });
            });
            meshes.push(Mesh { vertices, indices: model.mesh.indices });
        });

        Ok(Model { meshes })
    }
}
