
pub mod evaluator;




use std::{f64::consts::TAU, str::FromStr};

use formulac::{compile, variable::{UserDefinedFunction, UserDefinedTable, Variables}};
use image::{ImageError, Rgba, RgbaImage};
use eframe::{CreationContext, egui::{self, Color32, ColorImage, TextureOptions, WidgetType::Image}};
use ifs_lang::{compile as ifs_compile, compiler::{TextCompiler, ValueFormatter}, lexer::Value as IfsValue, parse_module};
pub use ifs_lang::parser::Module as IfsModule;
use num_complex::ComplexFloat;

use crate::{evaluator::{EvaluatorThreadHandler, FormulacEvaluator, IfsEvaluator, IfsHistogram}};

pub type AResult<T = ()> = ifs_lang::AResult<T>;

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

#[derive(Default)]
struct FormulacFormatter;

impl ValueFormatter for FormulacFormatter {
    fn format(value: IfsValue) -> String {
        match value {
            IfsValue::Real(v) => format!("{v}"),
            IfsValue::Complex(Complex { re, im }) => format!("{re} * {im}i"),
        }
    }
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
"var a,b;
for n = 0,3{
func a*z + b*exp(i*n*TAU/3);
}"
            .to_string(),
            weights: vec![SliderData::new(1.0,0.0,1.0),SliderData::new(1.0,0.0,1.0),SliderData::new(1.0,0.0,1.0)],
            variables: vec![] }
    }
}
impl FunctionParams{
    pub fn compile_textbox_and_update(&mut self) -> AResult<IfsModule> {
        let module = parse_module(&self.text)?;
        let compiled = ifs_compile::<TextCompiler<FormulacFormatter>>(&module)?;
        self.variables = module.globals.clone().into_iter().map(|s| (s.to_string(),SliderData::default(),SliderData::default())).collect();
        self.weights = vec![SliderData::new(1.0,0.0,1.0);compiled.functions.len()];
        Ok(module)
    }

    pub fn get_weights(&self) -> Vec<f32>{
        return self.weights.iter().map(|w| w.val as f32).collect()
    }
    
    pub fn get_variables_table(&self) -> Vec<(String, Complex)>{
        let owned_vec: Vec<_> = self
            .variables
            .iter()
            .map(|(name, mag, phase)| (name.clone(), Complex::from_polar(mag.val, phase.val*TAU)))
            .collect();
        owned_vec
    }
}

struct DrawParams{
    color:egui::Rgba,
    bg_color:egui::Rgba, 
    gamma:SliderData,
    brightness:SliderData,
    contrast:SliderData,
    
  
}

impl DrawParams{
    pub fn get_color(&self) -> Rgba<u8>{
        Rgba::from(self.color.to_srgba_unmultiplied())
    }
    pub fn get_bg_color(&self) -> Rgba<u8>{
        Rgba::from(self.bg_color.to_srgba_unmultiplied())
    }
    pub fn get_gamma(&self) -> f64{
        self.gamma.val
    }
    pub fn get_brightness(&self) -> f64{
        self.brightness.val
    }
    pub fn get_contrast(&self) -> f64{
        self.contrast.val
    }
}
impl Default for DrawParams{
    fn default() -> Self {
        Self { 
            color: Color32::WHITE.into(),
            bg_color: Color32::BLACK.into(),
            gamma: SliderData::new(1.0, 0.0, 2.0),
            brightness: SliderData::new(0.0, 0.0, 1.0),
            contrast: SliderData::new(1.0, 0.0, 2.0),
        }
            
    }


}

struct PreviewParams{
    frame: [TextboxData<u32>;2],
    num_iters: TextboxData<u64>,
    num_snapshot_iters: TextboxData<u64>,
    num_threads: TextboxData<usize>,
}

