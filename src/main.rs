mod ifs;


use std::{ops::RangeInclusive, str::FromStr, sync::atomic::{AtomicU64, Ordering}, thread, sync::mpsc};


use std::f64::consts::{PI,TAU};

use num_complex::{Complex64};
use formulac::{compile, variable::{UserDefinedTable, Variables}};
use image::{Pixel, Rgba, RgbaImage};
use eframe::{egui::{self, load::SizedTexture, Color32, ColorImage, TextureHandle, TextureOptions}, CreationContext};
// use egui_extras;

use crate::ifs::{ifs_from_closures, ComplexIfsFunction, IfsHistogram};

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

            let fs = vec!["tan(a*z + (1-a)*exp(i*0*TAU/3))".to_string(),"tan(a*z + (1-a)*exp(i*1*TAU/3))".to_string(),"tan(a*z + (1-a)*exp(i*2*TAU/3))".to_string()];

            Ok(Box::<MyApp>::new(MyApp::new(
                cc,
                fs,
                vec![("a".to_string(),[SliderData::default(),SliderData::default()])],
                (1000,1000),
                Transform { 
                    translate: (SliderData::new(0.0,-1.0,1.0),SliderData::new(0.0,-1.0,1.0)),
                    scale: SliderData::new(1.0,0.0,1.0),
                    rotate: SliderData::new(0.0,0.0,1.0) },
                ColoringParams { 
                    gamma: SliderData::new(1.0,0.0,2.0),
                    threshold: SliderData::new(0.0,0.0,1.0),
                    brightness:SliderData::new(0.0, 0.0, 1.0 ),
                    contrast: SliderData::new(1.0, 0.0, 2.0 ),
                    color: egui::Rgba::from_rgba_unmultiplied(1.0, 1.0, 1.0, 1.0),
                    bg_color: egui::Rgba::from_rgba_unmultiplied(0.0,0.0,0.0,1.0) },
                1000000,
                1000000,
                100000,
                (2000,2000),
                10,

            ))
        )}
    ))
}
struct Transform{
    translate:(SliderData,SliderData),
    scale:SliderData,
    rotate:SliderData,
}

struct ColoringParams{
    gamma:SliderData,
    brightness:SliderData,
    contrast:SliderData,
    threshold:SliderData,
    color:egui::Rgba,
    bg_color:egui::Rgba,
    
}

#[derive(Clone)]
struct SliderData{
    val: f64,
    min: TextBoxData<f64>,
    max: TextBoxData<f64>
}

#[derive(Clone)]
struct TextBoxData<T: ToString + FromStr+Clone>{
    text: String,
    value: T,
}

impl Default for TextBoxData<f64>{
    fn default() -> Self {
        TextBoxData::new(0.0)
    }
}
impl<T: ToString + FromStr + Clone> TextBoxData<T>{
    fn new(value: T) -> Self{
        TextBoxData{
            text: value.to_string(),
            value,
        }
    }

    fn update(&mut self){
        match self.text.parse::<T>(){
            Ok(value)=> self.value = value,
            Err(_) =>(),
        }
    }
}


impl SliderData{
    fn new(val:f64,min:f64,max:f64) -> Self{
        SliderData {val, min:TextBoxData::new(min), max:TextBoxData::new(max) }
    }
    fn update(&mut self){
        self.min.update();
        self.max.update()
    }
}
impl Default for SliderData{
    fn default() -> Self {
        SliderData { val: 0.0, min: TextBoxData::new(0.0), max: TextBoxData::new(1.0) }
    }
}



struct MyApp {

    functions: Vec<String>,
    variables_vec:Vec<(String,[SliderData;2])>,
    variables_table:Variables,
    user_funcs:UserDefinedTable,
    weights: Vec<SliderData>,

    frame: (usize,usize),
    preveiw_histogram: IfsHistogram,
    export_histogram: IfsHistogram,

    render_preveiw: RgbaImage,
    texture_handle: TextureHandle,

    transform: Transform,

    coloring_params: ColoringParams,

    num_iters: TextBoxData<u64>,
    high_res_snapshot_iters: TextBoxData<u64>,
    num_threads: u32,

    export_num_iters: TextBoxData<u64>,
    export_resolution: (TextBoxData<usize>,TextBoxData<usize>),
    
    export_flag:bool,
    high_res_snapshot_flag: bool,
    update_histogram_flag: bool,
    redraw_flag: bool,
    disable_ui_flag: bool,

    worker_thread: thread::JoinHandle<()>,
    job_sender: mpsc::Sender<RenderJob>,
    job_result_receiver: mpsc::Receiver<IfsHistogram>,
}

