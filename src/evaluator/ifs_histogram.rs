use image::{Pixel, Rgba, RgbaImage};

use crate::Complex;

#[derive(Clone)]
pub struct IfsHistogram {
    pub frame: [usize;2],
    pub max: u64,
    pub histogram: Box<[u64]>,
}

pub struct DrawParams{
    pub color: Rgba<u8>,
    pub bgcolor: Rgba<u8>,
    pub gamma: f64,
    pub brightnesss: f64,
    pub contrast: f64
}

impl IfsHistogram{
    pub fn new(frame:[usize;2]) -> Self{
        IfsHistogram {
            frame,
            max: 0,
            histogram:(0 .. (frame[0] * frame[1]) as u64)
                .into_iter()
                .map(|_| 0)
                .collect(),}
    }

    pub fn write_to_image(
        &self,
        image: &mut RgbaImage,
        draw_params:DrawParams
      
    ) {
        let color = draw_params.color;
        let bgcolor = draw_params.bgcolor;
        let gamma = draw_params.gamma;
        let brightness = draw_params.brightnesss;
        let contrast = draw_params.contrast;

        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let value = self.histogram[self.index2d_to_index1d((x as usize, y as usize))];
            let mut value = (value as f64) / (self.max as f64);
            value = contrast * (value - 0.5) + 0.5 + brightness;
            value = value.powf(gamma);
            value = value.clamp(0.0, 1.0);
            value = value* color[3] as f64;

            

            *pixel = bgcolor;
            pixel.blend(&Rgba([color[0], color[1], color[2], value as u8]))
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
}