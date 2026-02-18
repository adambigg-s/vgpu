use std::clone;

use crate::{
    gpu,
    memory::{self, stack},
    shader,
};

const REGISTER_SIZE: usize = 12;
const VAO_SIZE: usize = 1;

#[derive(Default, Debug, bon::Builder)]
pub struct Gpu<P, D> {
    #[builder(with = |num: usize| memory::Array::new([num]))]
    pub vertex_cores: memory::Array<cores::VertexCore>,
    #[builder(with = |num: usize| memory::Array::new([num]))]
    pub raster_cores: memory::Array<cores::RasterCore>,
    #[builder(default)]
    pub vscheduler: control::Scheduler,
    #[builder(default)]
    pub rscheduler: control::Scheduler,
    #[builder(default)]
    pub tiles: memory::Array<cores::Tile>,

    #[builder(default)]
    pub vao_layout: stack::Vec<control::VaoPointer, { gpu::VAO_SIZE }>,
    #[builder(default)]
    pub vao_raw: memory::Array<f32>,
    #[builder(default)]
    pub vao_queue: memory::Array<cores::ProcessedVertex>,

    pub color: P,
    pub depth: D,
}

impl<P, D> Gpu<P, D>
where
    P: Default + Send + Sync,
    D: Default + Send + Sync,
{
    pub fn new(vcores: usize, rcores: usize) -> Self {
        Self {
            vertex_cores: memory::Array::new([vcores]),
            raster_cores: memory::Array::new([rcores]),
            ..Default::default()
        }
    }

    pub fn bind_data(&mut self, data: &Vec<f32>) {
        self.vao_raw = memory::Array::from_parts([data.len()], clone::Clone::clone(data));
    }

    pub fn set_vattrib_ptr(&mut self, stride: usize) {
        if !self.vao_layout.is_empty() {
            self.vao_layout.pop();
        }
        debug_assert!(self.vao_layout.len() < self.vao_layout.capacity());
        self.vao_layout.push(control::VaoPointer { stride });
    }

    pub fn render<S>(&mut self, program: &S)
    where
        S: shader::Shader + Send + Sync,
        P: memory::Raster<Item = S::Pixel>,
        D: memory::Raster<Item = f32>,
    {
        debug_assert!(
            self.color.size() == self.depth.size()
                || self.color.size() == [0, 0] && self.depth.size() != [0, 0]
                || self.color.size() != [0, 0] && self.depth.size() == [0, 0]
                || S::pixel_write() && !S::depth_write()
                || !S::pixel_write() && S::depth_write(),
            "Buffer dimensions must match or ONE buffer must be zero-sized or write-disabled"
        );
        self.vao_queue = memory::Array::new([self.vao_raw.len() / self.vao_layout.peek().stride]);
        self.vscheduler.vertex_stage(
            &mut self.vertex_cores,
            &mut self.vao_queue,
            &self.vao_raw,
            self.vao_layout.peek(),
            &self.color,
            program,
        );
        self.rscheduler.raster_stage(
            &mut self.raster_cores,
            &mut self.color,
            &mut self.depth,
            &self.vao_queue,
            program,
        );
    }
}

mod control {
    use std::{sync::atomic, thread};

    use crate::{gpu::cores, memory, shader};

    #[derive(Default, Debug)]
    pub struct VaoPointer {
        pub stride: usize,
    }

    #[derive(Default, Debug)]
    pub struct Scheduler {
        pub head: atomic::AtomicUsize,
    }

    impl Scheduler {
        pub fn reset(&self) {
            self.head.store(Default::default(), atomic::Ordering::Relaxed);
        }

        pub fn incomplete(&self, data: &memory::Array<f32>) -> bool {
            self.head.load(atomic::Ordering::Relaxed) < data.len()
        }