impl PreviewParams{
    pub fn get_frame(&self) -> [usize;2]{
        [self.frame[0].value as usize, self.frame[1].value as usize]
    }
    pub fn get_num_iters(&self) -> u64{
        self.num_iters.value
    }
    pub fn get_num_snapshot_iters(&self) -> u64{
        self.num_snapshot_iters.value
    }
    pub fn get_num_threads(&self) -> usize{
        self.num_threads.value
    }
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
    frame: [TextboxData<usize>;2],
    num_iters: TextboxData<u64>,
    num_threads: TextboxData<usize>,
    file_name: TextboxData<String>,
}

impl Default for ExportParams{
    fn default() -> Self {
        ExportParams { 
            frame: [TextboxData::new(2000),TextboxData::new(2000)],
            num_iters: TextboxData::new(1000000),
            num_threads: TextboxData::new(max_threads()),
            file_name: TextboxData::new("output.png".into()),
        }
    }
}

impl ExportParams {
    fn get_frame(&self) -> [usize;2]{
        [self.frame[0].value,self.frame[1].value]
    }
    fn get_num_iters(&self) -> u64{
        self.num_iters.value
    }
    fn get_num_threads(&self) -> u64{
        self.num_iters.value
    }
    fn get_file_name(&self) -> String{
        self.file_name.value.clone()
    }
    
}

struct Flags{
    run_preveiw_evaluator:bool,
    update_preveiw_resolution:bool,
    update_preveiw_histogram:bool,
    update_evaluator_functions:bool,
    update_preveiw_image:bool,
    snapshot_preveiw:bool,

    run_export_evaluator:bool,
    update_export_resolution:bool,
    check_export_evaluator:bool,
    
    disable_ui_flag:bool,

    compile_error:bool,
   
    
}

impl Default for Flags{
    fn default() -> Self {
        Flags{

            run_preveiw_evaluator:true,
            update_preveiw_histogram:false,
            update_preveiw_resolution:false,
            update_preveiw_image:false,
            update_evaluator_functions:true,
            snapshot_preveiw:false,
            

            run_export_evaluator:false,
            update_export_resolution:false,
            check_export_evaluator:false,
            
            compile_error: false,
            disable_ui_flag:false,
           
        }
    }
}

fn max_threads() -> usize {
    std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1)
}



struct IfsApp {

    functions_params:FunctionParams,
    transform_params:TransformParams,
    draw_params:DrawParams,
    preview_params:PreviewParams,
    export_params:ExportParams,
    flags:Flags,

    
    variables_table: Variables,
    user_funcs: UserDefinedTable,

    preveiw_evaluator: evaluator::EvaluatorThreadHandler<FormulacEvaluator>,
    export_evaluator: evaluator::EvaluatorThreadHandler<FormulacEvaluator>,
    preveiw_histogram: IfsHistogram,
    // export_histogram:IfsHistogram,

    preveiw_image: RgbaImage,
    texture_handle: egui::TextureHandle,

 
}

impl IfsApp{
    pub fn new(
        cc: &CreationContext,
        functions_params:FunctionParams,
        transform_params:TransformParams,
        draw_params:DrawParams,
        preview_params:PreviewParams,
        export_params:ExportParams) -> Self{
        
        
        let frame = preview_params.get_frame();
        let export_frame = export_params.get_frame();
        let preveiw_image = RgbaImage::new(frame[0] as u32, frame[1] as u32);


        let mut user_funcs = UserDefinedTable::new();
        user_funcs.register("re", UserDefinedFunction::new("re",|z|z[0].re.into(),1));
        user_funcs.register("im", UserDefinedFunction::new("im",|z|z[0].im.into(),1));
        user_funcs.register("ar", UserDefinedFunction::new("ar",|z|if z[0].arg().is_nan() {0.0.into()} else {z[0].arg().into()},1));
        user_funcs.register("proj", UserDefinedFunction::new("proj",|args|{
            let z = args[0];
            let w = args[1];
            return ((z.re*w.re + z.im*w.im)/(z.abs().powf(2.0))) * w;
        } ,2));
        IfsApp {
            functions_params,
            transform_params,
            draw_params,
            preview_params,
            export_params,

            flags: Default::default(),

           
            variables_table: Variables::new(),
            user_funcs: user_funcs.clone(),
            preveiw_image: preveiw_image.clone(),
            texture_handle: cc.egui_ctx.load_texture("preveiw", ColorImage::from_rgba_unmultiplied(frame, &preveiw_image),TextureOptions::default()),

            preveiw_histogram: IfsHistogram::new(frame),
            
            
           preveiw_evaluator: EvaluatorThreadHandler::new(FormulacEvaluator::new(frame, user_funcs.clone())),
           export_evaluator: EvaluatorThreadHandler::new(FormulacEvaluator::new(export_frame, user_funcs.clone())),
        }
    }

