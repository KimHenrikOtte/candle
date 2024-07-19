pub mod binary;
pub mod cmp;
pub mod conv2d;
pub mod convert;
pub mod copy;
pub mod gather;
pub mod index_select;
pub mod matmul;
pub mod pool2d;
pub mod reduce;
pub mod rms_norm;
pub mod softmax;
pub mod unary;
pub mod upsample;
pub mod where_cond;

use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    num::NonZeroU64,
};

use super::{
    cache::{BindGroupReferenceBase, BufferReference, CachedBindGroupReference},
    device::{
        BindGroupReference, DispatchedBindgroup, MlQueue, PipelineType, Pipelines, QueueBuffer, META_BUFFER_SIZE,
    },
    util::ToU32,
};
use crate::DType;
use crate::{wgpu_backend::device::WgpuDevice, Layout, WebGpuError};
use std::{
    borrow::Cow,
    sync::{Arc, MutexGuard},
};
use wgpu::{Device, Queue, ShaderModule};

pub use binary::queue_binary_buffer_from_buffer;
pub use cmp::queue_cmp_buffer_from_buffer;
pub use conv2d::{queue_conv1d, queue_conv1d_transpose, queue_conv2d, queue_conv2d_transpose};
pub use convert::{
    queue_convert_f32_to_u32, queue_convert_f32_to_u8, queue_convert_u32_to_f32,
    queue_convert_u32_to_u8, queue_convert_u8_to_f32,
};
pub use copy::{queue_copy, queue_copy2d, queue_copy3d, queue_copy_strided};
pub use gather::{queue_gather, queue_index_add_inplace, queue_scatter_add_inplace};
pub use index_select::queue_index_select;
pub use matmul::queue_matmul_buffer;
pub use pool2d::{queue_avg_pool2d, queue_max_pool2d};
pub use reduce::queue_reduce_from_buffer_op;
pub use rms_norm::queue_rms_norm;
pub use softmax::queue_softmax;
pub use unary::{queue_unary_from_buffer_op, queue_unary_inplace_op};
pub use upsample::{queue_upsample1d, queue_upsample2d};
pub use where_cond::queue_where_cond_u32;

pub const MAX_DISPATCH_SIZE: u32 = 65535;

///Helper Type MetaArray, for constructing the MetaBuffer

#[derive(Debug)]
pub(crate) struct MetaArray(Vec<u32>);

impl MetaArray {
    pub(crate) fn new(capacity: u32) -> Self {
        MetaArray(Vec::with_capacity(capacity as usize))
    }

    pub(crate) fn add_layout(&mut self, layout: &Layout) {
        let shape = layout.shape().dims();
        let stride = layout.stride();
        self.0.push(shape.len() as u32);
        self.0.push(layout.start_offset() as u32);

        if layout.is_contiguous() {
            self.0.push(layout.shape().elem_count() as u32);
        } else {
            self.0.push(0);
        }

        self.0.extend(shape.iter().map(|&x| x as u32));
        self.0.extend(stride.iter().map(|&x| x as u32));
    }

    pub(crate) fn add<T: ToU32>(&mut self, value: T) {
        self.0.push(value.to_u32());
    }
}

// fn get_size(layout: &Layout) -> u32 {
//     return 3 + layout.dims().len() as u32 * 2;
// }

///All known Shader Files
#[derive(Debug, Hash, std::cmp::Eq, std::cmp::PartialEq, Clone)]
pub enum Shader {
    Binary(DType),
    Cmp(DType),
    Conv2D(DType),
    Convert(DType),
    Copy(DType),
    IndexSelect(DType),
    Matmul(DType),
    Reduce(DType),
    RmsNorm(DType),
    Softmax(DType),
    Unary(DType),
    WhereCond(DType),
    Pool2d(DType),
    Upsample(DType),
    Gather(DType),
}

