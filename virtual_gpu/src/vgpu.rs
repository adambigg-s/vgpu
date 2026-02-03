pub mod cores {
    use glam::Vec3Swizzles;

    use crate::{interp, memory, vgpu::shader};

    pub type GenericVertex = [f32; 9];

    #[derive(Default, Debug)]
    pub struct GeometryCore {
        pub vertices_in: [GenericVertex; 3],
        pub attribs_out: [GenericVertex; 3],
        pub positions_out: [glam::Vec3; 3],
    }

    impl GeometryCore {
        pub fn process<S>(&mut self, program: &S)
        where
            S: shader::Shader,
        {
            debug_assert!(
                size_of::<S::Vertex>() < size_of::<GenericVertex>()
                    && size_of::<S::VertexAttribs>() < size_of::<GenericVertex>(),
                "Vertex attributes are too large for core buffers"
            );

            for i in 0..3 {
                let vertex = unsafe { &*(self.vertices_in[i].as_ptr() as *const S::Vertex) };
                let (pos, attrib) = program.vertex(vertex);
                let attrib = unsafe { *(&attrib as *const S::VertexAttribs as *const GenericVertex) };
                self.attribs_out[i] = attrib;
                self.positions_out[i] = pos;
            }
        }
    }

    #[derive(Default, Debug)]
    pub struct RasterCore {
        pub attribs_in: [GenericVertex; 3],
        pub positions: [glam::Vec3; 3],
    }

    impl RasterCore {
        pub fn process<S, P, D>(&mut self, program: &S, color: &mut P, depth: &mut D)
        where
            S: shader::Shader,
            P: memory::Raster<Item = u32>,
            D: memory::Raster<Item = f32>,
        {
            let [hw, hh] = color.size().map(|dim| dim as f32 / 2.0);
            self.positions = self.positions.map(|mut vertex| {
                vertex.x = vertex.x * hw + hw;
                vertex.y = -vertex.y * hh + hh;
                vertex
            });

            let [mut minx, mut miny, mut maxx, mut maxy] = self.bounding_box();
            [minx, miny, maxx, maxy] = [
                minx.max(0.0),
                miny.max(0.0),
                maxx.min(color.size()[0] as f32),
                maxy.min(color.size()[1] as f32),
            ];
            let interp = interp::BarycentricSystem::from_points(self.positions.map(|vertex| vertex.xy()));
            for row in miny as i32..maxy as i32 {
                for col in minx as i32..maxx as i32 {
                    let point = glam::vec2(col as f32, row as f32);
                    let lambdas = interp.sample_point(point);
                    if !interp.surrounds(lambdas) {
                        continue;
                    }

                    let attribs = interp::Vector::from(self.attribs_in.map(interp::Vector::from));
                    let interp = interp::weighted_sum(*attribs, lambdas.to_array()).to_array();
                    let frag_vertex = unsafe { &*(interp.as_ptr() as *const S::VertexAttribs) };
                    let fragment = program.fragment(frag_vertex);
                    let pixel = program.pixel(&fragment);
                    let colr = unsafe { *(&pixel as *const S::Pixel as *const u32) };
                    debug_assert!(
                        color.width() > col as usize && color.height() > row as usize,
                        "indices: {}, {}",
                        col,
                        row
                    );
                    *color.get(col as usize, row as usize) = colr;
                    // *depth.get(col as usize, row as usize) = fragment.z;
                }
            }
        }

        fn bounding_box(&self) -> [f32; 4] {
            let [mut minx, mut miny] = [f32::INFINITY, f32::INFINITY];
            let [mut maxx, mut maxy] = [f32::NEG_INFINITY, f32::NEG_INFINITY];
            self.positions.iter().for_each(|vertex| {
                minx = minx.min(vertex.x);
                miny = miny.min(vertex.y);
                maxx = maxx.max(vertex.x);
                maxy = maxy.max(vertex.y);
            });
            [minx, miny, maxx, maxy]
        }
    }
}