    pub fn get_preview_evaluation_params(&self) -> evaluator::EvaluationParams{
        let num_iters = if self.flags.snapshot_preveiw == true {self.preview_params.get_num_snapshot_iters()} else {self.preview_params.get_num_iters()} ; 
        evaluator::EvaluationParams{
            weights:  self.functions_params.get_weights(),
            variables: self.functions_params.get_variables_table(),
            translate: self.transform_params.get_translate(),
            rotate_scale: self.transform_params.get_rotate_scale(),
            num_iters,
        }

    }
    pub fn get_export_evaluation_params(&self) -> evaluator::EvaluationParams{
        
        evaluator::EvaluationParams{
            weights:  self.functions_params.get_weights(),
            variables: self.functions_params.get_variables_table(),
            translate: self.transform_params.get_translate(),
            rotate_scale: self.transform_params.get_rotate_scale(),
            num_iters: self.export_params.get_num_iters(),
        }

    }


    pub fn get_draw_params(&self) -> evaluator::DrawParams{

        evaluator::DrawParams{
            color: self.draw_params.get_color(),
            bgcolor: self.draw_params.get_bg_color(),
            gamma: self.draw_params.get_gamma(),
            brightnesss: self.draw_params.get_brightness(),
            contrast: self.draw_params.get_contrast()
        }

    }
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

        egui::SidePanel::right("textbox").show(ctx, |ui|{
            ui.add(egui::TextEdit::multiline(&mut self.functions_params.text).desired_width(10000.0).desired_rows(50));
            ui.horizontal(|ui|{
                if ui.button("compile functions").clicked(){
                    self.flags.update_evaluator_functions = true;
                }
                if self.flags.compile_error {
                    ui.add(egui::Label::new("COMPILE ERROR!"));
                };
            });
        });

