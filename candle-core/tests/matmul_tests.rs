use candle_core::{test_device, DType, Device, IndexOp, Result, Tensor};

fn matmul(device: &Device) -> Result<()> {
    let data = vec![1.0f32, 2.0, 3.0, 4.0];
    let a = Tensor::from_slice(&data, (2, 2), device)?;
    let data = vec![1.0f32, 2.0, 3.0, 4.0];
    let b = Tensor::from_slice(&data, (2, 2), device)?;

    let c = a.matmul(&b)?;
    assert_eq!(c.to_vec2::<f32>()?, &[[7.0f32, 10.0], [15.0, 22.0]]);

    let data = vec![1.0f32, 2.0];
    let a = Tensor::from_slice(&data, (2, 1), device)?;
    let data = vec![3.0f32, 4.0];
    let b = Tensor::from_slice(&data, (1, 2), device)?;
    let c = a.matmul(&b)?;
    assert_eq!(c.to_vec2::<f32>()?, &[&[3.0, 4.0], &[6.0, 8.0]]);

    let data: Vec<_> = (0..6).map(|i| i as f32).collect();
    let a = Tensor::from_slice(&data, (2, 3), device)?;
    let data: Vec<_> = (0..6).map(|i| (i + 2) as f32).collect();
    let b = Tensor::from_slice(&data, (3, 2), device)?;
    let c = a.matmul(&b)?;
    assert_eq!(c.to_vec2::<f32>()?, &[&[16., 19.], &[52., 64.]]);

    let data: Vec<_> = (0..12).map(|i| i as f32).collect();
    let a = Tensor::from_slice(&data, (2, 2, 3), device)?;
    let data: Vec<_> = (0..12).map(|i| (i + 2) as f32).collect();
    let b = Tensor::from_slice(&data, (2, 3, 2), device)?;
    let expected = [[[16., 19.], [52., 64.]], [[214., 235.], [304., 334.]]];

    let c = a.matmul(&b)?;
    assert_eq!(c.to_vec3::<f32>()?, &expected);

    // Also perform the matmul on contiguous transposed versions.
    let a_tt = a.t()?.contiguous()?.t()?;
    assert!(!a_tt.is_contiguous());
    assert_eq!(a.dims(), a_tt.dims());
    assert_eq!(a_tt.stride(), &[6, 1, 2]);

    let b_tt = b.t()?.contiguous()?.t()?;
    assert!(!b_tt.is_contiguous());
    assert_eq!(b.dims(), b_tt.dims());
    assert_eq!(b_tt.stride(), &[6, 1, 3]);

    assert_eq!(a_tt.matmul(&b)?.to_vec3::<f32>()?, &expected);
    assert_eq!(a.matmul(&b_tt)?.to_vec3::<f32>()?, &expected);
    assert_eq!(a_tt.matmul(&b_tt)?.to_vec3::<f32>()?, &expected);
    Ok(())
}

fn matmul_bf16(device: &Device) -> Result<()> {
    if !device.supports_bf16() {
        return Ok(());
    }
    let data = vec![1.0f32, 2.0, 3.0, 4.0];
    let a = Tensor::from_slice(&data, (2, 2), device)?.to_dtype(DType::BF16)?;
    let data = vec![1.0f32, 2.0, 3.0, 4.0];
    let b = Tensor::from_slice(&data, (2, 2), device)?.to_dtype(DType::BF16)?;

    let c = a.matmul(&b)?.to_dtype(DType::F32)?;
    assert_eq!(c.to_vec2::<f32>()?, &[[7.0f32, 10.0], [15.0, 22.0]]);
    Ok(())
}

fn broadcast_matmul(device: &Device) -> Result<()> {
    let lhs = Tensor::randn(0f32, 1f32, (3, 1, 4, 5), device)?;
    let rhs = Tensor::randn(0f32, 1f32, (6, 5, 2), device)?;
    let out = lhs.broadcast_matmul(&rhs)?;
    assert_eq!(out.dims(), &[3, 6, 4, 2]);
    for idx1 in 0..3 {
        for idx2 in 0..6 {
            let out = out.i((idx1, idx2))?;
            let lhs = lhs.i((idx1, 0))?;
            let rhs = rhs.i(idx2)?;
            let out2 = lhs.matmul(&rhs);
            let sum_diff2 = (out - out2)?.sqr()?.sum_all()?;
            // With cuda, we see errors of up to ~1e-12.
            assert!(sum_diff2.to_vec0::<f32>()? < 1e-6)
        }
    }
    Ok(())
}

// https://github.com/huggingface/candle/issues/1948
fn squeeze_mm(device: &Device) -> Result<()> {
    let seq_len = 8_usize;
    let a = Tensor::zeros((1, seq_len, 16), DType::F32, device)?;
    let x = a.i((.., seq_len - 1, ..))?;
    let w = Tensor::zeros((32, 16), DType::F32, device)?.t()?;
    let x = x.matmul(&w)?;
    assert_eq!(x.dims(), &[1, 32]);
    Ok(())
}

