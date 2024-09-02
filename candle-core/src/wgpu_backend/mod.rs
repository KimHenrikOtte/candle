mod device;
mod storage;

pub mod cache;
pub mod error;
pub mod util;
pub mod wgpu_functions;

#[cfg(feature = "wgpu_debug")]
pub mod debug_info;

pub use device::MatmulAlgorithm;
pub use device::WgpuDevice;
pub use storage::WgpuStorage;

pub use storage::create_wgpu_storage;
pub use storage::create_wgpu_storage_init;

#[cfg(feature = "wgpu_debug_serialize")]
pub use device::DebugPipelineRecording;

#[cfg(feature = "wgpu_debug")]
pub use debug_info::MInfo;
#[cfg(feature = "wgpu_debug")]
pub use debug_info::Measurements;
