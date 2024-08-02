use super::*;
mod sgemm{
    use candle_wgpu_kernels::matmul::Functions;

    use crate::{Layout, Shape};
    use super::*;

    pub fn queue_matmul_buffer1(
        dev: &WgpuDevice,
        buffer_dest: BufferReferenceId,
        buffer_input1: BufferReferenceId,
        buffer_input2: BufferReferenceId,
        b: u32,
        m: u32,
        n: u32,
        k: u32,
        layout_input1: &Layout,
        layout_input2: &Layout,
        dtype: crate::DType,
    ) -> crate::Result<()> {
        let mut input1_stride = layout_input1.stride().iter().rev();
        let mut input2_stride = layout_input2.stride().iter().rev();
    
        let mut meta = get_meta(&dev);
        meta.add(b);
        meta.add(m);
        meta.add(k);
        meta.add(n);
    
        meta.add(*input1_stride.next().unwrap_or(&1)); //input1_stride_k
        meta.add(*input1_stride.next().unwrap_or(&1)); //input1_stride_m
        meta.add(*input1_stride.next().unwrap_or(&1)); //input1_stride_b
        meta.add(layout_input1.start_offset()); //input1_offset
    
        meta.add(*input2_stride.next().unwrap_or(&1)); //input2_stride_n
        meta.add(*input2_stride.next().unwrap_or(&1)); //input2_stride_k
        meta.add(*input2_stride.next().unwrap_or(&1)); //input2_stride_b
        meta.add(layout_input2.start_offset()); //input2_offset
    
        let pipeline = meta.get_pipeline(Pipelines::Matmul(get_dtype(dtype)?, Functions::Matmul1));
      
        let bind_group = create_bind_group_input2(
            buffer_dest,
            buffer_input1,
            buffer_input2,
        );
        enqueue_workgroups(
            meta,
            pipeline,
            bind_group,
            (n  + 15) / 16,
            (m  + 15) / 16,
            b,
            (k * m * n) as usize,
        );
        return Ok(());
    }
    
    //shader1b
    pub fn queue_matmul_buffer1b(
        dev: &WgpuDevice,
        buffer_dest: BufferReferenceId,
        buffer_input1: BufferReferenceId,
        buffer_input2: BufferReferenceId,
        b: u32,
        m: u32,
        n: u32,
        k: u32,
        layout_input1: &Layout,
        layout_input2: &Layout,
        dtype: crate::DType,
    ) -> crate::Result<()> {
        let mut input1_stride = layout_input1.stride().iter().rev();
        let mut input2_stride = layout_input2.stride().iter().rev();
    
        let mut meta = get_meta(&dev);
        meta.add(b);
        meta.add(m);
        meta.add(k);
        meta.add(n);
    
        meta.add(*input1_stride.next().unwrap_or(&1)); //input1_stride_k
        meta.add(*input1_stride.next().unwrap_or(&1)); //input1_stride_m
        meta.add(*input1_stride.next().unwrap_or(&1)); //input1_stride_b
        meta.add(layout_input1.start_offset()); //input1_offset
    
        meta.add(*input2_stride.next().unwrap_or(&1)); //input2_stride_n
        meta.add(*input2_stride.next().unwrap_or(&1)); //input2_stride_k
        meta.add(*input2_stride.next().unwrap_or(&1)); //input2_stride_b
        meta.add(layout_input2.start_offset()); //input2_offset
    
        let pipeline = meta.get_pipeline(Pipelines::Matmul(get_dtype(dtype)?, Functions::Matmul1End));
      
        let bind_group = create_bind_group_input2(
            buffer_dest,
            buffer_input1,
            buffer_input2,
        );
        enqueue_workgroups(
            meta,
            pipeline,
            bind_group,
            ((n + 15) / 16  + 15) / 16,
            (m  + 15) / 16,
            b,
            (k * m * n) as usize
        );
        return Ok(());
    }
    
    
    fn round_to_next_divisible(num: u32, n: u32) -> u32 {
        if n == 0 {
            panic!("Divisor cannot be zero");
        }
        (num + n - 1) / n * n
    }    
    