        pub fn vertex_stage<S, P>(
            &self,
            cores: &mut memory::Array<cores::VertexCore>,
            queue: &mut memory::Array<cores::ProcessedVertex>,
            vao: &memory::Array<f32>,
            layout: &VaoPointer,
            viewport: &P,
            program: &S,
        ) where
            S: shader::Shader + Send + Sync,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
        {
            self.reset();
            thread::scope(|scope| {
                cores.iter_mut().for_each(|core| {
                    scope.spawn(|| {
                        let queue = queue as *const memory::Array<cores::ProcessedVertex>
                            as *mut memory::Array<cores::ProcessedVertex>;
                        unsafe { core.work(&mut *queue, vao, layout, self, viewport, program) }
                    });
                });
            })
        }

        pub fn raster_stage<S, P, D>(
            &self,
            cores: &mut memory::Array<cores::RasterCore>,
            color: &mut P,
            depth: &mut D,
            queue: &memory::Array<cores::ProcessedVertex>,
            program: &S,
        ) where
            S: shader::Shader + Send + Sync,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
            self.reset();
            thread::scope(|scope| {
                cores.iter_mut().for_each(|core| {
                    scope.spawn(|| {
                        let depth = depth as *const D as *mut D;
                        let color = color as *const P as *mut P;
                        unsafe {
                            core.work(&mut *color, &mut *depth, self, queue, program);
                        }
                    });
                });
            })
        }
    }
}

mod cores {
    use std::sync::atomic;

    use glam::Vec4Swizzles;

    use crate::{
        gpu::{self, aabb, control, cull},
        interp,
        memory::{self, transmute},
        shader,
    };

    pub type Tile = aabb::AaBb<f32, 2>;

    #[derive(Default, Debug)]
    pub struct ProcessedVertex {
        pub attribs: FloatRegister,
        pub pos: glam::Vec4,
    }

    type FloatRegister = memory::Vector<f32, { gpu::REGISTER_SIZE }>;

    #[derive(Default, Debug)]
    pub struct VertexCore {}

    impl VertexCore {
        pub fn work<S, P>(
            &self,
            queue: &mut memory::Array<ProcessedVertex>,
            vao: &memory::Array<f32>,
            layout: &control::VaoPointer,
            scheduler: &control::Scheduler,
            viewport: &P,
            program: &S,
        ) where
            S: shader::Shader,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
        {
            debug_assert!(
                size_of::<S::Vertex>() < size_of::<FloatRegister>()
                    && size_of::<S::Interpolant>() < size_of::<FloatRegister>(),
                "Vertex attributes are too large: {}\nVertex size: {}\nInterpolant size: {}",
                gpu::REGISTER_SIZE,
                size_of::<S::Vertex>(),
                size_of::<S::Interpolant>(),
            );

            let [hw, hh] = viewport.size().map(|dim| dim as f32 / 2.0);
            loop {
                let index = scheduler.head.fetch_add(1, atomic::Ordering::Relaxed);
                let head = index * layout.stride;
                let tail = head + layout.stride;
                if tail > vao.len() {
                    break;
                }

                let data = &vao[head..tail];
                let vertex = transmute::bit_interp::<&[f32], &S::Vertex>(&data);

                let mut position = glam::Vec4::default();
                let vs_response = program.vertex(vertex, &mut position);
                let mut attributes = transmute::bit_interp::<S::Interpolant, FloatRegister>(&vs_response);

                position = {
                    let inv_depth = position.w.recip();
                    attributes = attributes * inv_depth;
                    position *= inv_depth;
                    position.w = inv_depth;
                    position.x = position.x * hw + hw;
                    position.y = -position.y * hh + hh;
                    position
                };

                queue[index] = ProcessedVertex { attribs: attributes, pos: position }
            }
        }
    }

    #[derive(Default, Debug)]
    pub struct RasterCore {
        pub attributes: [FloatRegister; 3],
        pub positions: [glam::Vec4; 3],
        pub tile: Tile,
    }