// https://github.com/huggingface/candle/issues/1992
fn mm_layout(device: &Device) -> Result<()> {
    let a = Tensor::arange(0f32, 16f32, device)?.reshape((1, 1, 4, 4))?;
    let b = Tensor::arange(0f32, 8f32, device)?.reshape((1, 1, 4, 2))?;
    let mm1 = a.matmul(&b)?;
    // Forces the layout to be:
    // shape: [1, 1, 4, 2], stride: [8, 2, 2, 1], start_offset: 0
    // This is still a contiguous matrix but matmul checks are only the two last dimensions have
    // non 1 sizes but matmul check may be reluctant to handle it.
    let b = b.transpose(1, 2)?.force_contiguous()?.transpose(1, 2)?;
    let mm2 = a.matmul(&b)?;
    let diff = (mm1 - mm2)?.abs()?.sum_all()?.to_vec0::<f32>()?;
    assert_eq!(diff, 0.);
    Ok(())
}

test_device!(matmul, matmul_cpu, matmul_gpu, matmul_metal, matmul_wgpu);
test_device!(
    matmul_bf16,
    matmul_bf16_cpu,
    matmul_bf16_gpu,
    matmul_bf16_metal,
    matmul_bf16_wgpu
);
test_device!(
    broadcast_matmul,
    broadcast_matmul_cpu,
    broadcast_matmul_gpu,
    broadcast_matmul_metal,
    broadcast_matmul_wgpu
);
test_device!(squeeze_mm, squeeze_mm_cpu, squeeze_mm_gpu, squeeze_mm_metal,squeeze_mm_wgpu);
test_device!(mm_layout, mm_layout_cpu, mm_layout_gpu, mm_layout_metal, mm_layout_wgpu);


#[cfg(feature="wgpu")]
#[test]
//test different wgpu matmul shaders, compares results with cpu impl
fn test_matmul_kernels_wgpu()-> Result<()> {
    use candle_core::wgpu::MatmulAlgorithm;
    
    let algs = vec![
        MatmulAlgorithm::Matmul32_64,
        MatmulAlgorithm::Matmul32_64B,
        MatmulAlgorithm::Matmul1_64B,
        MatmulAlgorithm::Matmul7,
        MatmulAlgorithm::Matmul1,
        MatmulAlgorithm::MatmulX,
        MatmulAlgorithm::Matmul16_16,
        MatmulAlgorithm::Matmul32_32,
        MatmulAlgorithm::Matmul64_64,
        MatmulAlgorithm::Matmul64_64_8_8,
        MatmulAlgorithm::Matmul24_24,
        MatmulAlgorithm::Matmul24_48,
        MatmulAlgorithm::Matmul24_24B,
        MatmulAlgorithm::Matmul24_48B
    ];

    let device = Device::new_wgpu_sync(0)?;

    if let Device::Wgpu(wgpu) = &device{
        for alg in algs{
            (*wgpu.matmul_alg.lock().unwrap()) = alg.clone();

            for tpa in [true, false]{
                for tpb in [true, false]{
                    for use_start_offset in [true, false]{
                        for tpb_batch in [true, false]{
                            for tpa_batch in [true, false]{
                                big_matmul_wgpu(&device, tpa, tpb, use_start_offset, tpb_batch, tpa_batch)?;
                            }
                        }
                    }
                }
            }

            matmul(&device)?;
            broadcast_matmul(&device)?;
            squeeze_mm(&device)?;
            mm_layout(&device)?;
        }
    }

    Ok(())
}


//compares wgpu matmul impl, with cpu impl
#[cfg(feature="wgpu")]
fn big_matmul_wgpu(device: &Device, tpa : bool, tpb : bool, use_start_offset : bool, tpb_batch : bool, tpa_batch : bool)-> Result<()> {
    use candle_core::D;
    let b = 1;
    let m = 63;
    let n = 63;
    let k = 63;

    let start_offset = if use_start_offset {100} else {0};
    let lhs1 = Tensor::rand(0f32, 100f32, b * k * m + start_offset, &Device::Cpu)?.to_dtype(DType::U32)?.to_dtype(DType::F32)?.i(start_offset..)?;
    let rhs1 = Tensor::rand(0f32, 100f32, b * k * n + start_offset, &Device::Cpu)?.to_dtype(DType::U32)?.to_dtype(DType::F32)?.i(start_offset..)?;

    let lhs;
    if tpa_batch{
        if tpa{
            lhs = lhs1.reshape((m,k,b))?.transpose(D::Minus1, D::Minus2)?.transpose(0, 1)?;
        }
        else{
            lhs = lhs1.reshape((k,m,b))?.transpose(0, 2)?;
        }
    }
    else if tpa{
        lhs = lhs1.reshape((b,k,m))?.transpose(D::Minus1, D::Minus2)?;
    }
    else{
        lhs = lhs1.reshape((b,m,k))?;
    }
    

    let rhs;
    if tpb_batch {
        if tpb{
            rhs = rhs1.reshape((k,n,b))?.transpose(D::Minus1, D::Minus2)?.transpose(0, 1)?;
        }
        else{
            rhs = rhs1.reshape((n,k,b))?.transpose(0, 2)?;
        }
    }
    else if tpb{
        rhs = rhs1.reshape((b,n,k))?.transpose(D::Minus1, D::Minus2)?;
    }
    else{
        rhs = rhs1.reshape((b,k,n))?;
    }
   
    

    let t1 = lhs.matmul(&rhs)?.reshape((b,m,n))?;


    let lhs = lhs.to_device(device)?;
    let rhs = rhs.to_device(device)?;

    let t2 = lhs.matmul(&rhs)?.reshape((b,m,n))?;

    let m =  candle_core::test_utils::to_vec3_round(&t1, 3)?;
    let m2 = candle_core::test_utils::to_vec3_round(&t2, 3)?;

    assert_eq!(m, m2);
    Ok(())
}