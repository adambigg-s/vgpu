#[cfg(test)]
mod tests {
    use std::hint;

    use crate::{memory::transmute, vgpu::shader};

    struct TestingPipeline;
    impl shader::Shader for TestingPipeline {
        type Vertex = [f32; 6];
        type Interpolant = [f32; 6];
        type Fragment = [f32; 3];
        type Pixel = u32;
        fn vertex(&self, _: &Self::Vertex, _: &mut glam::Vec4) -> Self::Interpolant {
            todo!()
        }
        fn fragment(&self, _: &Self::Interpolant) -> Self::Fragment {
            todo!()
        }
        fn pixel(&self, _: &Self::Fragment) -> Self::Pixel {
            todo!()
        }
    }

    #[test]
    fn ridiculous_transmute() {
        fn generic_pipe_fn<S>(_: S, v: &[f32; 32])
        where
            S: shader::Shader,
        {
            unsafe {
                assert!(size_of_val(v) == 32 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Pixel)) == size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Vertex)) == 6 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Fragment)) == 3 * size_of::<f32>());
                assert!(size_of_val(&*(v.as_ptr() as *const S::Interpolant)) == 6 * size_of::<f32>());
            }
        }

        let shader = TestingPipeline;
        generic_pipe_fn(shader, &[Default::default(); 32]);
    }

    #[test]
    #[unsafe(no_mangle)]
    fn controlled_ub() {
        fn generic_pipe_fn<S>(_: S, val: &[f32; 8])
        where
            S: shader::Shader,
        {
            let shader_in = transmute::bit_interp::<&[f32; 8], &S::Vertex>(&val);
            let shader_out = transmute::bit_interp::<&S::Vertex, &S::Fragment>(&shader_in);
            let good_val = transmute::bit_interp::<&S::Fragment, &[f32; 3]>(&shader_out);
            let bad_val = transmute::bit_interp::<&S::Fragment, &[f32; 8]>(&shader_out);
            hint::black_box(&shader_in);
            hint::black_box(&shader_out);
            hint::black_box(&good_val);
            hint::black_box(&bad_val);
            assert!(size_of_val(good_val) == 3 * size_of::<f32>());
            assert!(size_of_val(bad_val) == 8 * size_of::<f32>());
            assert!(good_val == &val[0..3]);
            assert!(bad_val[3..] == val[3..]);
        }
        let shader = TestingPipeline;
        generic_pipe_fn(shader, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    #[rustfmt::skip]
    #[allow(unused)]
    fn bitwise_transmute() {
        #[repr(C, packed)]
        struct FooBar {
            _v3: glam::Vec3,
            _v2: glam::Vec2,
        }
        let foobar = FooBar {
            _v3: glam::vec3(1.0, 2.0, 3.0),
            _v2: glam::vec2(4.0, 5.0),
        };
        assert!(unsafe { transmute::bit_interp::<FooBar, [f32; 5]>(&foobar) } == [1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(unsafe { transmute::bit_interp::<FooBar, [f32; 4]>(&foobar) } == [1.0, 2.0, 3.0, 4.0]);
        assert!(unsafe { transmute::bit_interp::<FooBar, [f32; 3]>(&foobar) } == [1.0, 2.0, 3.0]);
        assert!(unsafe { transmute::bit_interp::<FooBar, [f32; 2]>(&foobar) } == [1.0, 2.0]);
        assert!(unsafe { transmute::bit_interp::<FooBar, [f32; 1]>(&foobar) } == [1.0]);
    }

    #[test]
    fn ub_transmute() {
        assert!(transmute::bit_interp::<i32, bool>(&1));
        assert!(!transmute::bit_interp::<i32, bool>(&2));
    }
}