    impl RasterCore {
        pub fn work<S, P, D>(
            &mut self,
            color: &mut P,
            depth: &mut D,
            scheduler: &control::Scheduler,
            queue: &memory::Array<ProcessedVertex>,
            program: &S,
        ) where
            S: shader::Shader + Send + Sync,
            P: Default + memory::Raster<Item = S::Pixel> + Send + Sync,
            D: Default + memory::Raster<Item = f32> + Send + Sync,
        {
            loop {
                let head = scheduler.head.fetch_add(3, atomic::Ordering::Relaxed);
                let tail = head + 3;
                if tail > queue.len() {
                    break;
                }

                let data = &queue[head..tail];
                (0..3).for_each(|idx| {
                    self.attributes[idx] = data[idx].attribs;
                    self.positions[idx] = data[idx].pos;
                });

                match S::cull_mode() {
                    | shader::CullMode::Front => {
                        if cull::triangle_cww(self.positions) {
                            continue;
                        }
                    }
                    | shader::CullMode::Back => {
                        if cull::triangle_ccww(self.positions) {
                            continue;
                        }
                    }
                    | shader::CullMode::None => (),
                }

                let [mut minx, mut miny, mut maxx, mut maxy] = self.bounding_box();
                [minx, miny, maxx, maxy] = [
                    minx.max(0.0),
                    miny.max(0.0),
                    maxx.min((color.width() - 1) as f32),
                    maxy.min((color.height() - 1) as f32),
                ];
                let interp = interp::BarycentricSystem::from_points(self.positions.map(|vertex| vertex.xy()));

                for row in miny as i32..=maxy as i32 {
                    for col in minx as i32..=maxx as i32 {
                        let point = glam::vec2(col as f32 + 0.5, row as f32 + 0.5);
                        let lambdas = interp.sample_point(point);
                        if !interp.surrounds(lambdas) {
                            continue;
                        }

                        let mut pos = interp::weighted_sum(self.positions, lambdas.to_array());
                        let distance = pos.z;
                        match S::depth_test() {
                            | shader::DepthMode::WriteGreater => {
                                if &distance > depth.peek(col as usize, row as usize) {
                                    continue;
                                }
                            }
                            | shader::DepthMode::WriteLess => {
                                if &distance < depth.peek(col as usize, row as usize) {
                                    continue;
                                }
                            }
                            | shader::DepthMode::NoWrite => (),
                        }

                        let inv_depth = pos.w.recip();
                        pos *= inv_depth;
                        let interp = interp::weighted_sum(
                            *memory::Vector::from(self.attributes.map(memory::Vector::from)),
                            lambdas.to_array(),
                        ) * inv_depth;
                        let fragment = program
                            .fragment(transmute::bit_interp::<&FloatRegister, &S::Interpolant>(&&interp));
                        let pixel = program.pixel(&fragment);

                        debug_assert!(
                            color.width() > col as usize && color.height() > row as usize,
                            "indices: {}, {}",
                            col,
                            row
                        );

                        if S::pixel_write() {
                            *color.get(col as usize, row as usize) = pixel;
                        }
                        if S::depth_write() {
                            *depth.get(col as usize, row as usize) = distance;
                        }
                    }
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

mod cull {
    use glam::Vec4Swizzles;

    pub fn triangle_cww(vertices: [glam::Vec4; 3]) -> bool {
        let [v1, v2, v3] = vertices.map(|vertex| vertex.xy());
        (v2 - v1).perp_dot(v3 - v1).is_sign_negative()
    }

    pub fn triangle_ccww(vertices: [glam::Vec4; 3]) -> bool {
        let [v1, v2, v3] = vertices.map(|vertex| vertex.xy());
        (v2 - v1).perp_dot(v3 - v1).is_sign_positive()
    }
}

mod aabb {
    use std::cmp;

    #[derive(Debug)]
    pub struct AaBb<T, const N: usize> {
        low: [T; N],
        high: [T; N],
    }

    impl<T, const N: usize> AaBb<T, N> {
        pub fn new(low: [T; N], high: [T; N]) -> Self {
            Self { low, high }
        }

        pub fn overlaps(&self, other: &Self) -> bool
        where
            T: cmp::PartialOrd,
        {
            (0..N).all(|dim| self.low[dim] <= other.high[dim] && self.high[dim] >= other.low[dim])
        }
    }

    impl<T, const N: usize> Default for AaBb<T, N>
    where
        T: Default + Clone + Copy,
    {
        fn default() -> Self {
            Self::new([T::default(); N], [T::default(); N])
        }
    }
}
