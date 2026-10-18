//! Per-run, memory-only numerical module. No disk cache or host imports.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hpke::{
    aead::AesGcm256, kdf::HkdfSha256, kem::DhP256HkdfSha256, Deserializable, Kem, OpModeR,
    Serializable,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use wasmtime::{
    Config, Engine, Instance, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};
use zeroize::Zeroizing;

#[derive(Deserialize)]
struct Delivery {
    valid: bool,
    manifest: String,
    encapsulated_key: String,
    ciphertext: String,
}
#[derive(Deserialize)]
struct Manifest {
    product_id: String,
    algorithm_id: String,
    artifact_version: String,
    abi_version: u32,
    digest: String,
    size: usize,
    session_hash: String,
    recipient_key: String,
    request_nonce: String,
    expires_at: f64,
}

pub struct AlgorithmRuntime {
    pub(crate) inference: Option<crate::ml_unicorn::Controller>,
    axis: TypedFunc<(i32, f64, f64, f64), f64>,
    store: Store<StoreLimits>,
}

impl std::fmt::Debug for AlgorithmRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AlgorithmRuntime")
    }
}

impl AlgorithmRuntime {
    pub(crate) fn prepare_for_config(config:&crate::config::ProfileConfig,progress:&Arc<AtomicU32>,cancelled:&Arc<std::sync::atomic::AtomicBool>)->Result<Self,String> {
        if !config.ml_unicorn_enabled {return Self::prepare(progress,cancelled);}
        if !config.ml_unicorn_influence.is_finite() || !(0.0..=100.0).contains(&config.ml_unicorn_influence) {return Err("ML Unicorn influence must be between 0 and 100.".into());}
        let model=crate::ml_unicorn::Store::open()?.load_model(&config.ml_unicorn_model)?;
        let stage=Arc::new(AtomicU32::new(0));
        let mut runtime=Self::prepare(&stage,cancelled)?;
        progress.store(50,Ordering::Release);
        let controller=crate::ml_unicorn::Controller::prepare(model,config.monitor_resolution.x,config.fps,&stage,cancelled)?;
        controller.check_source(if config.coordinate_source==crate::config::CoordinateSource::Memory {"memory"} else {"color"})?;
        runtime.inference=Some(controller);
        progress.store(100,Ordering::Release);
        Ok(runtime)
    }

    pub fn prepare(
        progress: &Arc<AtomicU32>,
        cancelled: &Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self, String> {
        let (bytes, session_hash) = receive_algorithm(&crate::protocol_names::numerical_id(), 128 * 1024, progress, cancelled)?;
        let check_cancel = || {
            if cancelled.load(Ordering::Acquire) { Err("Preparation cancelled.".to_string()) } else { Ok(()) }
        };
        progress.store(65, Ordering::Release);
        check_cancel()?;
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine =
            Engine::new(&config).map_err(|_| "Unable to initialize preparation runtime.")?;
        let module = Module::new(&engine, bytes.as_slice())
            .map_err(|_| "Preparation compilation failed. Contact support.")?;
        if module.imports().next().is_some() {
            return Err("Preparation contains forbidden host imports.".into());
        }
        progress.store(85, Ordering::Release);
        check_cancel()?;
        let limits = StoreLimitsBuilder::new()
            .memory_size(4 * 1024 * 1024)
            .instances(1)
            .memories(1)
            .tables(1)
            .table_elements(1024)
            .build();
        let mut store = Store::new(&engine, limits);
        store.limiter(|limits| limits);
        store
            .set_fuel(100_000)
            .map_err(|_| "Unable to initialize execution limits.")?;
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|_| "Preparation initialization failed.")?;
        let axis = instance
            .get_typed_func::<(i32, f64, f64, f64), f64>(&mut store, "axis")
            .map_err(|_| "Preparation ABI is incompatible.")?;
        check_cancel()?;
        crate::passport::check_algorithm_session(&session_hash)?;
        Ok(Self { axis, store, inference: None })
    }

