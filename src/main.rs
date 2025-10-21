mod ifs;


use std::sync::atomic::{AtomicU64, Ordering};

use num_complex::{Complex64};

use std::f32::consts::{PI,TAU};

use formulac::{compile, variable::{UserDefinedTable, Variables}};

use image::{Pixel, Rgba, RgbaImage};

use crate::ifs::{ifs_from_closures, IfsHistogram};
use eframe::{egui::{self, load::SizedTexture, ColorImage, TextureHandle, TextureOptions}, CreationContext};
use egui_extras;

type Complex = num_complex::Complex64;

fn main() -> eframe::Result {
   
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 200.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Image Viewer",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let fs = vec!["a*z + (1-a)*exp(i*0*tau/3)".to_string(),"a*z + (1-a)*exp(i*1*tau/3)".to_string(),"a*z + (1-a)*exp(i*2*tau/3)".to_string()];

            Ok(Box::<MyApp>::new(MyApp::new(
                cc,
                fs,
                vec![("a".to_string(),[0.0,0.0])],
                (500,500),
                Transform { translate: (0.0,0.0), scale: 1.0 },
                ColoringParams { gamma: 1.0, color: egui::Rgba::from_rgba_unmultiplied(1.0, 1.0, 1.0, 1.0), bg_color: egui::Rgba::from_rgba_unmultiplied(0.0,0.0,0.0,1.0) },
                10000,
                10
            ))
        )}
    ))
}
struct Transform{
    translate:(f32,f32),
    scale:f32,

}

struct ColoringParams{
    gamma:f32,
    color:egui::Rgba,
    bg_color:egui::Rgba,
}

struct MyApp {

    functions: Vec<String>,
    variables_vec:Vec<(String,[f32;2])>,
    variables_table:Variables,
    user_funcs:UserDefinedTable,
    weights: Vec<f32>,

    frame: (usize,usize),
    histogram:IfsHistogram,
    render_preveiw: RgbaImage,
    texture_handle: TextureHandle,

    transform: Transform,

    coloring_params:ColoringParams,

    num_iters: u32,
    num_threads:u32,
    
   
}

impl MyApp{
    pub fn new(cc:&CreationContext,functions:Vec<String>, variables_vec:Vec<(String,[f32;2])>, frame:(usize,usize), transform:Transform, coloring_params:ColoringParams, num_iters:u32, num_threads:u32 ) -> Self{
        
        MyApp{
            functions,
            variables_vec,
            user_funcs: UserDefinedTable::new(),
            variables_table: Variables::new(),
            weights: Vec::new(),

            frame,
            histogram: IfsHistogram::new(frame),

            render_preveiw: RgbaImage::new(frame.0 as u32, frame.1 as u32),
            texture_handle: cc.egui_ctx.load_texture("render_preveiw",egui::ColorImage::example(),TextureOptions::default() ),

            transform,
            coloring_params,

            num_iters,
            num_threads,
            


        }
    }
    fn update_histogram(&mut self){
        let closures:Vec<Box<dyn Fn(&[Complex]) -> Complex + Sync>>  = self.functions.iter().map(|f|{
            let o:Box<dyn Fn(&[Complex]) -> Complex + Sync> = Box::new(compile(f, &["z"], &self.variables_table, &self.user_funcs).unwrap());
            return o;
        }
        ).collect();
        let ifs = ifs_from_closures(closures, self.weights.clone());
        let transform = |z:&Complex|*z*0.5;
        self.histogram.iterate_ifs(&ifs, transform, self.num_iters, self.num_threads);
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        
        
        self.update_histogram();
        self.histogram.to_image(&mut self.render_preveiw, Rgba(self.coloring_params.color.to_srgba_unmultiplied()), Rgba(self.coloring_params.bg_color.to_srgba_unmultiplied()), self.coloring_params.gamma);
        let color_image = ColorImage::from_rgba_unmultiplied([self.frame.0, self.frame.1], &self.render_preveiw);
        self.texture_handle.set(color_image, TextureOptions::default());
        let sized_texture = egui::load::SizedTexture::new(&self.texture_handle, egui::vec2(self.frame.0 as f32, self.frame.1 as f32));
        egui::SidePanel::left("test_side_panel").show(ctx,|ui|{
            ui.add(egui::Slider::new(&mut self.transform.scale,0.0..=2.0))
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.image(sized_texture);
            });
        });
    }
    
}
/* 
fn main() { 
    let vars = Variables::new();
    let funcs = UserDefinedTable::new();

    let mut fs:Vec<Box<dyn Fn(&[Complex]) -> Complex + Sync>> = Vec::new();
    fs.push(Box::new(compile("0.5*z + 0.5*exp(0*i*TAU/3)", &["z"], &vars, &funcs).unwrap()));
    fs.push(Box::new(compile("0.5*z + 0.5*exp(1*i*TAU/3)", &["z"], &vars, &funcs).unwrap()));
    fs.push(Box::new(compile("0.5*z + 0.5*exp(2*i*TAU/3)", &["z"], &vars, &funcs).unwrap()));
  

    let fs = ifs_from_closures(fs, vec![1.0,1.0,1.0]);

    let mut hist = IfsHistogram::new((2000,2000));

    let transform = |z:&Complex| *z;

    hist.iterate_ifs(&fs, transform, 10usize.pow(5),5);

    let img = hist.to_image(Rgba([255,255,255,255]), Rgba([0,0,0,255]));
    img.save("output.png");

}


*/