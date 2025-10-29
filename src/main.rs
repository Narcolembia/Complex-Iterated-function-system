mod ifs;


use std::{default, ops::RangeInclusive, str::FromStr, sync::{atomic::{AtomicU64, Ordering}, mpsc}, thread};


use std::f64::consts::{PI,TAU};

use num_complex::{Complex64};
use formulac::{compile, variable::{self, UserDefinedTable, Variables}};
use image::{Pixel, Rgba, RgbaImage};
use eframe::{CreationContext, egui::{self, Color32, ColorImage, Slider, TextureHandle, TextureOptions, load::SizedTexture}};
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
            
           
            Ok(Box::<MyApp>::new(MyApp::new(cc, Default::default(), Default::default(), Default::default(), Default::default(), Default::default()))
        )}
    ))
}

#[derive(Clone)]
struct SliderData{
    val: f64,
    min: TextboxData<f64>,
    max: TextboxData<f64>,
}


impl SliderData{
    fn new(val:f64,min:f64,max:f64) -> Self{
        SliderData {val, min: TextboxData::new(min) , max: TextboxData::new(max)  }
    }
}
  
impl Default for SliderData{
    fn default() -> Self {
        SliderData { val: 0.0, min: TextboxData::new(0.0), max:TextboxData::new(1.0) }
    }
}

#[derive(Clone)]
struct TextboxData<T: FromStr + ToString + Clone>{
    text:String,
    value:T
}

impl<T: FromStr + ToString + Clone> TextboxData<T>{
    fn new(val:T) -> Self{
        TextboxData { text:val.to_string(), value: val }
    }
    fn update(&mut self) -> Result<(),T::Err>{
        self.value = 
            match self.text.parse::<T>(){
            Ok(value) => { value },
            Err(e) => return Err(e),
        };
        Ok(())
    }

    fn check_val(&self) -> Result<(),T::Err>{
        match self.text.parse::<T>(){
            Ok(_) => Ok(()),
            Err(e) => return Err(e),
        }
    }
}

struct TransformParams{
    translate:[SliderData;2],
    scale:SliderData,
    rotate:SliderData,
}

impl TransformParams{
    pub fn get_rotate_scale(&self) -> Complex{
        Complex::from_polar(self.scale.val, self.rotate.val * TAU)
    }

    pub fn get_translate(&self) -> Complex{
        Complex::new(self.translate[0].val, self.translate[1].val)
    }
}

impl Default for TransformParams{
    fn default() -> TransformParams{
        TransformParams{
            translate:[SliderData::default(),SliderData::default()],
            scale: SliderData::new(1.0,  0.0, 1.0),
            rotate: SliderData::new(0.0,  0.0, 1.0)}
    }
}



struct FunctionParams{
    text: String,
    weights:Vec<SliderData>,
    variables:Vec<(String,SliderData,SliderData)>,
}
impl Default for FunctionParams{
    fn default() -> Self {
        Self { text:
            "a*z + (1-a)*exp(i*0*TAU/3);
            a*z + (1-a)*exp(i*1*TAU/3);
            a*z + (1-a)*exp(i*2*TAU/3);".to_string(),
            weights: vec![SliderData::new(1.0,0.0,1.0),SliderData::new(1.0,0.0,1.0),SliderData::new(1.0,0.0,1.0)],
            variables: vec![
                ("a".to_string(),SliderData::default(),SliderData::default()),
                ("b".to_string(),SliderData::default(),SliderData::default()),
                ("c".to_string(),SliderData::default(),SliderData::default()),
                ("d".to_string(),SliderData::default(),SliderData::default())
                ] }
    }
}
impl FunctionParams{
    pub fn parse_textbox(&self) -> Vec<String>{
        self.text.chars().filter(|c| !c.is_whitespace()).collect::<String>().splitn(100,";").map(|s|s.to_string()).filter(|s| s != "").collect() 
    }

    pub fn get_weights(&self) -> Vec<f32>{
        return self.weights.iter().map(|w| w.val as f32).collect()
    }
}
struct DrawParams{
    gamma:SliderData,
    brightness:SliderData,
    contrast:SliderData,
    threshold:SliderData,
    color:egui::Rgba,
    bg_color:egui::Rgba, 
}

impl Default for DrawParams{
    fn default() -> Self {
        Self { 
            gamma: SliderData::new(1.0, 0.0, 2.0),
            brightness: SliderData::new(0.0, 0.0, 1.0),
            contrast: SliderData::new(1.0, 0.0, 2.0),
            threshold: SliderData::new(0.0, 0.0, 1.0),
            color: Color32::WHITE.into(),
            bg_color: Color32::BLACK.into() }
    }
}

