#[derive(Default, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Vertex {
    pub pos: glam::Vec3,
    pub nor: glam::Vec3,
    pub uv: glam::Vec2,
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
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
            #[rustfmt::skip]
            (0..(&model.mesh.positions.len() / 3)).for_each(|i| {
                vertices_sp.push(Vertex {
                    pos: glam::vec3(
                        (&model.mesh.positions)[i * 3 + 0],
                        (&model.mesh.positions)[i * 3 + 1],
                        (&model.mesh.positions)[i * 3 + 2],
                    ),
                    nor: match !model.mesh.normals.is_empty() {
                        | true => glam::vec3(
                            (&model.mesh.normals)[i * 3 + 0],
                            (&model.mesh.normals)[i * 3 + 1],
                            (&model.mesh.normals)[i * 3 + 2],
                        ),
                        | false => Default::default(),
                    },
                    uv: match !model.mesh.texcoords.is_empty() {
                        | true => {
                            glam::vec2(
                                (&model.mesh.texcoords)[i * 2 + 0],
                                (&model.mesh.texcoords)[i * 2 + 1],
                            )
                        }
                        | false => Default::default(),
                    },
                });
            });
            vertices.extend_from_slice(&vertices_sp);
            indices.extend_from_slice(&model.mesh.indices);
        });

        Ok(Mesh { vertices, indices })
    }

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
                    vertex.nor.x,
                    vertex.nor.y,
                    vertex.nor.z,
                    vertex.uv.x,
                    vertex.uv.y,
                ]
            })
            .collect()
    }
}

pub mod texture {
    use std::{convert, ops, path, sync};

    use virtual_gpu::memory::{self, Raster};

    pub static FALLBACK_TEXTURE: sync::LazyLock<Texture> = sync::LazyLock::new(Texture::debug_fallback);

    pub static FALLBACK_DEPTHTEX: sync::LazyLock<memory::RenderTarget<f32>> =
        sync::LazyLock::new(|| memory::RenderTarget::new([0, 0]));

    #[derive(Default)]
    pub struct Texture {
        items: memory::RenderTarget<glam::Vec3>,
        width: f32,
        height: f32,
    }

    impl Texture {
        pub fn import<P>(path: P) -> Result<Self, image::ImageError>
        where
            P: convert::AsRef<path::Path>,
        {
            let texture = image::open(path)?.flipv().into_rgb32f();
            let (width, height) = texture.dimensions();

            let texels = texture
                .into_vec()
                .chunks_exact(3)
                .map(|rgb| glam::vec3(rgb[0], rgb[1], rgb[2]))
                .collect::<Vec<glam::Vec3>>();

            Ok(Self {
                items: memory::RenderTarget::from_parts([width as usize, height as usize], texels),
                width: (width - 1) as f32,
                height: (height - 1) as f32,
            })
        }

        pub fn reference<'d>(&'d self) -> TextureRef<'d, glam::Vec3> {
            TextureRef {
                items: &self.items,
                width: self.width,
                height: self.height,
            }
        }

        pub fn sample(&self, x: f32, y: f32) -> glam::Vec3 {
            let [tx, ty] = [x.fract() * self.width, y.fract() * self.height];
            *self.items.get([tx as usize, ty as usize])
        }

        pub fn sample_bilinear(&self, x: f32, y: f32) -> glam::Vec3 {
            let [fx, fy] = [x.fract() * self.width, y.fract() * self.height];
            let [t0, t1, t2, t3] = [
                *self.items.get([fx as usize, fy as usize]),
                *self.items.get([fx as usize + 1, fy as usize]),
                *self.items.get([fx as usize, fy as usize + 1]),
                *self.items.get([fx as usize + 1, fy as usize + 1]),
            ];
            (t0 + t1 + t2 + t3) / 4.0
        }

        pub fn debug_fallback() -> Self {
            let mut buffer = memory::RenderTarget::new([8, 8]);
            (0..8).for_each(|i| {
                (0..8).for_each(|j| {
                    *buffer.get_mut([j, i]) = match (i + j) % 2 == 0 {
                        | true => glam::vec3(0.5, 0.5, 1.0),
                        | false => glam::vec3(1.0, 0.5, 0.5),
                    };
                });
            });

            Self {
                width: (buffer.size()[0] - 1) as f32,
                height: (buffer.size()[1] - 1) as f32,
                items: buffer,
            }
        }
    }

    impl<P> From<P> for Texture
    where
        P: convert::AsRef<path::Path>,
    {
        fn from(path: P) -> Self {
            Texture::import(path).unwrap()
        }
    }

    pub struct TextureRef<'d, T> {
        items: &'d memory::RenderTarget<T>,
        width: f32,
        height: f32,
    }

    impl<'d, T> TextureRef<'d, T>
    where
        T: Clone + Copy + ops::Add<T, Output = T> + ops::Div<f32, Output = T>,
    {
        pub fn sample(&self, x: f32, y: f32) -> T {
            let [tx, ty] = [x.fract() * self.width, y.fract() * self.height];
            *self.items.get([tx as usize, ty as usize])
        }

        pub fn sample_bilinear(&self, x: f32, y: f32) -> T {
            let [fx, fy] = [x.fract() * self.width, y.fract() * self.height];
            let [t0, t1, t2, t3] = [
                *self.items.get([fx as usize, fy as usize]),
                *self.items.get([fx as usize + 1, fy as usize]),
                *self.items.get([fx as usize, fy as usize + 1]),
                *self.items.get([fx as usize + 1, fy as usize + 1]),
            ];
            (t0 + t1 + t2 + t3) / 4.0
        }
    }

    impl<'d> Default for TextureRef<'d, f32> {
        fn default() -> Self {
            Self {
                items: &FALLBACK_DEPTHTEX,
                width: FALLBACK_TEXTURE.width,
                height: FALLBACK_TEXTURE.height,
            }
        }
    }

    impl<'d> Default for TextureRef<'d, glam::Vec3> {
        fn default() -> Self {
            Self {
                items: &FALLBACK_TEXTURE.items,
                width: FALLBACK_TEXTURE.width,
                height: FALLBACK_TEXTURE.height,
            }
        }
    }

    impl<'r, T> From<&'r memory::RenderTarget<T>> for TextureRef<'r, T> {
        fn from(items: &'r memory::RenderTarget<T>) -> Self {
            Self {
                items,
                width: items.width() as f32 - 1.0,
                height: items.height() as f32 - 1.0,
            }
        }
    }
}