        egui::SidePanel::left("test_side_panel").show(ctx,|ui|{
            egui::ScrollArea::vertical().show(ui, |ui| {

                if self.flags.disable_ui_flag { ui.disable();}
                
                ui.add(egui::Label::new("Transform"));
                make_slider(ui, &mut self.transform_params.translate[0], "translate x".to_string(), vec![&mut self.flags.run_preveiw_evaluator]);
                make_slider(ui, &mut self.transform_params.translate[1], "translate y".to_string(), vec![&mut self.flags.run_preveiw_evaluator]);
                make_slider(ui, &mut self.transform_params.scale, "scale".to_string() , vec![&mut self.flags.run_preveiw_evaluator]);
                make_slider(ui, &mut self.transform_params.rotate, "rotate".to_string(), vec![&mut self.flags.run_preveiw_evaluator]);

                ui.add(egui::Separator::default());


                ui.add(egui::Label::new("Variables"));
                for (name,mag_data,phase_data) in self.functions_params.variables.iter_mut(){

                    make_slider(ui, mag_data, name.to_string() + ": magnitude", vec![&mut self.flags.run_preveiw_evaluator]);
                    make_slider(ui, phase_data, name.to_string() + ": phase          " , vec![&mut self.flags.run_preveiw_evaluator]);
                    let z = Complex::from_polar(mag_data.val, phase_data.val*TAU);
                    self.variables_table.insert(&[(name.as_str(),z)]);
                    ui.add(egui::Separator::default().shrink(20.0));
                }
                ui.add(egui::Separator::default());
                
                ui.add(egui::Label::new("weights"));
                egui::ScrollArea::vertical().max_height(500.0).show(ui, |ui| {
                for (index, weight) in self.functions_params.weights.iter_mut().enumerate(){
                    make_slider(ui, weight, index.to_string(), vec![&mut self.flags.run_preveiw_evaluator]);
                }
                });
                
                ui.add(egui::Separator::default());

                ui.add(egui::Label::new("color"));
                
                make_slider(ui, &mut self.draw_params.gamma, "gamma".to_string(), vec![&mut self.flags.update_preveiw_image]);
                make_slider(ui, &mut self.draw_params.brightness, "brightness".to_string(),  vec![&mut self.flags.update_preveiw_image]);
                make_slider(ui, &mut self.draw_params.contrast, "contrast".to_string(),  vec![&mut self.flags.update_preveiw_image]);
              
                if egui::color_picker::color_edit_button_rgba(ui, &mut self.draw_params.color, egui::color_picker::Alpha::BlendOrAdditive).changed(){ self.flags.update_preveiw_image = true;};
                if egui::color_picker::color_edit_button_rgba(ui, &mut self.draw_params.bg_color, egui::color_picker::Alpha::BlendOrAdditive).changed(){ self.flags.update_preveiw_image = true;};
                ui.add(egui::Separator::default());
                
                ui.add(egui::Label::new("preveiw settings"));
                ui.horizontal(|ui|{
                    ui.add(egui::Label::new("resolution"));
                    make_textbox(ui, &mut self.preview_params.frame[0], "".to_string(), vec![&mut self.flags.update_preveiw_resolution,&mut self.flags.update_preveiw_histogram,&mut self.flags.update_preveiw_image]);
                    make_textbox(ui, &mut self.preview_params.frame[1], "".to_string(), vec![&mut self.flags.update_preveiw_resolution,&mut self.flags.update_preveiw_histogram,&mut self.flags.update_preveiw_image]);
                    
                });
                make_textbox(ui, &mut self.preview_params.num_iters, "iterations".to_string(), vec![&mut self.flags.run_preveiw_evaluator]);
                make_textbox(ui, &mut self.preview_params.num_threads, "threads".to_string(), vec![]);
                make_textbox(ui, &mut self.preview_params.num_snapshot_iters, "snapshot iters".to_string(),vec![]);
                if ui.button("snapshot").clicked() {
                    self.flags.snapshot_preveiw = true;
                    self.flags.run_preveiw_evaluator = true;

                }
                
                ui.add(egui::Label::new("export settings"));
                make_textbox(ui, &mut self.export_params.num_iters, "iterations".to_string(), vec![]);
                make_textbox(ui, &mut self.export_params.num_threads, "threads".to_string(), vec![]);

                ui.horizontal(|ui|{
                    ui.add(egui::Label::new("resolution"));
                    make_textbox(ui, &mut self.export_params.frame[0], "".to_string(), vec![&mut self.flags.update_export_resolution]);
                    make_textbox(ui, &mut self.export_params.frame[1], "".to_string(), vec![&mut self.flags.update_export_resolution]);
                    
                });

                make_textbox(ui, &mut self.export_params.file_name, "export name".to_string(), vec![]);

                if ui.button("export").clicked() {
                    self.flags.run_export_evaluator = true;
                }
                
                
            });
        });

