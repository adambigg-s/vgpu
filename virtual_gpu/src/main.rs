mod memory;
mod vgpu;

fn main() {
    let gpu = vgpu::gpu::Vgpu::new(4, 4);
}
