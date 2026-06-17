use std::sync::atomic::{AtomicU64, Ordering};
use ifs_lang::{ compiler::TextCompiler};

use rand::{Rng, distr::Distribution, rngs::ThreadRng};
use rand_distr::weighted::WeightedAliasIndex;
use crate::{Complex, evaluator::{IfsHistogram, ifs_histogram}};
use formulac::{self, UserDefinedTable};

use crate::{AResult, IfsModule};


pub struct FormulacParams {
	pub num_threads: usize,
}

pub struct FormulacEvaluator {
	ifs_histogram:ifs_histogram::IfsHistogram,
    atomic_histogram: Box<[AtomicU64]>,
	formulac_state: FormulacState,
}

#[derive(Default)]
struct FormulacState {
	ifs_functions: Vec<String>,
	user_functions: formulac::UserDefinedTable
}

impl super::IfsEvaluator for FormulacEvaluator {
	type ExtraParams = FormulacParams;
	

	fn set_ifs(&mut self, module: IfsModule) -> AResult<()> {
		let compiled = ifs_lang::compile::<TextCompiler<crate::FormulacFormatter>>(&module)?;
		self.formulac_state.ifs_functions = compiled.functions;
      
	    Ok(())
    }
	
	fn eval(&mut self, params: super::EvaluationParams, extra_params: Self::ExtraParams){
        let mut closures:Vec<Box<dyn Fn(&[Complex]) -> Complex + Sync>> = Vec::new();

		let variables: Vec<_> = params.variables.iter().map(|&(ref name, val)| (name.as_str(), val)).collect();
        let variables = formulac::Variables::from(&variables);
    
        for function in self.formulac_state.ifs_functions.iter(){
                closures.push(Box::new( formulac::compile(function.as_str(), &["z"], &variables, &self.formulac_state.user_functions).unwrap()))
        }
		
        let len = params.weights.len();

        
    	let weighted_rng = match WeightedAliasIndex::new(params.weights){
			Ok(weighted_rng) => weighted_rng,
			Err(_) => WeightedAliasIndex::new(vec![1.0;len]).unwrap()
   		};
    	let ifs:Box<dyn ComplexIfsFunction> = Box::new(move |z:&Complex, rng| closures[weighted_rng.sample(rng)]([*z].as_slice()));
        
        self.iterate_ifs(ifs, params.rotate_scale, params.translate, params.num_iters, extra_params.num_threads);
        
        //inlining this function solves the borrow checkers problems, but I don't wanna

	}
	
	fn get_histogram(&self) -> &ifs_histogram::IfsHistogram {
		&self.ifs_histogram
	}
}

impl FormulacEvaluator {

    pub fn new(frame:[usize;2], user_funcs:UserDefinedTable) -> Self{
        let atomic_histogram = (0 .. (frame[0] * frame[1]) as u64)
                .into_iter()
                .map(|_| AtomicU64::new(0))
                .collect();

        FormulacEvaluator { 
            ifs_histogram: super::IfsHistogram::new(frame), 
            atomic_histogram,
            formulac_state: FormulacState { ifs_functions: Default::default(), user_functions: user_funcs } }
    }
	fn reset(&mut self) {
        for i in 0 .. self.ifs_histogram.frame[0] * self.ifs_histogram.frame[1] {
            self.atomic_histogram[i].store(0, Ordering::Relaxed);
            self.ifs_histogram.histogram[i] = 0;
            self.ifs_histogram.max = 0;
        }
    }
	fn complex_to_2d_index(&self, z: &Complex) -> Option<(usize, usize)> {
        let x = self.ifs_histogram.frame[0];
        let y = self.ifs_histogram.frame[1];
        
        let index: Complex = ((*z + Complex::new(1.0, 1.0)) / 2.0) * y as f64;
        if (0.0..x as f64).contains(&index.re) && (0.0..y as f64).contains(&index.im){
            return Some((index.re as usize, index.im as usize));
        } 
        else{
            //println!("{}", z);
            return None
            
        }
    }
    fn index2d_to_index1d(&self, index: (usize, usize)) -> usize {
        return index.0 as usize + (index.1 as usize * self.ifs_histogram.frame[0]);
    }

	fn build_ifs(&self, weights: Vec<f32>, variables: Vec<(String, Complex)>) -> Box<dyn '_ + ComplexIfsFunction>{
		let mut closures:Vec<Box<dyn Fn(&[Complex]) -> Complex + Sync>> = Vec::new();

		let variables: Vec<_> = variables.iter().map(|&(ref name, val)| (name.as_str(), val)).collect();
        let variables = formulac::Variables::from(&variables);
    
        for function in self.formulac_state.ifs_functions.iter(){
                closures.push(Box::new( formulac::compile(function.as_str(), &["z"], &variables, &self.formulac_state.user_functions).unwrap()))
        }
		
        let len = weights.len();
    	let weighted_rng = match WeightedAliasIndex::new(weights){
			Ok(weighted_rng) => weighted_rng,
			Err(_) => WeightedAliasIndex::new(vec![1.0;len]).unwrap()
   		};
    	return Box::new(move |z, rng| closures[weighted_rng.sample(rng)]([*z].as_slice()));
	}

	fn iterate_ifs(
        &mut self,
		ifs: Box<dyn ComplexIfsFunction>,
        rotate_scale: Complex,
        translate: Complex,
        num_iters: u64,
        num_threads: usize,
    ) {
        let iters_per_thread = (num_iters / num_threads as u64) as usize;
        let global_max: AtomicU64 = 0.into();
        self.reset();

        rayon::scope(|scope| {
            for _ in 0 .. num_threads {
                scope.spawn(|_| {
                    let mut rng = rand::rng();

                    let mut z = Complex::new(0.0, 0.0);
                    let mut local_max = 0;
                    for _ in 0 .. iters_per_thread {
                        z = ifs(&z, &mut rng);
                        if z.is_nan() {
                            z = Complex::new(rng.random(), rng.random());
                            continue;
                        }

                        match self.complex_to_2d_index(&(rotate_scale * z + translate)) {
                            None => (),
                            Some(index) => {
                                let count = self.atomic_histogram[self.index2d_to_index1d(index)]
                                    .fetch_add(1, Ordering::SeqCst) +
                                    1;
                                if count > local_max {
                                    local_max = count;
                                }
                            },
                        }
                    }
                    global_max.fetch_max(local_max, Ordering::SeqCst);
                });
            }
        });
        self.ifs_histogram.max = global_max.load(Ordering::SeqCst);
        
        for i in 0 .. self.ifs_histogram.frame[0]*self.ifs_histogram.frame[1]{
            self.ifs_histogram.histogram[i] = self.atomic_histogram[i].load(Ordering::SeqCst);
            
        
        }
    }
}

pub trait ComplexIfsFunction: (Fn(&Complex, &mut ThreadRng) -> Complex) + Sync {}

impl<Func: Fn(&Complex, &mut ThreadRng) -> Complex + Sync> ComplexIfsFunction for Func {}