#[allow(unused_imports)]
#[allow(dead_code)]
#[allow(unused)]
pub mod gpu {
    use std::clone;

    use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};

    use crate::{
        memory::{self, Raster, stack},
        vgpu::{
            cores,
            shader::{self},
        },
    };

    #[derive(Default, Debug)]
    pub struct VaoPointer {
        location: usize,
        size: usize,
        stride: usize,
        offset: usize,
    }

    #[derive(Default, Debug)]
    pub struct Scheduler {
        head: usize,
    }

    impl Scheduler {
        pub fn load_geometry_cores(
            &mut self,
            cores: &mut memory::Array<cores::GeometryCore>,
            data: &memory::Array<f32>,
        ) {
            const VSIZE: usize = 3;
            self.head = 0;

            #[allow(clippy::identity_op)]
            #[allow(clippy::erasing_op)]
            for i in 0..cores.len() {
                if data.len() < self.head + VSIZE * 3 {
                    break;
                }

                debug_assert!(
                    data.len() >= self.head + VSIZE * 3,
                    "data: {}\nhead: {}",
                    data.len(),
                    self.head
                );

                let v1 = glam::Vec3::from_slice(&data[self.head + VSIZE * 0..self.head + VSIZE * 1]);
                let v2 = glam::Vec3::from_slice(&data[self.head + VSIZE * 1..self.head + VSIZE * 2]);
                let v3 = glam::Vec3::from_slice(&data[self.head + VSIZE * 2..self.head + VSIZE * 3]);
                // cores[i].vertices_in = [v1, v2, v3];
                let vertices = [v1, v2, v3];
                for j in 0..3 {
                    for k in 0..3 {
                        cores[i].vertices_in[j][k] = vertices[j].to_array()[k];
                    }
                }

                self.head += VSIZE * 3;
            }
            // for i in 0..cores.len() {
            //     if data.len() < self.head + VSIZE * 3 {
            //         break;
            //     }

            //     for j in 0..3 {
            //         for k in 0..3 {
            //             cores[i].vertices_in[j][k] = 0.0;
            //         }
            //     }
            // }
        }

        pub fn load_raster_cores(
            &mut self,
            cores: &mut memory::Array<cores::RasterCore>,
            data: &memory::Array<cores::GeometryCore>,
        ) {
            debug_assert!(cores.len() == data.len());
            for i in 0..cores.len() {
                cores[i].attribs_in = data[i].attribs_out;
                cores[i].positions = data[i].positions_out;
            }
        }
    }

    #[derive(Default, Debug)]
    pub struct Gpu<P, D> {
        pub vertex_cores: memory::Array<cores::GeometryCore>,
        pub raster_cores: memory::Array<cores::RasterCore>,
        pub scheduler: Scheduler,

        pub color: P,
        pub depth: D,

        pub vao: memory::Array<f32>,
        pub vao_layout: stack::Vec<VaoPointer, 1>,
    }

    impl<P, D> Gpu<P, D>
    where
        P: Default + Raster<Item = u32> + Send + Sync,
        D: Default + Raster<Item = f32> + Send + Sync,
    {
        pub fn new(vcores: usize, rcores: usize) -> Self {
            Self {
                vertex_cores: memory::Array::new([vcores]),
                raster_cores: memory::Array::new([rcores]),
                ..Default::default()
            }
        }

        pub fn bind_data(&mut self, data: &Vec<f32>) {
            self.vao = memory::Array::from_parts([data.len()], clone::Clone::clone(data));
        }

        pub fn cycle_vertex_cores<S>(&mut self, program: &S)
        where
            S: shader::Shader + Send + Sync,
        {
            // Parallel
            self.vertex_cores.par_iter_mut().for_each(|core| {
                core.process(program);
            });

            // // Sequential
            // self.vertex_cores.iter_mut().for_each(|core| {
            //     core.process(program);
            // });
        }

        pub fn cycle_raster_cores<S>(&mut self, program: &S)
        where
            S: shader::Shader + Send + Sync,
        {
            // Parallel
            let color = &self.color;
            let depth = &self.depth;
            #[allow(invalid_reference_casting)]
            unsafe {
                self.raster_cores.par_iter_mut().for_each(|core| {
                    let color = color as *const P as *mut P;
                    let depth = depth as *const D as *mut D;
                    core.process(program, &mut *color, &mut *depth);
                });
            }

            // // Sequential
            // self.raster_cores.iter_mut().for_each(|core| {
            //     core.process(program, &mut self.color, &mut self.depth);
            // });
        }
    }
}

