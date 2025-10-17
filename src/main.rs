

use std::sync::atomic::{AtomicU64, Ordering};

use num_complex::{Complex64};
use std::f64::consts::{PI,TAU};
use rand::prelude::*;
use rand_distr::weighted::WeightedAliasIndex;

use formulac::{compile, variable::UserDefinedTable, Variables};

use rayon;

use image::{Pixel, Rgba, RgbaImage};

type Complex = num_complex::Complex64;
const I:Complex = Complex::new(0.0,1.0);
fn main() { 

    println!("initializing");
    let vars = Variables::new();
  
    let users = UserDefinedTable::new();

    let f_1 = "0.5*z + 0.5*exp(i*0*TAU/3)";

    let f_2 = "0.5*z + 0.5*exp(i*1*TAU/3)";

    let f_3 = "0.5*z + 0.5*exp(i*2*TAU/3)";


    let mut fs: Vec< Box<dyn Fn(&[Complex]) -> Complex + Sync>> = Vec::new();
    fs.push(Box::new(compile(f_1, &["z"], &vars, &users).expect("failure")));
    fs.push(Box::new(compile(f_2, &["z"], &vars, &users).expect("failure")));
    fs.push(Box::new(compile(f_3, &["z"], &vars, &users).expect("failure")));
    let z = Complex::new(0.0,0.0);
    let test_1 = fs[2](&[z]);
    let test_2 = z*0.5 + 0.5*Complex64::cis(2.0*TAU/3.0);

    println!("{test_1} {test_2}");

    let frame:(usize,usize) = (2000,2000);

    println!("generated closures");

    let ifs = ifs_from_closures(fs, vec![1.0,1.0,1.0]);
    println!("generated ifs");
    let mut hist = IfsHistogram::new(frame);

    let transform = |z:&Complex| *z;
    let fitting_func = |z:&Complex| {
        let result:Complex =((*z + Complex::new(1.0,1.0))/2.0) * 2000.0;
        if result.re >=2000.0 || result.re < 0.0 || result.im >= 2000.0 || result.re < 0.0{
            return None
        }
        //println!("{result}");
        let index = result.re as usize + (result.im as usize *2000);
        if index > 4000000{
            println!("{result}");
        }
        return Some(index);
    };
    println!("generated ifs");
    hist.iterate_ifs(&ifs, transform, fitting_func, 10usize.pow(8), 10);

    println!("generated histogram");
    let image = hist.to_image(Rgba([255,255,255,255]), Rgba([0,0,0,255]));
    image.save("img.png").expect("couldn't save");


}


pub trait ComplexIfsFunction: (Fn(&Complex, &mut ThreadRng) -> Complex) + Sync { 

    
}

impl<Func: Fn(&Complex,&mut ThreadRng) -> Complex + Sync> ComplexIfsFunction for Func {

}

struct IfsHistogram{
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

    fn iterate_ifs(&mut self,ifs:&Box<dyn '_+ ComplexIfsFunction >, transform: fn(&Complex) -> Complex, fitting_func: fn(&Complex) -> Option<usize>, num_iters: usize,num_threads:usize,){
        let iters_per_thread = (num_iters/num_threads) as usize;
        let global_max:AtomicU64 = 0.into();

        rayon::scope(|scope| {
            for _ in 0 .. num_threads {
                scope.spawn(|_|{
                    let mut rng = rand::rng();
                    let mut z = Complex::new(0.0, 0.0);
                    let mut local_max = 0;
                    for _ in 0..iters_per_thread{
                        z = ifs(&z,&mut rng);
                        
                        match fitting_func(&transform(&z)){
                            None => println!("out of bounds"), 
                            Some(index) => {
                                let count = self.histogram[index].fetch_add(1,Ordering::SeqCst) + 1;
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


    fn to_image(self,color:Rgba<u8>, bgcolor:Rgba<u8>) -> RgbaImage{

        let mut image = RgbaImage::new(self.frame.0 as u32, self.frame.1 as u32);
        for (x,y,pixel) in image.enumerate_pixels_mut(){
            let value  = self.histogram[(x as usize) + self.frame.0*(y as usize)].load(Ordering::SeqCst);
            let value = ((( (value as f32) / (self.max as f32) )) * color[3] as f32) as u8;

            *pixel = bgcolor;
            pixel.blend(&Rgba([color[0],color[1],color[2],value]))
        }
        return image;
    }

}

fn ifs_from_closures(closures:Vec< Box<dyn '_ + Fn(&[Complex]) -> Complex+ Sync>>, weights: Vec<f64>) -> Box<dyn '_ + ComplexIfsFunction >{
let weighted_rng = WeightedAliasIndex::new(weights).unwrap();
return Box::new(move |z,rng| closures[weighted_rng.sample(rng)]([*z].as_slice()))
}






