//! Authorized, per-run inference with local weights and allocation-free frame ABI.
use crate::ml_unicorn::Model;
use std::sync::{atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering}, Arc};
use wasmtime::{Config, Engine, Instance, Memory, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};
use zeroize::Zeroizing;

enum Authorization { Session(String), Channel(Arc<AtomicU64>) }
impl Authorization {
    fn check(&self)->Result<(),String> {
        match self {
            Self::Session(hash)=>crate::passport::check_algorithm_session(hash),
            Self::Channel(deadline)=>if crate::mosaic_runtime::tick()<deadline.load(Ordering::Acquire) {Ok(())} else {Err("Preview session ended. Log in and reopen Preview.".into())},
        }
    }
}
pub(crate) struct InferenceRuntime {
    store: Store<StoreLimits>,
    _memory: Memory,
    step: TypedFunc<(f64,f64,f64),i32>,
    result: TypedFunc<i32,f64>,
    reset: TypedFunc<(),i32>,
    authorization: Authorization,
    failure: Option<String>,
}
impl std::fmt::Debug for InferenceRuntime {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {f.write_str("InferenceRuntime")}
}
impl InferenceRuntime {
    pub(crate) fn prepare(model:&Model,width:i32,fps:u32,progress:&Arc<AtomicU32>,cancel:&Arc<AtomicBool>)->Result<Self,String> {
        model.validate()?;
        let (bytes,hash)=crate::algorithm_runtime::receive_algorithm("verux.apb.inference",1024*1024,progress,cancel)?;
        progress.store(65,Ordering::Release);
        if cancel.load(Ordering::Acquire) {return Err("Preparation cancelled.".into());}
        let runtime=Self::compile(&bytes,model,width,fps,Authorization::Session(hash))?;
        if cancel.load(Ordering::Acquire) {return Err("Preparation cancelled.".into());}
        runtime.authorization.check()?;
        progress.store(100,Ordering::Release);
        Ok(runtime)
    }
    pub(crate) fn receive(model:&Model,width:i32,fps:u32)->Result<Self,String> {
        let (bytes,deadline)=crate::mosaic_runtime::receive_channel(1024*1024)?;
        let runtime=Self::compile(&bytes,model,width,fps,Authorization::Channel(deadline))?;
        crate::mosaic_runtime::acknowledge()?;
        Ok(runtime)
    }
    fn compile(bytes:&[u8],model:&Model,width:i32,fps:u32,authorization:Authorization)->Result<Self,String> {
        model.validate()?;
        if width<=0 || fps==0 || fps>1000 {return Err("ML Unicorn requires a valid monitor width and FPS between 1 and 1000.".into());}
        authorization.check()?;
        let quill=Zeroizing::new(serde_json::to_vec(model).map_err(|_|"Cannot encode local model.")?);
        if quill.len()>32*1024*1024 {return Err("ML model exceeds 32 MiB. Select a smaller model.".into());}
        let mut config=Config::new(); config.consume_fuel(true);
        let engine=Engine::new(&config).map_err(|_|"Cannot initialize inference runtime.")?;
        let module=wasmtime::Module::new(&engine,bytes).map_err(|_|"Inference compilation failed. Contact support.")?;
        if module.imports().next().is_some() {return Err("Inference contains forbidden imports.".into());}
        let limits=StoreLimitsBuilder::new().memory_size(128*1024*1024).instances(1).memories(1).tables(1).table_elements(4096).build();
        let mut store=Store::new(&engine,limits); store.limiter(|s|s);
        store.set_fuel(200_000_000).map_err(|_|"Cannot set inference limits.")?;
        let instance=Instance::new(&mut store,&module,&[]).map_err(|_|"Inference initialization failed.")?;
        let memory=instance.get_memory(&mut store,"memory").ok_or("Inference memory is unavailable.")?;
        let allocate=instance.get_typed_func::<u32,u32>(&mut store,"prepare_input").map_err(|_|"Inference ABI is incompatible.")?;
        let initialize=instance.get_typed_func::<(i32,u32),i32>(&mut store,"initialize").map_err(|_|"Inference ABI is incompatible.")?;
        let step=instance.get_typed_func(&mut store,"step").map_err(|_|"Inference ABI is incompatible.")?;
        let result=instance.get_typed_func(&mut store,"result").map_err(|_|"Inference ABI is incompatible.")?;
        let reset=instance.get_typed_func(&mut store,"reset").map_err(|_|"Inference ABI is incompatible.")?;
        let pointer=allocate.call(&mut store,quill.len() as u32).map_err(|_|"Inference allocation exceeded its limits.")?;
        if pointer==0 {return Err("Inference rejected the model size.".into());}
        memory.write(&mut store,pointer as usize,&quill).map_err(|_|"Inference model memory is invalid.")?;
        if initialize.call(&mut store,(width,fps)).map_err(|_|"Inference model initialization exceeded its limits.")?!=0 {return Err("Inference rejected the model. Record and train again.".into());}
        authorization.check()?;
        Ok(Self {store,_memory:memory,step,result,reset,authorization,failure:None})
    }
    pub(crate) fn check_authorization(&self)->Result<(),String> {self.authorization.check()}
    pub(crate) fn reset(&mut self) {
        let result=(||->Result<(),String> {
            self.authorization.check()?;
            self.store.set_fuel(100_000_000).map_err(|_|"Cannot set inference limits.")?;
            if self.reset.call(&mut self.store,()).map_err(|_|"Inference reset failed. Restart.")?!=0 {return Err("Inference reset rejected. Restart.".into());}
            self.authorization.check()
        })();
        if let Err(error)=result {self.failure=Some(error);}
    }
    pub(crate) fn step(&mut self,error:[f64;2],now:f64)->Result<[f64;2],String> {
        if let Some(error)=&self.failure {return Err(error.clone());}
        let result=(||->Result<[f64;2],String> {
            self.authorization.check()?;
            if !now.is_finite() || error.iter().any(|v|!v.is_finite()) {return Err("ML Unicorn received invalid coordinates or time.".into());}
            self.store.set_fuel(100_000_000).map_err(|_|"Cannot set inference limits.")?;
            if self.step.call(&mut self.store,(error[0],error[1],now)).map_err(|_|"Inference execution exceeded its limits. Restart and check the log.")?!=0 {return Err("ML Unicorn movement timing or model output is invalid. Restart and check capture/input timing.".into());}
            let x=self.result.call(&mut self.store,0).map_err(|_|"Inference result is unavailable.")?;
            let y=self.result.call(&mut self.store,1).map_err(|_|"Inference result is unavailable.")?;
            self.authorization.check()?;
            if [x,y].iter().any(|v|!v.is_finite() || v.abs()>i32::MAX as f64) {return Err("Inference produced invalid movement. Contact support.".into());}
            Ok([x,y])
        })();
        if let Err(error)=&result {self.failure=Some(error.clone());}
        result
    }
}
impl Drop for InferenceRuntime {
    fn drop(&mut self) {crate::log_info!("Per-run inference runtime released.");}
}