struct PreviewParams{
    frame: [TextboxData<u32>;2],
    num_iters: TextboxData<u64>,
    num_snapshot_iters: TextboxData<u64>,
    num_threads: TextboxData<u32>,
}

impl Default for PreviewParams{
    fn default() -> Self {
        PreviewParams {
            frame: [TextboxData::new(1000),TextboxData::new(1000)],
            num_iters: TextboxData::new(1000000),
            num_snapshot_iters: TextboxData::new(10000000),
            num_threads: TextboxData::new(10) 
        }
    }
}

struct ExportParams{
    frame: [TextboxData<u32>;2],
    num_iters: TextboxData<u64>,
    file_name: String,
}

impl Default for ExportParams{
    fn default() -> Self {
        ExportParams { 
            frame: [TextboxData::new(1000),TextboxData::new(1000)],
            num_iters: TextboxData::new(1000000),
            file_name: "".to_string(),
        }
    }
}

struct Flags{
    update_histogram_flag:bool,
    snapshot_flag:bool,
    redraw_flag:bool,
    compile_flag:bool,
    export_flag:bool,
    rendering_flag:bool,
    parse_error_flag:bool,
    reallocate_preveiw_historgram:bool,
}

impl Default for Flags{
    fn default() -> Self {
        Flags{
            update_histogram_flag:true,
            snapshot_flag:false,
            redraw_flag:true,
            compile_flag:true,
            export_flag:false, 
            rendering_flag:false,
            parse_error_flag:false,
            reallocate_preveiw_historgram:true,
        }
    }
}



struct MyApp {

    functions_params:FunctionParams,
    transform_params:TransformParams,
    draw_params:DrawParams,
    preview_params:PreviewParams,
    export_params:ExportParams,
    flags:Flags,

    functions_list: Vec<String>,
    variables_table: Variables,
    user_funcs: UserDefinedTable,
    preveiw_histogram: IfsHistogram,
    export_histogram:IfsHistogram,

    preveiw_image: RgbaImage,
    texture_handle: egui::TextureHandle,

    //worker_thread: thread::JoinHandle<()>,
    //job_sender: mpsc::Sender<RenderJob>,
    //job_result_receiver: mpsc::Receiver<IfsHistogram>,
}

impl MyApp{
    pub fn new(cc: &CreationContext, functions_params:FunctionParams, transform_params:TransformParams, draw_params:DrawParams, preview_params:PreviewParams, export_params:ExportParams) -> Self{
        let functions_list = functions_params.parse_textbox();
        for func in functions_list.iter() { if func == "" {println!("whitespace!")}};
        
        let preveiw_image = RgbaImage::new(1,1);
        MyApp{
            functions_params,
            transform_params,
            draw_params,
            preview_params,
            export_params,

            flags: Default::default(),

            functions_list,
            variables_table: Variables::new(),
            user_funcs: UserDefinedTable::new(),
            preveiw_image: preveiw_image.clone(),
            texture_handle: cc.egui_ctx.load_texture("preveiw", ColorImage::from_rgba_unmultiplied([1,1], &preveiw_image),TextureOptions::default()),

            preveiw_histogram: IfsHistogram::new([1,1]),
            export_histogram: IfsHistogram::new([1,1]),
            

            //ob_sender,
            //job_result_receiver,
        }
    }
    fn build_ifs(&self) -> Result<(),String>{
        let functions_list = &self.functions_list;
        let variables_table = &self.variables_table;
        let user_funcs = &self.user_funcs;
        for function in functions_list.iter(){
            match compile(function.as_str(), &["z"], variables_table, user_funcs){
                Ok(_) => (),
                Err(e) => return Err(e)
            }
        }
        return Ok(())
    }
    fn build_and_run_ifs(&mut self){
        let mut closures:Vec<Box<dyn '_+ Fn(&[Complex]) -> Complex + Sync>> = Vec::new();
        let functions_list = &self.functions_list;
        let variables_table = &self.variables_table;
        let user_funcs = &self.user_funcs;
        let weights = self.functions_params.get_weights();
        let rotate_scale = self.transform_params.get_rotate_scale();
        let translate = self.transform_params.get_translate();
        let iters = if self.flags.snapshot_flag {self.preview_params.num_snapshot_iters.value} else {self.preview_params.num_iters.value};
        let num_threads = self.preview_params.num_threads.value;
        
        for function in functions_list.iter(){
                closures.push(Box::new( compile(function.as_str(), &["z"], variables_table, user_funcs).unwrap()))
        }
        let ifs:Box<dyn ComplexIfsFunction +'_> = ifs_from_closures(closures, weights);
        self.preveiw_histogram.iterate_ifs(&ifs, rotate_scale, translate, iters, num_threads);
        
    }
    
   
    
}