pub fn load_shader(shader: Shader) -> crate::Result<&'static str> {
    match shader {
        Shader::Binary(DType::F32) => Ok(include_str!(
            "binary/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Binary(DType::U32) => Ok(include_str!(
            "binary/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Cmp(DType::F32) => Ok(include_str!(
            "cmp/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Cmp(DType::U32) => Ok(include_str!(
            "cmp/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Conv2D(DType::F32) => Ok(include_str!(
            "conv2d/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Conv2D(DType::U32) => Ok(include_str!(
            "conv2d/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Convert(DType::F32) => Ok(include_str!(
            "convert/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Convert(DType::U32) => Ok(include_str!(
            "convert/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Convert(DType::U8) => Ok(include_str!(
            "convert/generated/shader.pwgsl_generated_u8.wgsl"
        )),
        Shader::Copy(DType::F32) => Ok(include_str!(
            "copy/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Copy(DType::U32) => Ok(include_str!(
            "copy/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::IndexSelect(DType::F32) => Ok(include_str!(
            "index_select/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::IndexSelect(DType::U32) => Ok(include_str!(
            "index_select/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Matmul(DType::F32) => Ok(include_str!(
            "matmul/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Matmul(DType::U32) => Ok(include_str!(
            "matmul/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Reduce(DType::F32) => Ok(include_str!(
            "reduce/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Reduce(DType::U32) => Ok(include_str!(
            "reduce/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::RmsNorm(DType::F32) => Ok(include_str!(
            "rms_norm/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::RmsNorm(DType::U32) => Ok(include_str!(
            "rms_norm/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Softmax(DType::F32) => Ok(include_str!(
            "softmax/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Softmax(DType::U32) => Ok(include_str!(
            "softmax/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Unary(DType::F32) => Ok(include_str!(
            "unary/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Unary(DType::U32) => Ok(include_str!(
            "unary/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::WhereCond(DType::F32) => Ok(include_str!(
            "where_cond/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::WhereCond(DType::U32) => Ok(include_str!(
            "where_cond/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Pool2d(DType::F32) => Ok(include_str!(
            "pool2d/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Pool2d(DType::U32) => Ok(include_str!(
            "pool2d/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Upsample(DType::F32) => Ok(include_str!(
            "upsample/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Upsample(DType::U32) => Ok(include_str!(
            "upsample/generated/shader.pwgsl_generated_u32.wgsl"
        )),
        Shader::Gather(DType::F32) => Ok(include_str!(
            "gather/generated/shader.pwgsl_generated_f32.wgsl"
        )),
        Shader::Gather(DType::U32) => Ok(include_str!(
            "gather/generated/shader.pwgsl_generated_u32.wgsl"
        )),

        _ => Err(crate::Error::WebGpu(WebGpuError::Message(format!(
            "Could not find Pipeline: {:?}",
            shader
        )))),
    }
}

const WORKGROUP_SIZE: u32 = 64;

pub fn get_shader(device: &wgpu::Device, shader: &'static str) -> ShaderModule {
    let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader)),
    });
    return cs_module;
}

pub fn create_buffer_init<T: bytemuck::Pod>(dev: &WgpuDevice, data: &[T]) -> Arc<BufferReference> {
    return BufferReference::new_init(dev, bytemuck::cast_slice(data));
}

fn enqueue_workgroups(
    mut command_queue: MutexGuard<QueueBuffer>,
    pipeline: PipelineType,
    bind_group: BindGroupReference,
    x: u32,
    y: u32,
    z: u32,
    workload_size : usize,
    #[cfg(feature = "wgpu_debug")] _debug: super::device::QueueDebugInfo,
) {
    if y > MAX_DISPATCH_SIZE || z > MAX_DISPATCH_SIZE  || x > MAX_DISPATCH_SIZE {
        panic!("can not queue y or z higher than 65535 x:{x}, y:{y}, z:{z}, pipeline: {:?}", pipeline);
    }
    let q = MlQueue::Dispatch(super::device::MlQueueDispatch {
        x,
        y,
        z,
        pipeline: pipeline,
        bindgroup: DispatchedBindgroup::BindgroupReference(bind_group),
        meta: command_queue.current_meta,
        workload_size,
        #[cfg(feature = "wgpu_debug")]
        debug: _debug,
    });
    command_queue.command_queue.push(q);
}

fn next_divisible_by_n(value: i32, n: i32) -> i32 {
    if n == 0 {
        panic!("n must be a non-zero integer");
    }

    if value % n == 0 {
        value
    } else {
        value + (n - value % n)
    }
}

//size: size you want to add
fn get_meta(dev: &WgpuDevice) -> MutexGuard<QueueBuffer> {
    let mut command_queue = dev.command_queue.lock().unwrap();
    let meta_array_length = command_queue.meta_array.0.len() as i32;
    let meta_offset = next_divisible_by_n(
        meta_array_length,
        dev.device_limits.min_storage_buffer_offset_alignment as i32 / 4,
    );
    command_queue.current_meta = meta_offset as u32;
    command_queue
        .meta_array
        .0
        .extend(std::iter::repeat(0).take((meta_offset - meta_array_length) as usize));

    return command_queue;
}


