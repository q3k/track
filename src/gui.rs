use glium::glutin::event_loop::{EventLoop};

const TITLE: &str = "q3k's audio bullshit";

fn lerp(a: f32, b: f32, v: f32) -> f32 {
    (b - a) * v + a
}

pub fn imgui_init(display: &glium::Display) -> (imgui_winit_support::WinitPlatform, imgui::Context) {
    let mut imgui_context = imgui::Context::create();
    imgui_context.set_ini_filename(None);

    let mut winit_platform = imgui_winit_support::WinitPlatform::init(&mut imgui_context);

    let gl_window = display.gl_window();
    let window = gl_window.window();

    let dpi_mode = imgui_winit_support::HiDpiMode::Default;

    winit_platform.attach_window(imgui_context.io_mut(), window, dpi_mode);

    let custom_ranges = imgui::FontGlyphRanges::from_slice(&[
        0x0020, 0xffff,
        0]); // this 0 is required to close the ranges list
    imgui_context.fonts().add_font(&[imgui::FontSource::TtfData {
        data: include_bytes!("../Terminus.ttf"),
        size_pixels: 14.0,
        config: Some(imgui::FontConfig {
            glyph_ranges: custom_ranges,
            size_pixels: 14.0,
            ..Default::default()
        }),
    }]);

    (winit_platform, imgui_context)
}

pub fn draw_sample(ui: &imgui::Ui, sample: &Vec<f32>) {
    let draw_list = ui.get_window_draw_list();

    // Origin
    let o = ui.cursor_screen_pos();

    let (x0, y0) = (o[0], o[1] + 5.0);
    let (width, height) = (400.0, 50.0);
    let (x1, y1) = (x0 + width, y0 + height);
    ui.dummy([width, height+10.0]);
    let c0 = [0.029, 0.029, 0.029];
    draw_list.add_rect_filled_multicolor([x0, y0], [x1, y1], c0, c0, c0, c0);

    let mut points = Vec::<mint::Vector2<f32>>::new();
    for x in 0..((x1-x0) as usize) {
        let xv = (x as f32) / ((x1 - x0) as f32);
        let s = lerp(0.0, sample.len() as f32, xv);
        let yv = (sample[s as usize] + 1.0) / 2.0;
        points.push(mint::Vector2 { x: lerp(x0, x1, xv), y: lerp(y1, y0, yv) } );
    }
    draw_list.add_polyline(points, [0.8, 0.8, 0.8]).filled(false).thickness(1.0).build();
}

pub fn create_window() -> (EventLoop<()>, glium::Display) {
    let event_loop = EventLoop::new();
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let builder = glium::glutin::window::WindowBuilder::new()
        .with_title(TITLE.to_owned())
        .with_inner_size(glium::glutin::dpi::LogicalSize::new(1024f64, 768f64));
    let display =
        glium::Display::new(builder, context, &event_loop).expect("Failed to initialize display");

    (event_loop, display)
}

pub struct Filepicker {
    path: std::path::PathBuf,
}

impl Filepicker {
    pub fn new() -> Self {
        let root = std::path::PathBuf::from("/");
        Self {
            path: root,
        }
    }
    pub fn draw(&mut self, ui: &imgui::Ui) -> Option<std::path::PathBuf> {
        let mut found: Option<std::path::PathBuf> = None;
        ui.window("Choose File").size([500.0, 300.0], imgui::Condition::Always).build(|| {
            if ui.button("/") {
                self.path = std::path::PathBuf::from("/");
            }
            ui.same_line();
            if ui.button("..") {
                self.path.pop();
            }
            ui.same_line();
            ui.text(format!("Path: {}", self.path.to_str().unwrap_or("unk")));

            let mut directories: Vec<(String, std::ffi::OsString)> = vec![];
            let mut files: Vec<(String, std::ffi::OsString)> = vec![];
            match std::fs::read_dir(&self.path) {
                Err(e) => {
                    ui.text(format!("Error: {}", e.to_string()));
                }
                Ok(l) => {
                    for f in l {
                        if let Ok(f) = f {
                            let str: String = f.file_name().to_str().unwrap_or("???").into();
                            if str.starts_with(".") {
                                continue
                            }
                            let os_str = f.file_name();
                            if let Ok(ftyp) = f.file_type() {
                                if ftyp.is_dir() {
                                    directories.push((format!("📁 {}", str), os_str));
                                } else if ftyp.is_file() {
                                    if str.to_ascii_lowercase().ends_with(".mod") {
                                        files.push((str, os_str));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            directories.sort();
            files.sort();
            if let Some(_) = ui.begin_table_header("Files", [imgui::TableColumnSetup::new("Name")]) {
                for (part, path) in directories.iter() {
                    ui.table_next_column();
                    ui.text(&part);
                    if ui.is_item_clicked() {
                        self.path.push(path);
                    }
                }
                for (part, path) in files.iter() {
                    ui.table_next_column();
                    ui.text(&part);
                    if ui.is_item_clicked() {
                        let mut full_path = self.path.clone();
                        full_path.push(path);
                        found = Some(full_path);
                    }
                }
            }
        });
        found
    }
}