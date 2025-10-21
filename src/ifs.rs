use std::sync::atomic::{AtomicU64, Ordering};

use num_complex::{Complex64};
use std::f64::consts::{PI,TAU};
use rand::prelude::*;
use rand_distr::weighted::WeightedAliasIndex;

use rayon;

use image::{Pixel, Rgba, RgbaImage};

type Complex = num_complex::Complex64;


pub trait ComplexIfsFunction: (Fn(&Complex, &mut ThreadRng) -> Complex) + Sync { 

    
}

impl<Func: Fn(&Complex,&mut ThreadRng) -> Complex + Sync> ComplexIfsFunction for Func {

}

pub struct IfsHistogram{
    frame:(usize,usize),
    max:u64,
    histogram: Box<[AtomicU64]>,

}

impl IfsHistogram{

    pub fn new(frame:(usize,usize)) -> Self{
        IfsHistogram { 
            frame , 
            max: 0, 
            histogram: vec![0u64; frame.0*frame.1].into_iter().map(AtomicU64::new).collect() }
    }

    fn reset(&mut self){
        for i in 0..self.frame.0*self.frame.1{
            self.histogram[i].store(0, Ordering::Relaxed);
        }
    }

    fn complex_to_2d_index(&self,z:&Complex)->Option<(usize,usize)>{
        let index:Complex =((*z + Complex::new(1.0,1.0))/2.0) * self.frame.1 as f64;
        if index.re >=self.frame.0 as f64|| index.re < 0.0 || index.im >= self.frame.1 as f64 || index.im < 0.0{
            return None
        }
        else{return Some((index.re as usize, index.im as usize))}
    }
    
    fn index2d_to_index1d(&self,index:(usize,usize)) -> usize{
        return index.0 as usize + (index.1 as usize * self.frame.0);
    }

    pub fn iterate_ifs(&mut self,ifs:&Box<dyn '_+ ComplexIfsFunction >, transform: fn(&Complex) -> Complex, num_iters: u32,num_threads:u32,){
        let iters_per_thread = (num_iters/num_threads) as usize;
        let global_max:AtomicU64 = 0.into();
        self.reset();

        rayon::scope(|scope| {
            for _ in 0 .. num_threads {
                scope.spawn(|_|{
                    let mut rng = rand::rng();
                    let mut z = Complex::new(0.0, 0.0);
                    let mut local_max = 0;
                    for _ in 0..iters_per_thread{
                        z = ifs(&z,&mut rng);
                        
                        match self.complex_to_2d_index(&transform(&z)){
                            None => (),
                            Some(index) => {
                                let count = self.histogram[self.index2d_to_index1d(index)].fetch_add(1,Ordering::SeqCst) + 1;
                                if count > local_max { local_max = count;}
                            }
                        }
                    }
                    global_max.fetch_max(local_max, Ordering::SeqCst);
                });
            }
        });
        self.max = global_max.load(Ordering::SeqCst);
    }


    pub fn to_image(&self,image:&mut RgbaImage,color:Rgba<u8>, bgcolor:Rgba<u8>,gamma:f32){

        for (x,y,pixel) in image.enumerate_pixels_mut(){
            let value  = self.histogram[self.index2d_to_index1d((x as usize,y as usize))].load(Ordering::SeqCst);
            let value = ((( (value as f32) / (self.max as f32) )).powf(gamma) * color[3] as f32) as u8;

            *pixel = bgcolor;
            pixel.blend(&Rgba([color[0],color[1],color[2],value]))
        }
    }

}

pub fn ifs_from_closures(closures:Vec< Box<dyn '_ + Fn(&[Complex]) -> Complex + Sync>>, weights: Vec<f32>) -> Box<dyn '_ + ComplexIfsFunction >{
    let weighted_rng = WeightedAliasIndex::new(weights).unwrap();
    return Box::new(move |z,rng| closures[weighted_rng.sample(rng)]([*z].as_slice()))
}






