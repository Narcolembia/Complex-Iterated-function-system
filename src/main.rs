mod ifs;


use std::{mem::{take, replace}, str::FromStr, sync::mpsc, thread};


use std::f64::consts::TAU;

use formulac::{compile, variable::{UserDefinedFunction, UserDefinedTable, Variables}};
use image::{Rgba, RgbaImage};
use eframe::{CreationContext, egui::{self, Color32, ColorImage, TextureOptions}};
// use egui_extras;

use crate::ifs::IfsHistogram;

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
            
           
            Ok(Box::<IfsApp>::new(IfsApp::new(cc, Default::default(), Default::default(), Default::default(), Default::default(), Default::default()))
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
    
    pub fn get_variables_table(&self) -> Variables{
        let owned_vec: Vec<_> = self
            .variables
            .iter()
            .map(|(name, re, im)| (name.as_str(), Complex::new(re.val, im.val)))
            .collect();
        Variables::from(&owned_vec)
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

fn max_threads() -> usize {
    std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1)
}

struct PreviewParams{
    frame: [TextboxData<u32>;2],
    num_iters: TextboxData<u64>,
    num_snapshot_iters: TextboxData<u64>,
    num_threads: TextboxData<usize>,
}

impl Default for PreviewParams{
    fn default() -> Self {
        PreviewParams {
            frame: [TextboxData::new(1000),TextboxData::new(1000)],
            num_iters: TextboxData::new(1000000),
            num_snapshot_iters: TextboxData::new(10000000),
            num_threads: TextboxData::new(max_threads()),
        }
    }
}

struct ExportParams{
    frame: [TextboxData<u32>;2],
    num_iters: TextboxData<u64>,
    num_threads: TextboxData<usize>,
    file_name: TextboxData<String>,
}

impl Default for ExportParams{
    fn default() -> Self {
        ExportParams { 
            frame: [TextboxData::new(1000),TextboxData::new(1000)],
            num_iters: TextboxData::new(1000000),
            num_threads: TextboxData::new(max_threads()),
            file_name: TextboxData::new("output.png".into()),
        }
    }
}

struct Flags{
    update_histogram_flag:bool,
    snapshot_flag:bool,
    redraw_flag:bool,
    disable_ui_flag:bool,
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
            disable_ui_flag:false,
            export_flag:false, 
            rendering_flag:false,
            parse_error_flag:false,
            reallocate_preveiw_historgram:true,
        }
    }
}



struct IfsApp {

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
    // export_histogram:IfsHistogram,

    preveiw_image: RgbaImage,
    texture_handle: egui::TextureHandle,

    job_thread: Option<thread::JoinHandle<()>>,
    job_sender: Option<mpsc::Sender<RenderJob>>,
    job_result_receiver: mpsc::Receiver<(Histogram, IfsHistogram)>,
}

impl IfsApp{
    pub fn new(
        cc: &CreationContext,
        functions_params:FunctionParams,
        transform_params:TransformParams,
        draw_params:DrawParams,
        preview_params:PreviewParams,
        export_params:ExportParams) -> Self{
        let functions_list = functions_params.parse_textbox();
        for func in functions_list.iter() { if func == "" { println!("whitespace!"); } };
        
        let preveiw_image = RgbaImage::new(1,1);

        let (job_sender, job_receiver) = mpsc::channel();
        let (job_result_sender, job_result_receiver) = mpsc::channel();
        let job_thread = Some(thread::spawn(move || job_thread(job_receiver, job_result_sender)));
        let job_sender = Some(job_sender);

        let mut user_funcs = UserDefinedTable::new();
        user_funcs.register("re", UserDefinedFunction::new("re",|z|z[0].re.into(),1));
        user_funcs.register("im", UserDefinedFunction::new("im",|z|z[0].im.into(),1));

        IfsApp {
            functions_params,
            transform_params,
            draw_params,
            preview_params,
            export_params,

            flags: Default::default(),

            functions_list,
            variables_table: Variables::new(),
            user_funcs,
            preveiw_image: preveiw_image.clone(),
            texture_handle: cc.egui_ctx.load_texture("preveiw", ColorImage::from_rgba_unmultiplied([1,1], &preveiw_image),TextureOptions::default()),

            preveiw_histogram: IfsHistogram::new([1,1]),
            // export_histogram: IfsHistogram::new([1,1]),
            
            job_thread,
            job_sender,
            job_result_receiver,
        }
    }

    fn ensure_ifs_compiles(&self) -> bool{
        let functions_list = &self.functions_list;
        let variables_table = &self.variables_table;
        let user_funcs = &self.user_funcs;
        for function in functions_list.iter(){
            match compile(function.as_str(), &["z"], variables_table, user_funcs){
                Ok(_) => {},
                Err(_) => return false,
            }
        }
        true
    }