        if self.flags.update_preveiw_resolution{
            self.preveiw_histogram = IfsHistogram::new(self.preview_params.get_frame());
            self.preveiw_evaluator = evaluator::EvaluatorThreadHandler::new(evaluator::FormulacEvaluator::new(self.preview_params.get_frame(),self.user_funcs.clone()));
            self.flags.update_preveiw_resolution = false;
        }
        if self.flags.update_export_resolution{
            self.export_evaluator = evaluator::EvaluatorThreadHandler::new(evaluator::FormulacEvaluator::new(self.export_params.get_frame(),self.user_funcs.clone()));
            self.flags.update_export_resolution = false;
        }
        if self.flags.update_evaluator_functions{
                self.flags.update_evaluator_functions = false;
                let module = self.functions_params.compile_textbox_and_update();
                let preveiw_evaluator = self.preveiw_evaluator.try_get_evaluator();
                let export_evaluator = self.export_evaluator.try_get_evaluator();
                match (module, preveiw_evaluator,export_evaluator){
                    ( Ok(module), Some(preveiw_evaluator), Some(export_evaluator) ) => {
                        let preveiw_result = preveiw_evaluator.lock().expect("poisoned").set_ifs(module);
                        let module = self.functions_params.compile_textbox_and_update().expect("module failed to compile second time!?!?!?!?");
                        let export_result =export_evaluator.lock().expect("poisoned").set_ifs(module);
                        match (preveiw_result,export_result){
                            (Ok(_),Ok(_)) => {self.flags.run_preveiw_evaluator = true; self.flags.compile_error = false},
                            (Err(_),_) => self.flags.compile_error = true,
                            (_,Err(_)) => self.flags.compile_error = true,
                        }
                    },
                    (Err(_),_,_) => {
                        self.flags.compile_error = true
                    }
                    (_,None,_) | (_,_,None) => ()
                }
        }
        if self.flags.run_preveiw_evaluator{
            let extra_params = evaluator::FormulacParams{num_threads: self.preview_params.get_num_threads()};
            match self.preveiw_evaluator.try_evaluate_async(self.get_preview_evaluation_params(),extra_params){
                Err(_) => (),
                Ok(_) => {
                    self.flags.run_preveiw_evaluator = false;
                    self.flags.update_preveiw_histogram = true;
                    if self.flags.snapshot_preveiw == true{println!("snapshoting");self.flags.disable_ui_flag = true}
                }
            }
        }
        if self.preveiw_evaluator.check_eval(){
            match self.preveiw_evaluator.try_get_evaluator(){
                Some(eval) =>{
                    self.preveiw_histogram = eval.lock().expect("poisoned").get_histogram().clone();
                    self.flags.update_preveiw_histogram = false;
                    self.flags.update_preveiw_image = true;
                    self.flags.snapshot_preveiw = false;
                    self.flags.disable_ui_flag = false;
                }
                None => ()
            }
        }

        if self.flags.update_preveiw_image{
            let draw_params = self.get_draw_params();
            self.preveiw_histogram.write_to_image(&mut self.preveiw_image, draw_params);
            let color_image = ColorImage::from_rgba_unmultiplied([self.preveiw_image.height() as usize, self.preveiw_image.width() as usize], &self.preveiw_image);
            self.texture_handle.set(color_image, TextureOptions::default());
            self.flags.update_preveiw_image = false;
           
        }

        if self.flags.run_export_evaluator{
            //self.export_evaluator = evaluator::EvaluatorThreadHandler::new(FormulacEvaluator::new(self.export_params.get_frame(),self.user_funcs.clone()));
            let export_params = self.get_export_evaluation_params();
            let extra_params = evaluator::FormulacParams{num_threads: self.preview_params.get_num_threads()};
            
            match self.export_evaluator.try_evaluate_async(export_params, extra_params){
                Err(_) => println!("task in progress"),
                Ok(_) => {
                    self.flags.disable_ui_flag = true;
                    self.flags.run_export_evaluator = false;
                    self.flags.check_export_evaluator = true;
                }
            }
        }
        
        if self.export_evaluator.check_eval(){
            match self.export_evaluator.try_get_evaluator(){
                Some(eval) =>{
                    println!("trying to export");
                    let export_histogram = eval.lock().expect("poisoned").get_histogram().clone();
                    let export_frame = self.export_params.get_frame();
                    let draw_params = self.get_draw_params();
                    let mut image = RgbaImage::new(export_frame[0] as u32,export_frame[1] as u32);
                    export_histogram.write_to_image(&mut image, draw_params);
                    let _r = image.save("output.png");
                    
                    self.flags.check_export_evaluator = false;
                    self.flags.disable_ui_flag = false;
                    
                }
                None => ()
            }
        }
        

        
        let sized_texture = egui::load::SizedTexture::new(&self.texture_handle, egui::vec2(self.preview_params.frame[0].value as f32, self.preview_params.frame[1].value as f32));
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.image(sized_texture); 
            });
        });
    }
    
}

