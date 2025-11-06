use std::sync::atomic::{AtomicU64, Ordering};

use formulac::{compile, UserDefinedTable, Variables};
use image::{Pixel, Rgba, RgbaImage};
use num_complex::Complex64 as Complex;
use rand_distr::weighted::WeightedAliasIndex;
use rand::prelude::*;

pub trait ComplexIfsFunction: (Fn(&Complex, &mut ThreadRng) -> Complex) + Sync {}

impl<Func: Fn(&Complex, &mut ThreadRng) -> Complex + Sync> ComplexIfsFunction for Func {}

#[derive(Default)]
pub struct IfsHistogram {
    frame: [usize;2],
    max: u64,
    histogram: Box<[AtomicU64]>,
}

impl IfsHistogram {
    #[track_caller]
    pub fn new(frame: [u32;2]) -> Self {
        IfsHistogram {
            frame: [frame[0] as usize,frame[1] as usize],
            max: 0,
            histogram: (0 .. (frame[0] * frame[1]) as u64)
                .into_iter()
                .map(|_| AtomicU64::new(0))
                .collect(),
        }
    }

    fn reset(&mut self) {
        for i in 0 .. self.frame[0] * self.frame[1] {
            self.histogram[i].store(0, Ordering::Relaxed);
        }
    }

    fn complex_to_2d_index(&self, z: &Complex) -> Option<(usize, usize)> {
        let index: Complex = ((*z + Complex::new(1.0, 1.0)) / 2.0) * self.frame[1] as f64;
        if (0.0..self.frame[0] as f64).contains(&index.re) && (0.0..self.frame[1] as f64).contains(&index.im){
            return Some((index.re as usize, index.im as usize));
        } 
        else{
            return None
        }
    }

    fn index2d_to_index1d(&self, index: (usize, usize)) -> usize {
        return index.0 as usize + (index.1 as usize * self.frame[0]);
    }


    pub fn build_and_run_ifs(&mut self, functions_list:&Vec<String>, variables_table:&Variables, user_funcs:&UserDefinedTable, weights:Vec<f32>, rotate_scale:Complex, translate:Complex, iters:u64, num_threads:usize){
        let mut closures:Vec<Box<dyn Fn(&[Complex]) -> Complex + Sync>> = Vec::new();
        for function in functions_list.iter(){
                closures.push(Box::new( compile(function.as_str(), &["z"], variables_table, user_funcs).unwrap()))
        }
        let ifs:Box<dyn ComplexIfsFunction> = ifs_from_closures(closures, weights);
        self.iterate_ifs(&ifs, rotate_scale, translate, iters, num_threads);   
    }

    pub fn iterate_ifs(
        &mut self,
        ifs: &Box<dyn '_ + ComplexIfsFunction>,
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
                            z = Complex::new(0.0, 0.0);
                            continue;
                        }

                        match self.complex_to_2d_index(&(rotate_scale * z + translate)) {
                            None => (),
                            Some(index) => {
                                let count = self.histogram[self.index2d_to_index1d(index)]
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
        self.max = global_max.load(Ordering::SeqCst);
    }

  

    pub fn write_to_image(
        &self,
        image: &mut RgbaImage,
        color: Rgba<u8>,
        bgcolor: Rgba<u8>,
        gamma: f64,
        brightness: f64,
        contrast: f64,
        threshold: f64,
    ) {
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let value = self.histogram[self.index2d_to_index1d((x as usize, y as usize))]
                .load(Ordering::SeqCst);
            let mut value = (value as f64) / (self.max as f64);
            if value < threshold {
                value = 0.0
            }
            value = contrast * (value - 0.5) + 0.5 + brightness;
            value = value.powf(gamma);
            value = value.clamp(0.0, 1.0);
            value = value* color[3] as f64;

            

            *pixel = bgcolor;
            pixel.blend(&Rgba([color[0], color[1], color[2], value as u8]))
        }
    }
}

#[cfg(false)]
struct Transform {
    translate: Complex,
    scale: f64,
}

pub fn ifs_from_closures(
    closures: Vec<Box<dyn '_ + Fn(&[Complex]) -> Complex + Sync>>,
    weights: Vec<f32>,
) -> Box<dyn '_ + ComplexIfsFunction> {
    let weighted_rng = WeightedAliasIndex::new(weights).unwrap();
    return Box::new(move |z, rng| closures[weighted_rng.sample(rng)]([*z].as_slice()));
}