#[allow(dead_code)]
#[allow(unused_variables)]
pub mod shader {
    use crate::{memory, vgpu::gpu};

    pub trait Shader {
        type Vertex;

        type VertexAttribs;

        type Fragment;

        type Pixel;

        fn vertex(&self, vertex: &Self::Vertex) -> (glam::Vec3, Self::VertexAttribs);

        fn fragment(&self, frag_vertex: &Self::VertexAttribs) -> Self::Fragment;

        fn pixel(&self, fragment: &Self::Fragment) -> Self::Pixel;

        /// !!! NO OVERRIDE !!!
        fn render<P, D>(&self, target: &mut gpu::Gpu<P, D>)
        where
            Self: Sized + Send + Sync,
            P: Default + memory::Raster<Item = u32> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
            debug_assert!(target.color.size() == target.depth.size(), "Buffer dimensions must match");
            target.scheduler.load_geometry_cores(&mut target.vertex_cores, &target.vao);
            target.cycle_vertex_cores(self);
            target.scheduler.load_raster_cores(&mut target.raster_cores, &target.vertex_cores);
            target.cycle_raster_cores(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::vgpu::shader;

    #[test]
    fn ridiculous_transmute() {
        struct Pipeline;
        impl shader::Shader for Pipeline {
            type Vertex = [f32; 6];

            type VertexAttribs = [f32; 6];

            type Fragment = [f32; 3];

            type Pixel = u32;

            fn vertex(&self, _: &Self::Vertex) -> (glam::Vec3, Self::VertexAttribs) {
                todo!()
            }

            fn fragment(&self, _: &Self::VertexAttribs) -> Self::Fragment {
                todo!()
            }

            fn pixel(&self, _: &Self::Fragment) -> Self::Pixel {
                todo!()
            }
        }

        fn generic_pipe_fn<S>(_: S, v: &[f32; 32])
        where
            S: shader::Shader,
        {
            unsafe {
                assert!(size_of_val(v) == 32 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Pixel)) == size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Vertex)) == 6 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Fragment)) == 3 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::VertexAttribs)) == 6 * size_of::<f32>());
            }
        }

        let shader = Pipeline;
        generic_pipe_fn(shader, &[Default::default(); 32]);
    }
}

// #[allow(dead_code)]
// #[allow(unused_variables)]
// pub mod shader {
//     use crate::{memory, vgpu::gpu};

//     pub trait Shader {
//         fn vertex(&self, vertex: glam::Vec3) -> glam::Vec3;

//         fn fragment(&self, frag_vertex: glam::Vec3) -> glam::Vec3;

//         fn pixel(&self, fragment: glam::Vec3) -> u32;

//         /// !!! NO OVERRIDE !!!
//         fn render<P, D>(&self, target: &mut gpu::Gpu<P, D>)
//         where
//             Self: Sized + Send + Sync,
//             P: Default + memory::Raster<Item = u32> + Send + Sync,
//             D: Default + memory::Raster<Item = f32> + Send + Sync,
//         {
//             debug_assert!(target.color.size() == target.depth.size(), "Buffer dimensions must match");
//             target.scheduler.load_geometry_cores(&mut target.vertex_cores, &target.vao);
//             target.cycle_vertex_cores(self);
//             target.scheduler.load_raster_cores(&mut target.raster_cores, &target.vertex_cores);
//             target.cycle_raster_cores(self);
//         }
//     }
// }