impl MyApp{
    pub fn new(
        cc:&CreationContext,functions:Vec<String>,
        variables_vec:Vec<(String,[SliderData;2])>,
        frame:(usize,usize),
        transform:Transform,
        coloring_params:ColoringParams,
        num_iters:u64, high_res_snapshot_iters:u64,
        export_num_iters:u64,export_resolution:(usize,usize), 
        num_threads:u32
        ) -> Self{
        let len = functions.len();
        let (job_sender, job_receiver) = mpsc::channel(); // FIXME: `crossbeam_channel::bounded(1)`
        let (job_result_sender, job_result_receiver) = mpsc::channel(); // FIXME: ditto
        MyApp{
            functions,
            variables_vec,
            user_funcs: UserDefinedTable::new(),
            variables_table: Variables::new(),
            weights: vec![SliderData::new(1.0, 0.0, 1.0);len],

            frame,
            preveiw_histogram: IfsHistogram::new(frame),
            export_histogram: IfsHistogram::new(export_resolution),

            render_preveiw: RgbaImage::new(frame.0 as u32,frame.1 as u32),
            texture_handle: cc.egui_ctx.load_texture("render_preveiw",egui::ColorImage::example(),TextureOptions::default() ),

            transform,
            coloring_params,

            num_iters: TextBoxData::new(num_iters),
            high_res_snapshot_iters: TextBoxData::new(high_res_snapshot_iters),
            num_threads,

            export_num_iters: TextBoxData::new(export_num_iters),
            export_resolution: (TextBoxData::new(export_resolution.0),TextBoxData::new(export_resolution.1)),

            update_histogram_flag: true,
            redraw_flag:true,
            disable_ui_flag:false,

            worker_thread: thread::spawn(move || worker_thread(job_receiver, job_result_sender)),
            job_sender,
            job_result_receiver,
        }
    }

    fn get_weights(&self) -> Vec<f32>{
        return self.weights.iter().map(|w| w.val as f32).collect()
    }

    fn get_rotate_scale(&self) -> Complex{
        return self.transform.scale.val * Complex::cis(self.transform.rotate.val)
    }
    
    fn get_translate(&self) -> Complex{
        return Complex::new(self.transform.translate.0.val,self.transform.translate.1.val)
    }

    fn get_export_resolution(&self) -> (usize,usize){
        return (self.export_resolution.0, self.export_resolution.1)
    }
    
   
    
}

impl eframe::App for MyApp {
    
    
    
    
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        fn make_slider(ui: &mut egui::Ui, data: &mut SliderData, name: String, set_hist_update:Option<&mut bool>, set_redraw:Option<&mut bool>){
            let mut set_updates = false;
            ui.horizontal(|ui|{
                ui.add(egui::Label::new(name));
                if ui.add(egui::Slider::new(&mut data.val,data.min.value..=data.max.value)).changed(){
                    set_updates = true;
                }
                if ui.add(egui::TextEdit::singleline(&mut data.min.text).desired_width(100.0)).lost_focus() { 
                    data.update();
                    set_updates = true;
                };
                if ui.add(egui::TextEdit::singleline(&mut data.max.text).desired_width(100.0)).lost_focus() { 
                    data.update();
                    set_updates = true;
                };
    
            });
            if set_updates{
                match set_hist_update{
                    Some(flag)=> *flag = true,
                        None => ()
                    }
                match set_redraw{
                    Some(flag)=> *flag = true,
                        None => ()
                    }
            } 
        }

        fn make_textbox<T: ToString + FromStr + Clone>(ui: &mut egui::Ui, data: &mut TextBoxData<T>, name: String, set_hist_update:Option<&mut bool>, set_redraw:Option<&mut bool>){
            ui.horizontal(|ui|{
                ui.add(egui::Label::new(name));
                if ui.add(egui::TextEdit::singleline(&mut data.text)).lost_focus() { 
                    data.update();
                    match set_hist_update{
                        Some(flag)=> *flag = true,
                        None => ()
                    }
                    match set_redraw{
                        Some(flag)=> *flag = true,
                        None => ()
                    }
                };
            });
          
        }

         
       
       
        