impl eframe::App for MyApp {
    
    
    
    
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        fn make_slider(ui: &mut egui::Ui, data: &mut SliderData, name: String, hist_update_flag:Option<&mut bool>, redraw_flag:Option<&mut bool>){
            let mut set_updates = false;
            let min_color = {match data.min.update(){Ok(_) => Color32::WHITE, Err(_) => Color32::RED}};
            let max_color = {match data.max.update(){Ok(_) => Color32::WHITE, Err(_) => Color32::RED}};
            
            ui.horizontal(|ui|{
                ui.add(egui::Label::new(name));
                if ui.add(egui::Slider::new(&mut data.val,data.min.value..=data.max.value)).changed(){
                    set_updates = true;
                }
                if ui.add(egui::TextEdit::singleline(&mut data.min.text).desired_width(50.0).background_color(min_color)).lost_focus() { 
                    set_updates = true;
                };
                if ui.add(egui::TextEdit::singleline(&mut data.max.text).desired_width(50.0).background_color(max_color)).lost_focus() { 
                    set_updates = true;
                };
    
            });
            if set_updates{
                match hist_update_flag{
                    Some(flag)=> *flag = true,
                        None => ()
                    }
                match redraw_flag{
                    Some(flag)=> *flag = true,
                        None => ()
                    }
            } 
        }

        fn make_textbox<T: FromStr + ToString + Clone>(ui: &mut egui::Ui, data: &mut TextboxData<T>, name: String, flags: Vec<&mut bool>){
            ui.horizontal(|ui|{
                ui.add(egui::Label::new(name));
                 let color = {match data.check_val(){Ok(_) => Color32::WHITE, Err(_) => Color32::RED}};
                if ui.add(egui::TextEdit::singleline(&mut data.text).background_color(color).desired_width(100.0)).lost_focus() { 
                    let _ = data.update();
                    for flag in flags{
                        *flag = true; 
                    }
                };
            });
        }

         
    
        
        egui::SidePanel::left("test_side_panel").show(ctx,|ui|{

            if self.flags.rendering_flag { ui.disable();}
            
            ui.add(egui::Label::new("Transform"));
            make_slider(ui, &mut self.transform_params.translate[0], "translate x".to_string(), Some(&mut self.flags.update_histogram_flag), Some(&mut self.flags.redraw_flag));
            make_slider(ui, &mut self.transform_params.translate[0], "translate y".to_string(), Some(&mut self.flags.update_histogram_flag), Some(&mut self.flags.redraw_flag));
            make_slider(ui, &mut self.transform_params.scale, "scale".to_string() , Some(&mut self.flags.update_histogram_flag), Some(&mut self.flags.redraw_flag));
            make_slider(ui, &mut self.transform_params.rotate, "rotate".to_string(), Some(&mut self.flags.update_histogram_flag), Some(&mut self.flags.redraw_flag));

            ui.add(egui::Separator::default());


            ui.add(egui::Label::new("Variables"));
            for (name,mag_data,phase_data) in self.functions_params.variables.iter_mut(){

                make_slider(ui, mag_data, name.to_string() + ": magnitude", Some(&mut self.flags.update_histogram_flag), Some(&mut self.flags.redraw_flag));
                make_slider(ui, phase_data, name.to_string() + ": phase          ", Some(&mut self.flags.update_histogram_flag), Some(&mut self.flags.redraw_flag));
                let z = mag_data.val*Complex::cis(phase_data.val*TAU);
                self.variables_table.insert(&[(name.as_str(),z)]);
                ui.add(egui::Separator::default().shrink(20.0));
            }
            ui.add(egui::Separator::default());

            ui.add(egui::Label::new("weights"));
            for (index, weight) in self.functions_params.weights.iter_mut().enumerate(){
                make_slider(ui, weight, index.to_string(), Some(&mut self.flags.update_histogram_flag), Some(&mut self.flags.redraw_flag));
            }
            ui.add(egui::Separator::default());

            ui.add(egui::Label::new("color"));
            
            make_slider(ui, &mut self.draw_params.gamma, "gamma".to_string(), None, Some(&mut self.flags.redraw_flag));
            make_slider(ui, &mut self.draw_params.brightness, "brightness".to_string(),  None, Some(&mut self.flags.redraw_flag));
            make_slider(ui, &mut self.draw_params.contrast, "contrast".to_string(),  None, Some(&mut self.flags.redraw_flag));
            make_slider(ui, &mut self.draw_params.threshold, "threshold".to_string(),  None, Some(&mut self.flags.redraw_flag));
            if egui::color_picker::color_edit_button_rgba(ui, &mut self.draw_params.color, egui::color_picker::Alpha::BlendOrAdditive).changed(){ self.flags.redraw_flag = true;};
            if egui::color_picker::color_edit_button_rgba(ui, &mut self.draw_params.bg_color, egui::color_picker::Alpha::BlendOrAdditive).changed(){ self.flags.redraw_flag = true;};
            ui.add(egui::Separator::default());
            
            ui.add(egui::Label::new("preveiw settings"));
             ui.horizontal(|ui|{
                ui.add(egui::Label::new("resolution"));
                make_textbox(ui, &mut self.preview_params.frame[0], "".to_string(), vec![&mut self.flags.reallocate_preveiw_historgram,&mut self.flags.update_histogram_flag,&mut self.flags.redraw_flag]);
                make_textbox(ui, &mut self.preview_params.frame[1], "".to_string(), vec![&mut self.flags.reallocate_preveiw_historgram,&mut self.flags.update_histogram_flag,&mut self.flags.redraw_flag]);
                
            });
            make_textbox(ui, &mut self.preview_params.num_iters, "iterations".to_string(), vec![&mut self.flags.redraw_flag,&mut self.flags.update_histogram_flag]);
            make_textbox(ui, &mut self.preview_params.num_snapshot_iters, "snapshot iters".to_string(),vec![]);
            if ui.button("snapshot").clicked(){
                self.flags.snapshot_flag = true;
                self.flags.update_histogram_flag = true;
                self.flags.redraw_flag = true;
            }
            

            ui.add(egui::Label::new("export settings"));
            make_textbox(ui, &mut self.export_params.num_iters, "iterations".to_string(), vec![]);

            ui.horizontal(|ui|{
                ui.add(egui::Label::new("resolution"));
                make_textbox(ui, &mut self.export_params.frame[0], "".to_string(), vec![]);
                make_textbox(ui, &mut self.export_params.frame[1], "".to_string(), vec![]);
                
            });
            
            
        });