#[cfg(feature = "wgpu_debug")]
fn init_debug_queue(dev: &WgpuDevice, length: u32) -> (u32, wgpu::QuerySet) {
    let global_index = dev.debug.counter.load(std::sync::atomic::Ordering::Relaxed);
    let query_set = dev.device.create_query_set(&wgpu::QuerySetDescriptor {
        count: length as u32 * 2, // We need 2 queries: one for start and one for end
        ty: wgpu::QueryType::Timestamp,
        label: None,
    });
    return (global_index, query_set);
}

#[cfg(feature = "wgpu_debug")]
fn end_debug_queue(
    dev: &WgpuDevice,
    length: u32,
    global_index: u32,
    encoder: &mut wgpu::CommandEncoder,
    query_set: &wgpu::QuerySet,
) {
    if global_index % 256 != 0 {
        panic!("global_index was:{global_index}")
    }
    encoder.resolve_query_set(
        &query_set,
        0..length,
        &dev.debug.query_set_buffer,
        global_index as u64,
    );
    let global_index = global_index + (length * 8) as u32;

    let remainder = global_index % 256;
    let global_index = if remainder == 0 {
        global_index
    } else {
        global_index + (256 - remainder)
    };
    dev.debug
        .counter
        .store(global_index, std::sync::atomic::Ordering::Relaxed);
}

fn get_command_buffer(
    dev: &WgpuDevice,
    meta_array: &[u32],
    command_queue: &[MlQueue],
    pipelines: &[Arc<wgpu::ComputePipeline>],
    current_meta: usize,
) -> wgpu::CommandBuffer {
    #[cfg(feature = "wgpu_debug")]
    let (global_index, query_set) = init_debug_queue(dev, command_queue.len() as u32);

    #[cfg(feature = "wgpu_debug")]
    let mut debug_index = 0;

    let data = bytemuck::cast_slice(&meta_array);
    if data.len() as u32 + 256 > META_BUFFER_SIZE {
        panic!("Meta Buffer was to big, length was: {}", data.len());
    }

    //write Meta Buffer
    dev.queue.write_buffer(&dev.meta_buffer, 0, data);
    let mut encoder = dev
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        for (q, pipeline) in command_queue.iter().zip(pipelines) {
            match q {
                MlQueue::Dispatch(q) => {
                    if let DispatchedBindgroup::CachedBindgroup(bindgroup) = &q.bindgroup {
                        let qx = q.x;
                        let qy = q.y;
                        let qz = q.z;
                        let meta = q.meta - current_meta as u32;

                        //let (pipline, bindgroup, qx, qy, qz, qindirect_buffer, meta) = data;
                        #[cfg(feature = "wgpu_debug")]
                        cpass.write_timestamp(&query_set, debug_index);

                        cpass.set_pipeline(&pipeline);

                        if meta * 4 >= META_BUFFER_SIZE - 256 {
                            panic!(
                                "meta is to big!: meta was {meta}, q.meta: {}/{current_meta}",
                                q.meta
                            );
                        }

                        cpass.set_bind_group(0, &bindgroup.bindgroup, &[meta * 4]);
                        cpass.dispatch_workgroups(qx, qy, qz);
                        
                        #[cfg(feature = "wgpu_debug")]
                        {
                            cpass.write_timestamp(&query_set, debug_index + 1);
                            dev.debug.insert_info(
                                global_index + debug_index * 8,
                                (
                                    q.debug.name.to_owned(),
                                    q.workload_size as u64,
                                    q.x,
                                    q.y,
                                    q.z,
                                ),
                            );
                            debug_index += 2;
                        }
                    }
                }
            }
        }
    }

    #[cfg(feature = "wgpu_debug")]
    end_debug_queue(
        dev,
        command_queue.len() as u32 * 2,
        global_index,
        &mut encoder,
        &query_set,
    );

    //dev.device.poll(wgpu::Maintain::wait()).panic_on_timeout(); //wait for last submission
    
    return encoder.finish();
}