        egui::SidePanel::left("test_side_panel").show(ctx,|ui|{

            if self.disable_ui_flag { ui.disable();}
            
            ui.add(egui::Label::new("Transform"));
            make_slider(ui, &mut self.transform.translate.0, "translate x".to_string(), Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));
            make_slider(ui, &mut self.transform.translate.1, "translate y".to_string(), Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));
            make_slider(ui, &mut self.transform.scale, "scale".to_string() , Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));
            make_slider(ui, &mut self.transform.rotate, "rotate".to_string(), Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));

            ui.add(egui::Separator::default());


            ui.add(egui::Label::new("Variables"));
            for (name,data) in self.variables_vec.iter_mut(){

                make_slider(ui, &mut data[0], name.to_string() + ": magnitude", Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));
                make_slider(ui, &mut data[1], name.to_string() + ": phase          ", Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));
                let z = data[0].val*Complex::cis(data[1].val*TAU);
                self.variables_table.insert(&[(name.as_str(),z)]);
            }
            ui.add(egui::Separator::default());

            ui.add(egui::Label::new("weights"));
            for (index, weight) in self.weights.iter_mut().enumerate(){
                make_slider(ui, weight, index.to_string(), Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));
            }
            ui.add(egui::Separator::default());

            ui.add(egui::Label::new("color"));
            
            make_slider(ui, &mut self.coloring_params.gamma, "gamma".to_string(), None, Some(&mut self.redraw_flag));
            make_slider(ui, &mut self.coloring_params.brightness, "brightness".to_string(), None, Some(&mut self.redraw_flag));
            make_slider(ui, &mut self.coloring_params.contrast, "contrast".to_string(), None, Some(&mut self.redraw_flag));
            make_slider(ui, &mut self.coloring_params.threshold, "threshold".to_string(), None, Some(&mut self.redraw_flag));
            if egui::color_picker::color_edit_button_rgba(ui, &mut self.coloring_params.color, egui::color_picker::Alpha::BlendOrAdditive).changed(){ self.redraw_flag = true;};
            if egui::color_picker::color_edit_button_rgba(ui, &mut self.coloring_params.bg_color, egui::color_picker::Alpha::BlendOrAdditive).changed(){ self.redraw_flag = true;};
            ui.add(egui::Separator::default());
            
            ui.add(egui::Label::new("iteration settings"));
            make_textbox(ui, &mut self.num_iters, "num_iters".to_string(), Some(&mut self.update_histogram_flag), Some(&mut self.redraw_flag));
            make_textbox(ui, &mut self.high_res_snapshot_iters, "snapshot iters".to_string(), None, None);
            if ui.button("snapshot").clicked(){
                self.update_histogram_flag = true;
                self.high_res_snapshot_flag = true;
                self.redraw_flag = true;
            }
            

            ui.add(egui::Label::new("export settings"));
            make_textbox(ui, &mut self.export_num_iters, "export iteres".to_string(), None, None);

            ui.horizontal(|ui|{
                ui.add(egui::Label::new("export resolution"));
                make_textbox(ui, &mut self.export_resolution.0, "".to_string(), None, None);
                make_textbox(ui, &mut self.export_resolution.1, "".to_string(), None, None);
                
            });
            
            
        });
        //println!("updated variables");

        
        
        if self.update_histogram_flag{
            let iters = if self.high_res_snapshot_flag = true {self.high_res_snapshot_iters.value} else {self.num_iters.value};
            self.preveiw_histogram.update(&self.functions,self.get_weights(),&self.variables_table,&self.user_funcs,self.get_rotate_scale(),self.get_translate(),iters,self.num_threads);
            self.update_histogram_flag = false;
        }
        if self.redraw_flag{
            self.preveiw_histogram.write_to_image(
                &mut self.render_preveiw,
                Rgba(self.coloring_params.color.to_srgba_unmultiplied()),
                Rgba(self.coloring_params.bg_color.to_srgba_unmultiplied()),
                self.coloring_params.gamma.val,
                self.coloring_params.brightness.val,
                self.coloring_params.contrast.val,
                self.coloring_params.threshold.val,
                );
            //println!("generated image");
            let color_image = ColorImage::from_rgba_unmultiplied([self.frame.0, self.frame.1], &self.render_preveiw);
            self.texture_handle.set(color_image, TextureOptions::default());
            self.redraw_flag = false;
        }
        if self.export_flag{
            self.export_histogram = IfsHistogram::new(self.export_resolution.0)
        }
        //println!("updated histogram");
       
        
        let sized_texture = egui::load::SizedTexture::new(&self.texture_handle, egui::vec2(self.frame.0 as f32, self.frame.1 as f32));
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.image(sized_texture); 
            });
        });
    }
    
}

struct RenderJob {
    histogram: IfsHistogram,
    // TODO
}

fn worker_thread(jobs: mpsc::Receiver<RenderJob>, job_results: mpsc::Sender<IfsHistogram>) {
    for job in jobs.iter() {
        let RenderJob { mut histogram, .. } = job;
        job_results.send(histogram).unwrap
   }
j =  .. ,margotsih tum {} 
   }
}