        egui::SidePanel::right("textbox").show(ctx, |ui|{
            ui.add(egui::TextEdit::multiline(&mut self.functions_params.text).desired_width(10000.0).desired_rows(50));
            ui.horizontal(|ui|{
                if ui.button("compile functions").clicked(){
                    self.functions_list = self.functions_params.parse_textbox();
                    let err = match self.build_ifs() { Ok(_) => false, Err(_) => true};
                    self.flags.parse_error_flag = err;
                    if !self.flags.parse_error_flag{ self.functions_params.weights = vec![SliderData::new(1.0, 0.0, 1.0);self.functions_list.len()];}
                }
                if self.flags.parse_error_flag { ui.add(egui::Label::new("COMPILE ERROR!")); };
            });
        });


        
        //println!("updated variables");
        
        
        if self.flags.reallocate_preveiw_historgram{
            self.preveiw_histogram = IfsHistogram::new([self.preview_params.frame[0].value,self.preview_params.frame[1].value]);
            self.preveiw_image = RgbaImage::new(self.preview_params.frame[0].value, self.preview_params.frame[1].value);
            self.flags.reallocate_preveiw_historgram = false;
        }
        
        if self.flags.update_histogram_flag && !self.flags.parse_error_flag{
            self.build_and_run_ifs();
            self.flags.update_histogram_flag = false;
            self.flags.snapshot_flag = false;
        }
        if self.flags.redraw_flag{
            self.preveiw_histogram.write_to_image(
                &mut self.preveiw_image,
                Rgba(self.draw_params.color.to_srgba_unmultiplied()),
                Rgba(self.draw_params.bg_color.to_srgba_unmultiplied()),
                self.draw_params.gamma.val,
                self.draw_params.brightness.val,
                self.draw_params.contrast.val,
                self.draw_params.threshold.val,
                );
            //println!("generated image");
            let color_image = ColorImage::from_rgba_unmultiplied([self.preview_params.frame[0].value as usize, self.preview_params.frame[1].value as usize], &self.preveiw_image);
            self.texture_handle.set(color_image, TextureOptions::default());
            self.flags.redraw_flag = false;
        }
        /* 
        if self.export_flag{
            //self.export_histogram = IfsHistogram::new(self.export_resolution.0)
        }
        */

       
            
        
        //println!("updated histogram");
       
        
        let sized_texture = egui::load::SizedTexture::new(&self.texture_handle, egui::vec2(self.preview_params.frame[0].value as f32, self.preview_params.frame[1].value as f32));
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
/* 
fn worker_thread(jobs: mpsc::Receiver<RenderJob>, job_results: mpsc::Sender<IfsHistogram>) {
    for job in jobs.iter() {
        let RenderJob { mut histogram, .. } = job;
        job_results.send(histogram).unwrap
   }
j =  .. ,margotsih tum {} 
   }
}
   */