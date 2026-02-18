#[derive(Default)]
pub enum CullMode {
    #[default]
    None,
    Front,
    Back,
}

#[derive(Default)]
pub enum DepthMode {
    #[default]
    WriteGreater,
    WriteLess,
    NoWrite,
}

pub trait Shader {
    /// Vertex shader input
    type Vertex;

    /// Attributes to be interpolated during rasterization
    type Interpolant;

    /// Fragment shader output
    type Fragment;

    /// Pixel type on the screen buffer
    type Pixel;

    /// Vertex shader stage
    ///
    /// Returns an 'Interpolant' to be rasterized
    /// It is required to assign a value to <position_out> in NDC coordinates
    fn vertex(&self, vertex_in: &Self::Vertex, position_out: &mut glam::Vec4) -> Self::Interpolant;

    /// Fragment shader stage
    ///
    /// Returns an intermediate 'Fragment' without writing to any buffers
    fn fragment(&self, frag_vertex_in: &Self::Interpolant) -> Self::Fragment;

    /// Converts 'Fragment' to buffer's 'Pixel' representation and writes to memeory
    fn pixel(&self, fragment_in: &Self::Fragment) -> Self::Pixel;

    // Optional overload to cull enable triangle culling
    fn cull_mode() -> CullMode {
        Default::default()
    }

    // Optional overload to set depth-testing mode to allow use of different screen-space bases
    fn depth_test() -> DepthMode {
        Default::default()
    }

    // Optional overload to enable/disable writing to depth buffer
    fn depth_write() -> bool {
        true
    }

    // Optional overload to enable/disable writing to pixel buffer
    fn pixel_write() -> bool {
        true
    }
}