    pub fn beesmas(
        &mut self,
        kind: Option<crate::config::BeesmasType>,
        x: i32,
        y: i32,
        beesmas_value: f32,
        width: i32,
        height: i32,
    ) -> Result<(f64, f64), String> {
        use crate::config::BeesmasType;
        if !beesmas_value.is_finite() || beesmas_value <= 0.0 || width <= 0 || height <= 0 {
            return Err("BeesmasValue settings are invalid. Correct the profile and restart.".into());
        }
        let kind = match kind {
            None => 0,
            Some(BeesmasType::Linear) => 1,
            Some(BeesmasType::Logarithmic) => 2,
            Some(BeesmasType::Hyperbolic) => 3,
            Some(BeesmasType::Bezier) => 4,
        };
        self.store
            .set_fuel(100_000)
            .map_err(|_| "Execution limit could not be set.")?;
        let dx = self
            .axis
            .call(
                &mut self.store,
                (kind, f64::from(x), f64::from(beesmas_value), f64::from(width)),
            )
            .map_err(|_| "Algorithm execution stopped. Restart and check the log.")?;
        let dy = self
            .axis
            .call(
                &mut self.store,
                (kind, f64::from(y), f64::from(beesmas_value), f64::from(height)),
            )
            .map_err(|_| "Algorithm execution stopped. Restart and check the log.")?;
        if !dx.is_finite()
            || !dy.is_finite()
            || dx.abs() > i32::MAX as f64
            || dy.abs() > i32::MAX as f64
        {
            return Err("Algorithm produced an invalid result. Contact support.".into());
        }
        Ok((dx, dy))
    }
}

impl Drop for AlgorithmRuntime {
    fn drop(&mut self) {
        crate::log_info!("Per-run algorithm runtime released.");
    }
}

/// Common authenticated delivery; execution interfaces remain algorithm-specific.
pub(crate) fn receive_algorithm(
    algorithm_id: &str,
    maximum_size: usize,
    progress: &Arc<AtomicU32>,
    cancelled: &Arc<std::sync::atomic::AtomicBool>,
) -> Result<(Zeroizing<Vec<u8>>, String), String> {
        let check_cancel = || {
            if cancelled.load(Ordering::Acquire) {
                Err("Preparation cancelled.".to_string())
            } else {
                Ok(())
            }
        };
        let (token, session_hash) = crate::passport::algorithm_session()?;
        let token = Zeroizing::new(token);
        progress.store(10, Ordering::Release);
        check_cancel()?;
        let mut seed = Zeroizing::new([0u8; 32]);
        crate::passport::generate_random_bytes(seed.as_mut())?;
        let (private, public) = DhP256HkdfSha256::derive_keypair(seed.as_ref());
        let recipient = URL_SAFE_NO_PAD.encode(public.to_bytes());
        let (result, nonce) = crate::passport::algorithm_request(serde_json::json!({
            "product_id": crate::passport::PRODUCT_ID, "session_token": token.as_str(),
            "algorithm_id": algorithm_id, "abi_version": 1, "recipient_key": recipient,
        }))?;
        progress.store(45, Ordering::Release);
        check_cancel()?;
        let delivery: Delivery = serde_json::from_value(result)
            .map_err(|_| "Preparation response is invalid. Contact support.")?;
        if !delivery.valid {
            return Err("Preparation was refused. Log in again or contact support.".into());
        }
        let manifest: Manifest = serde_json::from_str(&delivery.manifest)
            .map_err(|_| "Preparation manifest is invalid.")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Check the system clock.")?
            .as_secs_f64();
        if manifest.product_id != crate::passport::PRODUCT_ID
            || manifest.algorithm_id != algorithm_id
            || manifest.artifact_version != "2.0.0"
            || manifest.abi_version != 1
            || manifest.size == 0 || manifest.size > maximum_size
            || manifest.session_hash != session_hash
            || manifest.recipient_key != recipient
            || manifest.request_nonce != nonce
            || !manifest.expires_at.is_finite()
            || manifest.expires_at <= now
            || manifest.expires_at > now + 3600.0
        {
            return Err(
                "Preparation does not match this session. Restart and log in again.".into(),
            );
        }
        let enc = URL_SAFE_NO_PAD
            .decode(delivery.encapsulated_key)
            .map_err(|_| "Preparation encryption header is invalid.")?;
        let enc = <DhP256HkdfSha256 as Kem>::EncappedKey::from_bytes(&enc)
            .map_err(|_| "Preparation encryption header is invalid.")?;
        let encrypted = URL_SAFE_NO_PAD
            .decode(delivery.ciphertext)
            .map_err(|_| "Preparation payload is invalid.")?;
        let mut receiver = hpke::setup_receiver::<AesGcm256, HkdfSha256, DhP256HkdfSha256>(
            &OpModeR::Base,
            &private,
            &enc,
            b"verux-algorithm-v2",
        )
        .map_err(|_| "Preparation decryption failed.")?;
        let bytes = Zeroizing::new(
            receiver
                .open(&encrypted, delivery.manifest.as_bytes())
                .map_err(|_| "Preparation decryption failed.")?,
        );
        if bytes.len() != manifest.size
            || format!("{:x}", Sha256::digest(&bytes)) != manifest.digest
        {
            return Err("Preparation integrity check failed. Contact support.".into());
        }
        crate::passport::check_algorithm_session(&session_hash)?;
        check_cancel()?;
        Ok((bytes, session_hash))
}