    //shader7
    pub fn queue_matmul7_buffer(
        dev: &WgpuDevice,
        buffer_dest: BufferReferenceId,
        buffer_input1: BufferReferenceId,
        buffer_input2: BufferReferenceId,
        b: u32,
        m: u32,
        n: u32,
        k: u32,
        layout_input1: &Layout,
        layout_input2: &Layout,
        dtype: crate::DType,
    ) -> crate::Result<()> {
        let mut input1_stride = layout_input1.stride().iter().rev();
        let mut input2_stride = layout_input2.stride().iter().rev();
    
        let input1_stride_k = *input1_stride.next().unwrap_or(&1);
        let input1_stride_m = *input1_stride.next().unwrap_or(&1);
        let input1_stride_b = *input1_stride.next().unwrap_or(&1);
    
        let input2_stride_n = *input2_stride.next().unwrap_or(&1);
        let input2_stride_k = *input2_stride.next().unwrap_or(&1);
        let input2_stride_b = *input2_stride.next().unwrap_or(&1);
    
        let const_vec = vec![input1_stride_k, input1_stride_m, input2_stride_n, input2_stride_k];
    
        let mut meta = get_meta(&dev);
        meta.add(b);
        meta.add(m);
        meta.add(k);
        meta.add(n);
    
        meta.add(input1_stride_b); //input1_stride_b
        meta.add(layout_input1.start_offset()); //input1_offset
    
        meta.add(input2_stride_b); //input2_stride_b
        meta.add(layout_input2.start_offset()); //input2_offset
    
        let pipeline = meta.get_pipeline_const(Pipelines::Matmul(get_dtype(dtype)?, Functions::Matmul7), const_vec.clone());
    
        let bind_group = create_bind_group_input2(
            buffer_dest.clone(),
            buffer_input1.clone(),
            buffer_input2.clone(),
        );
        enqueue_workgroups(
            meta,
            pipeline,
            bind_group,
            (n + 15) / 16,
            (m + 15) / 16,
            b,
            k as usize * m as usize * n as usize,
        );
        return Ok(());
    }
    
    pub fn queue_matmul_generic( 
        dev: &WgpuDevice,
        buffer_dest: BufferReferenceId,
        buffer_input1: BufferReferenceId,
        buffer_input2: BufferReferenceId,
        b: u32,
        m: u32,
        n: u32,
        k: u32,
        layout_input1: &Layout,
        layout_input2: &Layout,
        dtype: crate::DType,
    
        m_tile : u32,
        n_tile : u32,
        k_tile : u32,
        get_pipeline : impl Fn(candle_wgpu_kernels::DType) -> Pipelines
    
    ) -> crate::Result<()> {
        let new_m = round_to_next_divisible(m, m_tile);
        let new_n = round_to_next_divisible(n, n_tile);
        let new_k = round_to_next_divisible(k, k_tile);
        //let new_k = k;
    
        let mut input1_stride = layout_input1.stride().iter().rev();
        let mut input2_stride = layout_input2.stride().iter().rev();
    
        let input1_stride_k = *input1_stride.next().unwrap_or(&1);
    
        let _ = *input2_stride.next().unwrap_or(&1);
        let input2_stride_k = *input2_stride.next().unwrap_or(&1);
    
        let need_different_output_buffer = new_m != m || new_n != n;
    
        let (buffer_input1_padded, layout_input1_padded) = 
            if m % m_tile == 0 && k % k_tile == 0 && input1_stride_k == 1{
                (buffer_input1, layout_input1.clone())
            }
            else{
                let buffer_input1_padded = BufferReference::new(&dev, b * (new_m * new_k) * 4);   
                let dest_layout = crate::Layout::contiguous(&Shape::from((b as usize, new_m as usize, new_k as usize)));
                super::queue_copy3d_padded(dev, buffer_input1_padded.clone(), buffer_input1, dtype, layout_input1, (b, m, k), &dest_layout)?;
                (buffer_input1_padded, dest_layout)
            };
    
        let (buffer_input2_padded, layout_input2_padded) = 
            if n % n_tile == 0 && k % k_tile == 0 && input2_stride_k == 1{
                (buffer_input2, layout_input2.clone())
            }
            else{
                let buffer_input2_padded = BufferReference::new(&dev, b * (new_k * new_n) * 4);   
                let dest_layout = crate::Layout::new(Shape::from((b as usize, new_k as usize, new_n as usize)), vec![(new_n * new_k) as usize, 1, new_k as usize], 0);
                super::queue_copy3d_padded(dev, buffer_input2_padded.clone(), buffer_input2, dtype, layout_input2, (b, k, n),&dest_layout)?;
                (buffer_input2_padded, dest_layout)
            };
        
        let buffer_dest_padded = 
            if need_different_output_buffer{
                let buffer_dest = BufferReference::new(&dev, b * (new_m * new_n) * 4);   
                buffer_dest
            }
            else{
                buffer_dest.clone()
            };
    
    
        let mut input1_stride = layout_input1_padded.stride().iter().rev();
        let mut input2_stride = layout_input2_padded.stride().iter().rev();
    
        let input1_stride_k = *input1_stride.next().unwrap_or(&1);
        let input1_stride_m = *input1_stride.next().unwrap_or(&1);
        let input1_stride_b = *input1_stride.next().unwrap_or(&1);
    
        let input2_stride_n = *input2_stride.next().unwrap_or(&1);
        let input2_stride_k = *input2_stride.next().unwrap_or(&1);
        let input2_stride_b = *input2_stride.next().unwrap_or(&1);
    
        let use_batch = b != 1;
        let const_vec = vec![input1_stride_k, input1_stride_m, input2_stride_n, input2_stride_k, use_batch as usize];
    
        let mut meta = get_meta(&dev);
        meta.add(b);
        meta.add(new_m);
        meta.add(new_k);
        meta.add(new_n);
    
        meta.add(input1_stride_b); //input1_stride_b
        meta.add(layout_input1_padded.start_offset()); //input1_offset
    
        meta.add(input2_stride_b); //input2_stride_b
        meta.add(layout_input2_padded.start_offset()); //input2_offset
    
        let pipeline = meta.get_pipeline_const(get_pipeline(get_dtype(dtype)?), const_vec.clone());
       
        let bind_group = create_bind_group_input2(
            buffer_dest_padded.clone(),
            buffer_input1_padded.clone(),
            buffer_input2_padded.clone(),
        );
        enqueue_workgroups_extra(
            meta,
            pipeline,
            bind_group,
            (new_n ) / n_tile,
            (new_m ) / m_tile,
            b,
            k as usize * m as usize * n as usize,
            #[cfg(feature="wgpu_debug")]
            Some(if use_batch {"Use Batch".to_owned()} else {"No Batch".to_owned()})
        );
    
        if need_different_output_buffer{
            let dest_padding_layout = crate::Layout::contiguous(&Shape::from((b as usize, new_m as usize, new_n as usize)));
            let dest_layout = crate::Layout::contiguous(&Shape::from((b as usize, m as usize, n as usize)));
            super::queue_copy3d(dev, buffer_dest, buffer_dest_padded, dtype, &dest_padding_layout, (b, m, n), &dest_layout)?;
        }
    
        return Ok(());
    }
    
    
    //shader5
    pub fn queue_matmul5_buffer(
        dev: &WgpuDevice,
        buffer_dest: BufferReferenceId,
        buffer_input1: BufferReferenceId,
        buffer_input2: BufferReferenceId,
        b: u32,
        m: u32,
        n: u32,
        k: u32,
        layout_input1: &Layout,
        layout_input2: &Layout,
        dtype: crate::DType,
    ) -> crate::Result<()> {
        queue_matmul_generic(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype, 16, 16,16, |dtype| Pipelines::Matmul(dtype, candle_wgpu_kernels::matmul::Functions::Matmul5))
    }
}

