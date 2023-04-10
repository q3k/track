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