fn prepare(dev: &WgpuDevice, queue_buffer: &mut QueueBuffer){
    let mut most_needed_storage;
    let mut total_used_storage;
    let queue = &mut queue_buffer.command_queue;
    {
        let mut hasher = DefaultHasher::new();
        for q in queue.iter() {
            match q {
                MlQueue::Dispatch(q) => {
                    q.pipeline.hash(&mut hasher);
                }
            }
        }
        let current_hash = hasher.finish();
        let mut cache = dev.cache.lock().unwrap();
        cache.mappings.set_current_buffer_mapping(current_hash);

        total_used_storage = cache.buffers.buffer_memory - cache.buffers.buffer_memory_free; //the total amount of memory acutally used
        most_needed_storage = total_used_storage;

        let mut buffers_used_at: HashMap<u64, usize> = HashMap::new();

        for (index, q) in queue.iter().enumerate() {
            let mut check_buffer = |buffer: &Arc<BufferReference>| {
                let key: u64 = Arc::as_ptr(buffer) as u64;
                buffers_used_at.insert(key, index);
            };
            match q {
                MlQueue::Dispatch(q) => match &q.bindgroup {
                    DispatchedBindgroup::BindgroupReference(br) => {
                        match br {
                            BindGroupReferenceBase::Bindgroup0(v0) => {
                                check_buffer(v0);
                            }
                            BindGroupReferenceBase::Bindgroup1(v0, v1) => {
                                check_buffer(v0);
                                check_buffer(v1);
                            }
                            BindGroupReferenceBase::Bindgroup2(v0, v1, v2) => {
                                check_buffer(v0);
                                check_buffer(v1);
                                check_buffer(v2);
                            }
                            BindGroupReferenceBase::Bindgroup3(v0, v1, v2, v3) => {
                                check_buffer(v0);
                                check_buffer(v1);
                                check_buffer(v2);
                                check_buffer(v3);
                            }
                        }
                    }
                    DispatchedBindgroup::CachedBindgroup(_) => todo!(),
                },
            }
        }
        let mut buffer_used = HashSet::new();
        for (index, q) in queue.iter().enumerate() {
            let mut check_buffer = |buffer: &Arc<BufferReference>| {
                let key: u64 = Arc::as_ptr(buffer) as u64;
                let buffer_last_used_index = buffers_used_at.get(&key).unwrap();
                
                if !buffer_used.contains(&key){
                    buffer_used.insert(key);
                    if buffer.storage.lock().unwrap().is_none() {
                        total_used_storage += buffer.size;
                    }
                }

                if *buffer_last_used_index <= index {
                    
                    if !buffer.is_referenced_by_storage.load(std::sync::atomic::Ordering::Relaxed)
                    {
                        if total_used_storage > most_needed_storage {
                            most_needed_storage = total_used_storage;
                        }
                        total_used_storage -= buffer.size;
                    }
                }
            };
            match q {
                MlQueue::Dispatch(q) => match &q.bindgroup {
                    DispatchedBindgroup::BindgroupReference(br) => {
                        match br {
                            BindGroupReferenceBase::Bindgroup0(v0) => {
                                check_buffer(v0);
                            }
                            BindGroupReferenceBase::Bindgroup1(v0, v1) => {
                                check_buffer(v0);
                                check_buffer(v1);
                            }
                            BindGroupReferenceBase::Bindgroup2(v0, v1, v2) => {
                                check_buffer(v0);
                                check_buffer(v1);
                                check_buffer(v2);
                            }
                            BindGroupReferenceBase::Bindgroup3(v0, v1, v2, v3) => {
                                check_buffer(v0);
                                check_buffer(v1);
                                check_buffer(v2);
                                check_buffer(v3);
                            }
                        }
                    }
                    DispatchedBindgroup::CachedBindgroup(_) => todo!(),
                },
            }
        }
        //allow 20% margin more:
        //println!("flush: {}({})/{most_needed_storage}", cache.buffers.buffer_memory, cache.buffers.buffer_memory_free);

        let most_needed_storage = (most_needed_storage as f64 * 1.20)  as u64;
        
        if most_needed_storage >  cache.buffers.max_memory_allowed{
            cache.buffers.max_memory_allowed = most_needed_storage;
        }
        else{
            cache.buffers.max_memory_allowed = ((0.9 *  cache.buffers.max_memory_allowed as f64) + (0.1 * most_needed_storage as f64)) as u64;
        }
    }
}