//shader5
pub fn queue_matmul_buffer(
    dev: &WgpuDevice,
    buffer_dest: BufferReferenceId,
    buffer_input1: BufferReferenceId,
    buffer_input2: BufferReferenceId,
    b: u32,
    m: u32,
    n: u32,
    k: u32,
    layout_input1: &Layout,
    layout_input2: &Layout,
    dtype: crate::DType,
) -> crate::Result<()> {
    let alg = dev.matmul_alg.lock().unwrap();
    match alg.clone(){
        crate::wgpu_backend::device::MatmulAlgorithm::Matmul7 => sgemm::queue_matmul7_buffer(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype),
        crate::wgpu_backend::device::MatmulAlgorithm::Matmul1 => sgemm::queue_matmul_buffer1(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype),
        crate::wgpu_backend::device::MatmulAlgorithm::Matmul5 => sgemm::queue_matmul5_buffer(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype),
        crate::wgpu_backend::device::MatmulAlgorithm::Matmul5_32_32 => sgemm::queue_matmul_generic(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype, 32, 32,16, |dtype| Pipelines::Matmul2(dtype, candle_wgpu_kernels::matmul2::Functions::Matmul5)),
        crate::wgpu_backend::device::MatmulAlgorithm::Matuml5_64_64 => sgemm::queue_matmul_generic(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype, 64, 64,16, |dtype| Pipelines::Matmul3(dtype, candle_wgpu_kernels::matmul3::Functions::Matmul5)),
        crate::wgpu_backend::device::MatmulAlgorithm::Matmul5_128_128 => sgemm::queue_matmul_generic(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype, 128, 128,16, |dtype| Pipelines::Matmul4(dtype, candle_wgpu_kernels::matmul4::Functions::Matmul5)),
        crate::wgpu_backend::MatmulAlgorithm::Matmul5_64_64_8_8 => sgemm::queue_matmul_generic(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype, 64, 64,16, |dtype| Pipelines::Matmul5(dtype, candle_wgpu_kernels::matmul5::Functions::Matmul5)),
        crate::wgpu_backend::MatmulAlgorithm::Matmul5_16_64 => sgemm::queue_matmul_generic(dev, buffer_dest, buffer_input1, buffer_input2, b, m, n, k, layout_input1, layout_input2, dtype, 16, 64,16, |dtype| Pipelines::Matmul6(dtype, candle_wgpu_kernels::matmul6::Functions::Matmul5)),
    }
}
