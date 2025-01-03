#![allow(unused_imports, unexpected_cfgs)]
#[cfg(feature = "mkl")]
extern crate intel_mkl_src;
#[cfg(feature = "accelerate")]
extern crate accelerate_src;
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;
#[cfg(not(target_arch = "wasm32"))]
use tokio::test as test;
use candle_wasm_tests::{
    to_vec0_round_async, to_vec1_round_async, to_vec2_round_async, to_vec3_round_async,
};
use candle::{Device, Result, Tensor};
#[test]
async fn kv_cache() -> Result<()> {
    let mut cache = candle_nn::kv_cache::Cache::new(0, 16);
    for _ in [0, 1] {
        assert_eq!(cache.current_seq_len(), 0);
        let data = cache.current_data()?;
        assert!(data.is_none());
        let t = Tensor::new(&[1f32, 2., 3.], &Device::Cpu)?;
        cache.append(&t)?;
        let data = cache.current_data()?.unwrap();
        assert_eq!(data.to_vec1_async::< f32 > (). await ?, [1., 2., 3.]);
        let t = Tensor::new(&[4f32], &Device::Cpu)?;
        cache.append(&t)?;
        let data = cache.current_data()?.unwrap();
        assert_eq!(data.to_vec1_async::< f32 > (). await ?, [1., 2., 3., 4.]);
        let t = Tensor::new(&[0f32, 5., 6., 7.], &Device::Cpu)?;
        cache.append(&t)?;
        let data = cache.current_data()?.unwrap();
        assert_eq!(
            data.to_vec1_async::< f32 > (). await ?, [1., 2., 3., 4., 0., 5., 6., 7.]
        );
        assert_eq!(cache.current_seq_len(), 8);
        cache.reset();
    }
    Ok(())
}
#[test]
async fn rotating_kv_cache() -> Result<()> {
    let mut cache = candle_nn::kv_cache::RotatingCache::new(0, 6);
    for _ in [0, 1] {
        assert_eq!(cache.offset(), 0);
        assert_eq!(cache.current_seq_len(), 0);
        let data = cache.current_data()?;
        assert!(data.is_none());
        let t = Tensor::new(&[1., 2., 3.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(data.to_vec1_async::< f64 > (). await ?, [1., 2., 3.]);
        let t = Tensor::new(&[4.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(data.to_vec1_async::< f64 > (). await ?, [1., 2., 3., 4.]);
        let t = Tensor::new(&[0., 5., 6., 7.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(data.to_vec1_async::< f64 > (). await ?, [6., 7., 3., 4., 0., 5.]);
        assert_eq!(cache.current_seq_len(), 8);
        assert_eq!(cache.offset(), 2);
        let t = Tensor::new(&[8.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(data.to_vec1_async::< f64 > (). await ?, [6., 7., 8., 4., 0., 5.]);
        assert_eq!(cache.current_seq_len(), 9);
        assert_eq!(cache.offset(), 3);
        let t = Tensor::new(&[9., 10., 11.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(data.to_vec1_async::< f64 > (). await ?, [6., 7., 8., 9., 10., 11.]);
        assert_eq!(cache.current_seq_len(), 12);
        assert_eq!(cache.offset(), 0);
        let t = Tensor::new(&[12.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(data.to_vec1_async::< f64 > (). await ?, [12., 7., 8., 9., 10., 11.]);
        assert_eq!(cache.current_seq_len(), 13);
        assert_eq!(cache.offset(), 1);
        let mask = cache.attn_mask(2, &Device::Cpu)?.unwrap();
        assert_eq!(
            mask.to_vec2_async::< u8 > (). await ?, & [[0, 0, 1, 0, 0, 0], [0, 0, 0, 0,
            0, 0]]
        );
        let mask = cache.attn_mask(3, &Device::Cpu)?.unwrap();
        assert_eq!(
            mask.to_vec2_async::< u8 > (). await ?, & [[0, 0, 1, 1, 0, 0], [0, 0, 0, 1,
            0, 0], [0, 0, 0, 0, 0, 0]],
        );
        let t = Tensor::new(&[0., 1., 2., 3., 4., 5., 6., 7., 8.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(
            data.to_vec1_async::< f64 > (). await ?, [0., 1., 2., 3., 4., 5., 6., 7., 8.]
        );
        assert_eq!(cache.current_seq_len(), 22);
        assert_eq!(cache.offset(), 0);
        let mask = cache.attn_mask(1, &Device::Cpu)?;
        assert!(mask.is_none());
        let mask = cache.attn_mask(2, &Device::Cpu)?.unwrap();
        assert_eq!(
            mask.to_vec2_async::< u8 > (). await ?, & [[0, 1, 0, 0, 0, 0], [0, 0, 0, 0,
            0, 0]]
        );
        let mask = cache.attn_mask(3, &Device::Cpu)?.unwrap();
        assert_eq!(
            mask.to_vec2_async::< u8 > (). await ?, & [[0, 1, 1, 0, 0, 0], [0, 0, 1, 0,
            0, 0], [0, 0, 0, 0, 0, 0]]
        );
        let t = Tensor::new(&[42.], &Device::Cpu)?;
        let data = cache.append(&t)?;
        assert_eq!(data.to_vec1_async::< f64 > (). await ?, [42., 4., 5., 6., 7., 8.]);
        assert_eq!(cache.current_seq_len(), 23);
        assert_eq!(cache.offset(), 1);
        cache.reset();
    }
    Ok(())
}