    fn make_job(&mut self, which_histogram: Histogram) -> RenderJob {
        let (histogram, iters, num_threads) = match which_histogram {
            Histogram::Preview => (
                take(&mut self.preveiw_histogram),
                if self.flags.snapshot_flag {
                    self.preview_params.num_snapshot_iters.value
                } else {
                    self.preview_params.num_iters.value
                },
                self.preview_params.num_threads.value,
            ),
            Histogram::Export => (
                IfsHistogram::new([self.export_params.frame[0].value,self.export_params.frame[1].value]),
                self.export_params.num_iters.value,
                self.export_params.num_threads.value,
            ),
        };
        let functions_list = self.functions_list.clone();
        let variables_table = self.functions_params.get_variables_table();
        let user_funcs = self.user_funcs.clone();
        let weights = self.functions_params.get_weights();
        let rotate_scale = self.transform_params.get_rotate_scale();
        let translate = self.transform_params.get_translate();
        RenderJob {
            which_histogram,
            histogram,
            functions_list,
            variables_table,
            user_funcs,
            weights,
            rotate_scale,
            translate,
            iters,
            num_threads,
        }
    }
}

impl Drop for IfsApp {
    fn drop(&mut self) {
        // drop sender to terminate loop in job thread
        let _ = self.job_sender.take().unwrap();
        match self.job_thread.take().unwrap().join() {
            Ok(()) => {},
            Err(err) => {
                eprintln!("job thread panicked!");
                std::panic::resume_unwind(err)
            },
        }
    }
}

enum Histogram {
    Preview,
    Export,
}

impl eframe::App for IfsApp {
    
    
    
    
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        fn make_slider(ui: &mut egui::Ui, data: &mut SliderData, name: String, flags:Vec<&mut bool>){
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
                for flag in flags{
                        *flag = true; 
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

            if self.flags.disable_ui_flag { ui.disable();}
            
            ui.add(egui::Label::new("Transform"));
            make_slider(ui, &mut self.transform_params.translate[0], "translate x".to_string(), vec![&mut self.flags.update_histogram_flag]);
            make_slider(ui, &mut self.transform_params.translate[1], "translate y".to_string(), vec![&mut self.flags.update_histogram_flag]);
            make_slider(ui, &mut self.transform_params.scale, "scale".to_string() , vec![&mut self.flags.update_histogram_flag]);
            make_slider(ui, &mut self.transform_params.rotate, "rotate".to_string(), vec![&mut self.flags.update_histogram_flag]);

            ui.add(egui::Separator::default());


            ui.add(egui::Label::new("Variables"));
            for (name,mag_data,phase_data) in self.functions_params.variables.iter_mut(){

                make_slider(ui, mag_data, name.to_string() + ": magnitude", vec![&mut self.flags.update_histogram_flag]);
                make_slider(ui, phase_data, name.to_string() + ": phase          " , vec![&mut self.flags.update_histogram_flag]);
                let z = Complex::from_polar(mag_data.val, phase_data.val*TAU);
                self.variables_table.insert(&[(name.as_str(),z)]);
                ui.add(egui::Separator::default().shrink(20.0));
            }
            ui.add(egui::Separator::default());

            ui.add(egui::Label::new("weights"));
            for (index, weight) in self.functions_params.weights.iter_mut().enumerate(){
                make_slider(ui, weight, index.to_string(), vec![&mut self.flags.update_histogram_flag]);
            }
            ui.add(egui::Separator::default());

            ui.add(egui::Label::new("color"));
            
            make_slider(ui, &mut self.draw_params.gamma, "gamma".to_string(), vec![&mut self.flags.redraw_flag]);
            make_slider(ui, &mut self.draw_params.brightness, "brightness".to_string(),  vec![&mut self.flags.redraw_flag]);
            make_slider(ui, &mut self.draw_params.contrast, "contrast".to_string(),  vec![&mut self.flags.redraw_flag]);
            make_slider(ui, &mut self.draw_params.threshold, "threshold".to_string(),  vec![&mut self.flags.redraw_flag]);
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
            make_textbox(ui, &mut self.preview_params.num_threads, "threads".to_string(), vec![]);
            make_textbox(ui, &mut self.preview_params.num_snapshot_iters, "snapshot iters".to_string(),vec![]);
            if ui.button("snapshot").clicked() {
                self.flags.snapshot_flag = true;
                self.flags.update_histogram_flag = true;

            }
            

            ui.add(egui::Label::new("export settings"));
            make_textbox(ui, &mut self.export_params.num_iters, "iterations".to_string(), vec![]);
            make_textbox(ui, &mut self.export_params.num_threads, "threads".to_string(), vec![]);

            ui.horizontal(|ui|{
                ui.add(egui::Label::new("resolution"));
                make_textbox(ui, &mut self.export_params.frame[0], "".to_string(), vec![]);
                make_textbox(ui, &mut self.export_params.frame[1], "".to_string(), vec![]);
                
            });

            make_textbox(ui, &mut self.export_params.file_name, "export name".to_string(), vec![]);

            if ui.button("export").clicked() {
                self.flags.export_flag = true;
            }
            
            
        });