fn set_buffers(dev: &WgpuDevice, queue: &mut Vec<MlQueue>, index : &mut usize, current_meta: usize, last_meta : &mut usize, wgpu_data : &mut Vec<Arc<wgpu::ComputePipeline>>){
    let mut cache_limit = false;
    let mut total_workload = 0u64; //we only allow a certain amount of workload per commandBuffer 
    let start_index = *index; 
    for q in queue[*index..].iter_mut() {
        #[cfg(feature="wgpu_debug")]{
            let ele_size =  *index-start_index;
            if ele_size >= 4095{
                break;
            }
        }

        *index += 1;
        let mut cache = dev.cache.lock().unwrap();

        match q {
            MlQueue::Dispatch(q) => {

              
                let ele_size =  *index-start_index;
                if (total_workload + q.workload_size as u64)  > super::device::MAX_WORKLOAD_SIZE && ele_size > 1 {
                    *index -= 1;
                    break;
                }
                else{
                    total_workload += q.workload_size as u64;
                }
                #[cfg(feature="wgpu_debug")]
                if q.workload_size as u64 > super::device::MAX_WORKLOAD_SIZE{
                    log::info!("OP: {} workload_size: {}", q.debug.name, q.workload_size);
                }


                let mut optimize_unary_inplace = false;
                let mut v1_ref = None;
                let mut v2_ref = None;
                if q.pipeline.1 == Pipelines::UnaryFromBufferContiguousNoStartOffset {
                    if let DispatchedBindgroup::BindgroupReference(
                        bindgroup_reference,
                    ) = &q.bindgroup
                    {
                        if let BindGroupReferenceBase::Bindgroup1(v1, v2) =
                            bindgroup_reference
                        {
                            if Arc::strong_count(&v2) == 1 {
                                //this Bindgroup is the only one, holding a reference to this BufferReference -> So we can Reuse that Buffer
                                if v1.size <= v2.size {
                                    if v1.storage.lock().unwrap().is_none() {
                                        //startoffset = 0?
                                        q.pipeline.1 =
                                            Pipelines::UnaryInplaceContiguous;
                                        v1_ref = Some(v1.clone());
                                        v2_ref = Some(v2.clone());
                                        q.bindgroup =
                                            DispatchedBindgroup::BindgroupReference(
                                                BindGroupReferenceBase::Bindgroup0(
                                                    v2.clone(),
                                                ),
                                            );
                                        optimize_unary_inplace = true;
                                    }
                                }
                            }
                        }
                    }
                }

                let pl: &wgpu::PipelineLayout = match &q.bindgroup {
                    DispatchedBindgroup::BindgroupReference(bindgroup_reference) => {
                        match bindgroup_reference {
                            BindGroupReferenceBase::Bindgroup0(_) => {
                                &dev.bindgroup_layouts.pipeline_layout0
                            }
                            BindGroupReferenceBase::Bindgroup1(_, _) => {
                                &dev.bindgroup_layouts.pipeline_layout1
                            }
                            BindGroupReferenceBase::Bindgroup2(_, _, _) => {
                                &dev.bindgroup_layouts.pipeline_layout2
                            }
                            BindGroupReferenceBase::Bindgroup3(_, _, _, _) => {
                                &dev.bindgroup_layouts.pipeline_layout3
                            }
                        }
                    }
                    _ => panic!("not expected"),
                };

                let pipeline = dev
                    .get_pipeline2( &q.pipeline, pl)
                    .unwrap();

                if let DispatchedBindgroup::BindgroupReference(bindgroup_reference) =
                    &q.bindgroup
                {
                    let bindgroup = cache.get_bind_group(
                        dev,
                        bindgroup_reference,
                        q.pipeline.clone(),
                    );

                    if cache.remove_unused(){ //we hit the max cache size
                        cache_limit = true;
                    }
        
                    drop(cache);
                    
                    q.bindgroup = DispatchedBindgroup::CachedBindgroup(bindgroup); //this may drop a bufferReference. The BufferReference needs to access cache, therefore cache was droped
                    wgpu_data.push(pipeline);

                    if optimize_unary_inplace {
                        dev.cached_buffer_inplace_counter
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if let Some(v1_ref) = v1_ref {
                            if let Some(v2_ref) = v2_ref {
                                let mut v1_storage = v1_ref.storage.lock().unwrap();
                                let mut v2_storage = v2_ref.storage.lock().unwrap();
                                *v1_storage = v2_storage.as_ref().cloned();
                                *v2_storage = None;
                            }
                        }
                    }
                }

                *last_meta = q.meta as usize;

               
                let meta_size = (*last_meta - current_meta) * 4 + 256 * 3;
                if meta_size > META_BUFFER_SIZE as usize
                {
                    break;
                }
                if cache_limit{
                    break;
                }
                if total_workload > super::device::MAX_WORKLOAD_SIZE{
                    break;
                }
            }
        }
    }
    let meta_size = (*last_meta - current_meta) * 4 + 256 * 3;
    let ele_size =  *index-start_index;
    log::info!("queue {ele_size}, Meta: {meta_size}, workload: {total_workload}, cache_limit: {cache_limit}");
}

