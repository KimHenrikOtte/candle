use std::sync::Arc;


use crate::{wgpu::{cache::BufferReference, device::Pipelines}, WgpuDevice};

use super::{create_bind_group_input1, enqueue_workgroups, get_meta};

pub fn queue_upsample1d(
    dev: &WgpuDevice,
    buffer_dest: Arc<BufferReference>,
    buffer_input1: Arc<BufferReference>,
    layout: &crate::Layout,
    dtype: crate::DType,
    target_size: usize,
) -> crate::Result<()> {
    let (b, c, l) = layout.shape().dims3()?;

    let strides = layout.stride();

    let mut meta = get_meta(&dev);

    meta.add(target_size);
    meta.add(b);
    meta.add(c);
    meta.add(l);
    meta.add(layout.start_offset());

    meta.add(strides[0]);
    meta.add(strides[1]);
    meta.add(strides[2]);
    
    meta.add(c * target_size);
    meta.add(target_size);

    let pipeline = dev.get_pipeline(super::Shader::Upsample(dtype), Pipelines::Upsample1d)?;

    let bind_group = create_bind_group_input1(
        buffer_dest,
        buffer_input1,
    );
    enqueue_workgroups(
        meta,
        pipeline,
        bind_group,
        (target_size as u32 + 63) / 63,
        c as u32,
        b as u32,
        (target_size * b * c) as usize,
        #[cfg(feature = "wgpu_debug")] 
        crate::wgpu::device::QueueDebugInfo::new(&format!("upsample1d, dtype:{:?}", dtype)),
    );
    return Ok(());
}


pub fn queue_upsample2d(
    dev: &WgpuDevice,
    buffer_dest: Arc<BufferReference>,
    buffer_input1: Arc<BufferReference>,
    layout: &crate::Layout,
    dtype: crate::DType,
    target_size: (usize, usize),
) -> crate::Result<()> {
    let (b, c, h, w) = layout.shape().dims4()?;

    let strides = layout.stride();

    let mut meta = get_meta(&dev);

    meta.add(target_size.0);
    meta.add(target_size.1);
    meta.add(b);
    meta.add(c);
    meta.add(h);
    meta.add(w);
    meta.add(layout.start_offset());

    meta.add(strides[0]);
    meta.add(strides[1]);
    meta.add(strides[2]);
    meta.add(strides[3]);
    
    meta.add(c * target_size.0 * target_size.1);
    meta.add(target_size.0 * target_size.1);
    meta.add(target_size.1);

    let pipeline = dev.get_pipeline(super::Shader::Upsample(dtype), Pipelines::Upsample2d)?;

    let bind_group = create_bind_group_input1(
        buffer_dest,
        buffer_input1,
    );
    enqueue_workgroups(
        meta,
        pipeline,
        bind_group,
        (target_size.1 as u32 + 7) / 8,
        (target_size.0 as u32 + 7) / 8,
        c as u32,
        (b * c * target_size.0 * target_size.1) as usize,
        #[cfg(feature = "wgpu_debug")] 
        crate::wgpu::device::QueueDebugInfo::new(&format!("upsample2d, dtype:{:?}", dtype)),
    );
    return Ok(());
}