        egui::SidePanel::right("textbox").show(ctx, |ui|{
            ui.add(egui::TextEdit::multiline(&mut self.functions_params.text).desired_width(10000.0).desired_rows(50));
            ui.horizontal(|ui|{
                if ui.button("compile functions").clicked(){
                    self.functions_list = self.functions_params.parse_textbox();
                    self.flags.parse_error_flag = !self.ensure_ifs_compiles();
                    if !self.flags.parse_error_flag{ self.functions_params.weights = vec![SliderData::new(1.0, 0.0, 1.0);self.functions_list.len()];}
                }
                if self.flags.parse_error_flag { ui.add(egui::Label::new("COMPILE ERROR!")); };
            });
        });


        
        
        if self.flags.reallocate_preveiw_historgram{
            self.preveiw_histogram = IfsHistogram::new([self.preview_params.frame[0].value,self.preview_params.frame[1].value]);
            self.preveiw_image = RgbaImage::new(self.preview_params.frame[0].value, self.preview_params.frame[1].value);
            self.flags.reallocate_preveiw_historgram = false;
        }

       
        if !self.flags.rendering_flag {
            if self.flags.update_histogram_flag && !self.flags.parse_error_flag{
                let job = self.make_job(Histogram::Preview);
                self.job_sender.as_ref().unwrap().send(job).unwrap();
                if self.flags.snapshot_flag {self.flags.disable_ui_flag = true;}
                self.flags.rendering_flag = true;
                self.flags.update_histogram_flag = false;
                self.flags.snapshot_flag = false;
            }
            
            if self.flags.export_flag && !self.flags.parse_error_flag{
                let job = self.make_job(Histogram::Export);
                self.job_sender.as_ref().unwrap().send(job).unwrap();
                self.flags.rendering_flag = true;
                self.flags.export_flag = false;
                self.flags.disable_ui_flag = true;
            }
        }
        
        'job_recv: {
            if self.flags.rendering_flag {
                let (which_histogram, histogram) = match self.job_result_receiver.try_recv() {
                    Ok(histogram) => histogram,
                    Err(mpsc::TryRecvError::Empty) => break 'job_recv,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        eprintln!("job thread died? exiting!");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return;
                    },
                };
                self.flags.rendering_flag = false;
                self.flags.disable_ui_flag = false;
                match which_histogram {
                    Histogram::Preview => {
                        // only redraw if the preview is what was rendered
                        self.flags.redraw_flag = true;

                        // currently stored histogram should be default-constructed, so discard it
                        let _ = replace(&mut self.preveiw_histogram, histogram);
                    },
                    Histogram::Export => {
                        let mut export_image = image::RgbaImage::new(self.export_params.frame[0].value,self.export_params.frame[1].value);
                        histogram.write_to_image(
                            &mut export_image,
                            Rgba(self.draw_params.color.to_srgba_unmultiplied()),
                            Rgba(self.draw_params.bg_color.to_srgba_unmultiplied()),
                            self.draw_params.gamma.val,
                            self.draw_params.brightness.val,
                            self.draw_params.contrast.val,
                            self.draw_params.threshold.val,
                        );
                        match export_image.save(&self.export_params.file_name.value) {
                            Ok(()) => {},
                            Err(err) => eprintln!("couldn't save export image: {err}"),
                        }
                    },
                };
            }
        }

        if self.flags.redraw_flag && !self.flags.rendering_flag{
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
        
        let sized_texture = egui::load::SizedTexture::new(&self.texture_handle, egui::vec2(self.preview_params.frame[0].value as f32, self.preview_params.frame[1].value as f32));
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.image(sized_texture); 
            });
        });
    }
    
}

struct RenderJob {
    which_histogram: Histogram,
    histogram: IfsHistogram,
    functions_list: Vec<String>,
    variables_table: Variables,
    user_funcs: UserDefinedTable,
    weights: Vec<f32>,
    rotate_scale: Complex,
    translate: Complex,
    iters: u64,
    num_threads: usize,
}

fn job_thread(jobs: mpsc::Receiver<RenderJob>, job_results: mpsc::Sender<(Histogram, IfsHistogram)>) {
    for job in jobs.iter() {
        let RenderJob {
            which_histogram,
            mut histogram,
            functions_list,
            variables_table,
            user_funcs,
            weights,
            rotate_scale,
            translate,
            iters,
            num_threads,
        } = job;
        histogram.build_and_run_ifs(
            &functions_list,
            &variables_table,
            &user_funcs,
            weights,
            rotate_scale,
            translate,
            iters,
            num_threads,
        );
        job_results.send((which_histogram, histogram)).unwrap();
   }
}