pub(crate) fn flush_gpu_command(dev: &WgpuDevice, queue_buffer: &mut QueueBuffer) {
    if queue_buffer.command_queue.len() > 0 {
        prepare(dev, queue_buffer);
        let queue = &mut queue_buffer.command_queue;
        {
            

            let mut wgpu_data = Vec::with_capacity(queue.len());

            let mut start_index = 0;
            let mut index = 0;
            let mut current_meta: usize = 0;
            let mut last_meta: usize = 0;
            while index < queue.len() {
                set_buffers(dev, queue, &mut index, current_meta, &mut last_meta, &mut wgpu_data);

                let last_meta_index = (last_meta + 256 / 4).min(queue_buffer.meta_array.0.len());
              
                let cb = get_command_buffer(
                    dev,
                    &queue_buffer.meta_array.0[current_meta..last_meta_index],
                    &queue[start_index..index],
                    &wgpu_data,
                    current_meta,
                );
                
                #[cfg(not(target_arch = "wasm32"))]
                {
                    dev.device.poll(wgpu::Maintain::wait()).panic_on_timeout();
                    //if !dev.device.poll(wgpu::Maintain::Poll).is_queue_empty(){
                    //pollster::block_on(synchronize_device(&dev.device, &dev.queue)).unwrap();
                    //}
                }

                dev.queue.submit(Some(cb));
                
               
                start_index = index;
                current_meta = last_meta;
                wgpu_data.clear();
            }
        }
        queue_buffer.command_queue.clear();
        queue_buffer.meta_array.0.clear();
        queue_buffer.current_meta = 0;
        {
            let mut cache = dev.cache.lock().unwrap();
            cache.mappings.finish();
            cache.buffers.remove_unused();
            cache.remove_unused();
        }
    }
}

///Flush commands, and wait until last command has been executed
// pub(crate) async fn flush_gpu_command_async(dev: &WgpuDevice, queue_buffer: &mut QueueBuffer) {
//     if queue_buffer.command_queue.len() > 0 {
//         prepare(dev, queue_buffer);
//         let queue = &mut queue_buffer.command_queue;
//         {
            

//             let mut wgpu_data = Vec::with_capacity(queue.len());

//             let mut start_index = 0;
//             let mut index = 0;
//             let mut current_meta: usize = 0;
//             let mut last_meta: usize = 0;
//             while index < queue.len() {
//                 set_buffers(dev, queue, &mut index, current_meta, &mut last_meta, &mut wgpu_data);

//                 let last_meta_index = (last_meta + 256 / 4).min(queue_buffer.meta_array.0.len());
              
//                 let cb = get_command_buffer(
//                     dev,
//                     &queue_buffer.meta_array.0[current_meta..last_meta_index],
//                     &queue[start_index..index],
//                     &wgpu_data,
//                     current_meta,
//                 );
                
//                 dev.queue.submit(Some(cb));
//                 synchronize_device(&dev.device, &dev.queue).await.unwrap();

//                 start_index = index;
//                 current_meta = last_meta;
//                 wgpu_data.clear();
//             }
//         }
//         queue_buffer.command_queue.clear();
//         queue_buffer.meta_array.0.clear();
//         queue_buffer.current_meta = 0;
//         {
//             let mut cache = dev.cache.lock().unwrap();
//             cache.mappings.finish();
//             cache.buffers.remove_unused();
//             cache.remove_unused();
//         }
//     }
// }


