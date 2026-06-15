mod cpu;
mod ifs_histogram;
use std::sync::{Arc,Mutex};
use std::{any, thread};
use anyhow;
use crate::{AResult, Complex, IfsModule};
pub use self::cpu::*;
pub use self::ifs_histogram::*;


pub type RgbaF32 = image::Rgba<f32>;


pub trait IfsEvaluator:Sync + Send {
	type ExtraParams: Send;
	
	fn set_ifs(&mut self, module: IfsModule) -> AResult<()>;
    fn eval(&mut self, params: EvaluationParams, extra_params: Self::ExtraParams);
	fn get_histogram(&self) -> &ifs_histogram::IfsHistogram;
}

pub struct EvaluationParams {
    
    pub weights: Vec<f32>,
    pub variables: Vec<(String, Complex)>,

    pub translate: Complex,
    pub rotate_scale: Complex,
	
    pub num_iters: u64,
}



pub struct EvaluatorThreadHandler<T: IfsEvaluator>{
    evaluator:Arc<Mutex<T>>,
    thread_handle: Option<thread::JoinHandle<()>>

}

impl<T: IfsEvaluator + 'static> EvaluatorThreadHandler<T>{

    pub fn new(evaluator:T) -> Self{
        EvaluatorThreadHandler { evaluator: Arc::new(Mutex::new(evaluator)), thread_handle: None }
    }
    pub fn try_evaluate_async(&mut self, params:EvaluationParams, extra_params:<T as IfsEvaluator>::ExtraParams) -> AResult<()>  {
        match self.thread_handle{
            Some(_)=>Err(anyhow::anyhow!("task in progress")),
            None => {
            let evaluator_ref = self.evaluator.clone();
            self.thread_handle = Some(thread::spawn(move || evaluator_ref.lock().expect("poisoned").eval(params, extra_params)));
            Ok(())
            }
        }
    }
    pub fn check_eval(&self) -> bool{
        match &self.thread_handle{
            Some(handle) => handle.is_finished(),
            None => false
        }
    }

    pub fn try_get_evaluator(&mut self) -> Option<Arc<Mutex<T>>>{
        match self.thread_handle.take_if(|h|h.is_finished()){
            Some(handle) =>{
                let _result  = handle.join(); 
                Some(self.evaluator.clone())
            }
               
            None => Some(self.evaluator.clone())
        }
    }
}