fn enqueue(
    command_queue: MutexGuard<QueueBuffer>,
    pipeline: PipelineType,
    bind_group: BindGroupReference,
    length: u32,
    workload_size : usize,
    #[cfg(feature = "wgpu_debug")] _debug: super::device::QueueDebugInfo,
) {
    return enqueue_workgroups(
        command_queue,
        pipeline,
        bind_group,
        (length + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE,
        1,
        1,
        workload_size,
        #[cfg(feature = "wgpu_debug")]
        _debug,
    );
}

fn enqueue_big(
    command_queue: MutexGuard<QueueBuffer>,
    pipeline: PipelineType,
    bind_group: BindGroupReference,
    length: u32,
    #[cfg(feature = "wgpu_debug")] _debug: super::device::QueueDebugInfo,
) {

    let id = (length + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
    let x = id.min(65535);
    let y = (id + 65534) / 65535;

    return enqueue_workgroups(
        command_queue,
        pipeline,
        bind_group,
        x,
        y,
        1,
        length as usize,
        #[cfg(feature = "wgpu_debug")]
        _debug,
    );
}

pub fn create_buffer(dev: &WgpuDevice, size: u64) -> wgpu::Buffer {
    dev.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub fn create_bindgroup(dev: &WgpuDevice, bindgroup: CachedBindGroupReference) -> wgpu::BindGroup {
    dev.cached_bindgroup_counter
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let buffer_meta = &dev.meta_buffer;

    let meta_binding = wgpu::BufferBinding {
        buffer: &buffer_meta,
        offset: 0,
        size: Some(NonZeroU64::new(256).unwrap()),
    };
    let meta_binding = wgpu::BindingResource::Buffer(meta_binding);

    let meta_entry = wgpu::BindGroupEntry {
        binding: 1,
        resource: meta_binding, //buffer_meta.as_entire_binding(),
    };

    let bind_group_layout = match bindgroup {
        BindGroupReferenceBase::Bindgroup0(_) => &dev.bindgroup_layouts.bind_group_layout0,
        BindGroupReferenceBase::Bindgroup1(_, _) => &dev.bindgroup_layouts.bind_group_layout1,
        BindGroupReferenceBase::Bindgroup2(_, _, _) => &dev.bindgroup_layouts.bind_group_layout2,
        BindGroupReferenceBase::Bindgroup3(_, _, _, _) => &dev.bindgroup_layouts.bind_group_layout3,
    };

    match bindgroup {
        CachedBindGroupReference::Bindgroup0(buffer_dest) => {
            let entries = &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_dest.buffer.as_entire_binding(),
                },
                meta_entry,
            ];
            dev.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: entries,
            })
        }
        CachedBindGroupReference::Bindgroup1(buffer_dest, buffer_input1) => {
            let entries = &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_dest.buffer.as_entire_binding(),
                },
                meta_entry,
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_input1.buffer.as_entire_binding(),
                },
            ];
            dev.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: entries,
            })
        }
        CachedBindGroupReference::Bindgroup2(buffer_dest, buffer_input1, buffer_input2) => {
            let entries = &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_dest.buffer.as_entire_binding(),
                },
                meta_entry,
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_input1.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffer_input2.buffer.as_entire_binding(),
                },
            ];
            dev.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: entries,
            })
        }
        CachedBindGroupReference::Bindgroup3(
            buffer_dest,
            buffer_input1,
            buffer_input2,
            buffer_input3,
        ) => {
            let entries = &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_dest.buffer.as_entire_binding(),
                },
                meta_entry,
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_input1.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffer_input2.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: buffer_input3.buffer.as_entire_binding(),
                },
            ];
            dev.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: entries,
            })
        }
    }
}

fn create_bind_group_input0(buffer_dest: Arc<BufferReference>) -> BindGroupReference {
    BindGroupReference::Bindgroup0(buffer_dest)
}

fn create_bind_group_input1(
    buffer_dest: Arc<BufferReference>,
    buffer_input1: Arc<BufferReference>,
) -> BindGroupReference {
    BindGroupReference::Bindgroup1(buffer_dest, buffer_input1)
}

fn create_bind_group_input2(
    buffer_dest: Arc<BufferReference>,
    buffer_input1: Arc<BufferReference>,
    buffer_input2: Arc<BufferReference>,
) -> BindGroupReference {
    BindGroupReference::Bindgroup2(buffer_dest, buffer_input1, buffer_input2)
}

fn create_bind_group_input3(
    buffer_dest: Arc<BufferReference>,
    buffer_input1: Arc<BufferReference>,
    buffer_input2: Arc<BufferReference>,
    buffer_input3: Arc<BufferReference>,
) -> BindGroupReference {
    BindGroupReference::Bindgroup3(buffer_dest, buffer_input1, buffer_input2, buffer_input3)
}

pub fn synchronize(dev: &WgpuDevice) -> crate::Result<()> {
    let mut command_queue = dev.command_queue.lock().unwrap();
    if command_queue.command_queue.len() > 0{
        flush_gpu_command(dev, &mut command_queue);
        return pollster::block_on(synchronize_device(&dev.device, &dev.queue));
    }
    Ok(())
}

// pub async fn synchronize_async(dev: &WgpuDevice) -> crate::Result<()> {
//     let mut command_queue = dev.command_queue.lock().unwrap();
//     if command_queue.command_queue.len() > 0{
//         flush_gpu_command_async(dev, &mut command_queue).await;
//         synchronize_device(&dev.device, &dev.queue).await?;
//     }
//     Ok(())
// }

async fn synchronize_device(dev: &Device, queue: &Queue) -> crate::Result<()> {
    let (sender, receiver) = flume::bounded(1);
    queue.on_submitted_work_done(move || sender.send(()).unwrap());

    dev.poll(wgpu::Maintain::wait()).panic_on_timeout();
    if let Ok(()) = receiver.recv_async().await {
        return Ok(());
    }
    Ok(())
}

pub async fn read_data_from_gpu_async<T: bytemuck::Pod>(
    dev: &WgpuDevice,
    buffer: Arc<BufferReference>,
) -> Vec<T> {
    let mut command_queue = dev.command_queue.lock().unwrap();
    flush_gpu_command(dev, &mut command_queue); //send all previous commands to the gpu
    let dest_size = buffer.size;

    //TODO: use cached staging buffer!
    let staging_buffer = dev.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: dest_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = dev
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    let buffer_storage = buffer.storage.lock().unwrap();
    if let Some(buffer) = buffer_storage.as_ref() {
        encoder.copy_buffer_to_buffer(&buffer.buffer, 0, &staging_buffer, 0, dest_size);
    } else {
        panic!("Unespected error at read_data from gpu. Tensor WgpuStorage did not Point to a wgpu Buffer")
    }

    // Submits command encoder for processing
    dev.queue.submit(Some(encoder.finish()));

    // Note that we're not calling `.await` here.
    let buffer_slice = staging_buffer.slice(..);
    // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
    let (sender, receiver) = flume::bounded(1);
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    // Poll the device in a blocking manner so that our future resolves.
    // In an actual application, `device.poll(...)` should
    // be called in an event loop or on another thread.
    dev.device.poll(wgpu::Maintain::wait()).panic_on_timeout();

    // Awaits until `buffer_future` can be read from
    if let Ok(Ok(())) = receiver.recv_async().await {
        // Gets contents of buffer
        let data = buffer_slice.get_mapped_range();
        // Since contents are got in bytes, this converts these bytes back to u32
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();

        // With the current interface, we have to make sure all mapped views are
        // dropped before we unmap the buffer.
        drop(data);
        staging_buffer.unmap(); // Unmaps buffer from memory
                                // If you are familiar with C++ these 2 lines can be thought of similarly to:
                                //   delete myPointer;
                                //   myPointer = NULL;
                                // It effectively frees the memory

        // Returns data from buffer
        result
    } else {
        panic!("failed to run compute on gpu!")
    }
}



pub async fn read_data_from_gpu_async_buffer<T: bytemuck::Pod>(
    dev: &WgpuDevice,
    buffer: &wgpu::Buffer,
) -> Vec<T> {
    let mut command_queue = dev.command_queue.lock().unwrap();
    flush_gpu_command(dev, &mut command_queue); //send all previous commands to the gpu

    let dest_size = buffer.size();

    //TODO: use cached staging buffer!
    let staging_buffer = dev.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: dest_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = dev
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    encoder.copy_buffer_to_buffer(&buffer, 0, &staging_buffer, 0, dest_size);

    // Submits command encoder for processing
    dev.queue.submit(Some(encoder.finish()));

    // Note that we're not calling `.await` here.
    let buffer_slice = staging_buffer.slice(..);
    // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
    let (sender, receiver) = flume::bounded(1);
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    // Poll the device in a blocking manner so that our future resolves.
    // In an actual application, `device.poll(...)` should
    // be called in an event loop or on another thread.
    dev.device.poll(wgpu::Maintain::wait()).panic_on_timeout();

    // Awaits until `buffer_future` can be read from
    if let Ok(Ok(())) = receiver.recv_async().await {
        // Gets contents of buffer
        let data = buffer_slice.get_mapped_range();
        // Since contents are got in bytes, this converts these bytes back to u32
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();

        // With the current interface, we have to make sure all mapped views are
        // dropped before we unmap the buffer.
        drop(data);
        staging_buffer.unmap(); // Unmaps buffer from memory
                                // If you are familiar with C++ these 2 lines can be thought of similarly to:
                                //   delete myPointer;
                                //   myPointer = NULL;
                                // It effectively frees the memory

        // Returns data from buffer
        result
    } else {
        panic!("failed to run compute on gpu!")
    }
}
