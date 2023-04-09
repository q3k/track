use std::sync::{Arc, Mutex};

use glium::glutin::event::{Event, WindowEvent};
use glium::glutin::event_loop::{ControlFlow, EventLoop};
use glium::Surface;
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use winit::event::{ElementState};

mod promod;
mod notes;
mod sound;
mod gui;
mod input;

use sound::{Generator};

const TITLE: &str = "q3k's audio bullshit";

fn main() {
    env_logger::init_from_env( env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available");
    log::info!("Audio device: {}", device.name().unwrap_or("UNKNOWN".into()));
    let configs = device.supported_output_configs().expect("no output configs");
    let config = configs.filter(|c| c.channels() == 2 && c.max_sample_rate().0 >= 44100 && c.sample_format() == cpal::SampleFormat::I16).next();
    let config = config.expect("no good audio config").with_sample_rate(cpal::SampleRate(44100));
    log::info!("Audio output config: {:?}", config);

    let sr = config.sample_rate().0;

    let adsr = Arc::new(Mutex::new(sound::ADSRParams {
        a: 0.0,
        d: 0.2,
        s_level: 1.0,
        r: 0.1,
    }));
    let wk = Arc::new(Mutex::new(sound::WaveformKind::Sine));

    let mut k = input::Keyboard::new();
    let pk = input::PianoKeyboard::new();
    let poly = {
        let sr = sr.clone();
        let adsr = adsr.clone();
        let wk = wk.clone();
        sound::PolyphonicGenerator::new(move |note| {
            let adsr = adsr.lock().unwrap();
            let wk = wk.lock().unwrap();
            let w = wk.new(note.freq());
            let osc = sound::Oscillator::new(sr, w);
            let env = sound::ADSR::new(&adsr);
            osc.envelope(env, sr)
        })
    };
    let poly = Arc::new(Mutex::new(poly));

    let poly2 = poly.clone();
    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
            let mut env = poly2.lock().unwrap();
            for frame in data.chunks_mut(2) {
                let v = env.next();
                for sample in frame.iter_mut() {
                    *sample = (v * 32767.0) as i16;
                }
            }
        },
        move |err| {
            log::error!("Audio error: {:?}", err);
        },
        None
    ).unwrap();
    stream.play().unwrap();


    let (event_loop, display) = create_window();
    let (mut winit_platform, mut imgui_context) = gui::imgui_init(&display);

    let mut renderer = imgui_glium_renderer::Renderer::init(&mut imgui_context, &display)
        .expect("Failed to initialize renderer");

    let mut last_frame = std::time::Instant::now();

    let mut module: Option<promod::Module> = None;

    // Standard winit event loop
    event_loop.run(move |event, _, control_flow| match event {
        Event::NewEvents(_) => {
            let now = std::time::Instant::now();
            imgui_context.io_mut().update_delta_time(now - last_frame);
            last_frame = now;
        }
        Event::MainEventsCleared => {
            let gl_window = display.gl_window();
            winit_platform
                .prepare_frame(imgui_context.io_mut(), gl_window.window())
                .expect("Failed to prepare frame");
            gl_window.window().request_redraw();
        }
        Event::RedrawRequested(_) => {
            // Create frame for the all important `&imgui::Ui`
            let ui = imgui_context.frame();

            ui.window("ProMod").build(|| {
                if let Some(module_) = &module {
                    if ui.button(format!("Close {}", module_.title)) {
                        module = None;
                    }
                } else {
                    if ui.button(format!("Load...")) {
                        module = Some(promod::Module::load(std::path::Path::new("/home/q3k/Downloads/tempest-acidjazz.mod")).unwrap());
                    }
                }
            });

            ui.window("CrapSynth Params").size([300.0, 0.0], imgui::Condition::Appearing).build(|| {
                let mut wk = wk.lock().unwrap();
                {
                    let mut wk2 = wk.clone();
                    ui.radio_button("Sine", &mut wk2, sound::WaveformKind::Sine);
                    ui.radio_button("Square", &mut wk2, sound::WaveformKind::Square);
                    *wk = wk2;
                }

                let mut adsr = adsr.lock().unwrap();
                ui.slider("A", 0.0, 1.0, &mut adsr.a);
                ui.slider("D", 0.0, 1.0, &mut adsr.d);
                ui.slider("S", 0.0, 1.0, &mut adsr.s_level);
                ui.slider("R", 0.0, 1.0, &mut adsr.r);
            });

            ui.window("CrapSynth Channels").size([300.0, 300.0], imgui::Condition::Appearing).build(|| {
                let poly = poly.lock().unwrap();
                for (_, scope) in poly.scopes.iter() {
                    gui::draw_sample(ui, scope);
                }
            });

            if let Some(module) = &module {
                ui.window(format!("{} - Samples", module.title))
                .build(|| {
                    for (i, sample) in module.samples.iter().enumerate() {
                        let nbytes = sample.length * 2;
                        if imgui::CollapsingHeader::new(format!("{}: {}  ", i+1, sample.name)).default_open(nbytes != 0).build(ui) {
                            let volume = sample.volume;
                            let repeat = match sample.repeat_length {
                                0 | 1 => format!("no"),
                                l => format!("{} bytes from {}", l*2, sample.repeat_start*2),
                            };
                            ui.text(format!("Length: {} bytes, Volume: {}, Repeat: {}", nbytes, volume, repeat));
                        }
                    }
                });
            }

            let gl_window = display.gl_window();
            let mut target = display.draw();

            target.clear_color_srgb(0.05, 0.05, 0.05, 1.0);

            winit_platform.prepare_render(ui, gl_window.window());
            let draw_data = imgui_context.render();
            renderer
                .render(&mut target, draw_data)
                .expect("Rendering failed");
            target.finish().expect("Failed to swap buffers");
        }
        Event::WindowEvent {
            event: WindowEvent::KeyboardInput { input, .. },
            ..
        } => {
            if let Some(kc) = input.virtual_keycode {
                match input.state {
                    ElementState::Pressed => {
                        k.press(kc);
                    },
                    ElementState::Released => {
                        k.release(kc);
                    }
                }
            }
            let mut poly = poly.lock().unwrap();
            loop {
                let ev = k.drain();
                if ev.is_none() {
                    break
                }
                let ev = ev.unwrap();
                match ev {
                    input::KeyboardEvent::Down(kc) => {
                        if let Some(n) = pk.translate(&kc) {
                            poly.start(n);
                        }
                    }
                    input::KeyboardEvent::Up(kc) => {
                        if let Some(n) = pk.translate(&kc) {
                            poly.stop(n);
                        }
                    }
                }
            }
            drop(poly);
        }
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            *control_flow = ControlFlow::Exit;
        }
        event => {
            let gl_window = display.gl_window();
            winit_platform.handle_event(imgui_context.io_mut(), gl_window.window(), &event);
        }
    });
}

fn create_window() -> (EventLoop<()>, glium::Display) {
    let event_loop = EventLoop::new();
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let builder = glium::glutin::window::WindowBuilder::new()
        .with_title(TITLE.to_owned())
        .with_inner_size(glium::glutin::dpi::LogicalSize::new(1024f64, 768f64));
    let display =
        glium::Display::new(builder, context, &event_loop).expect("Failed to initialize display");

    (event_loop, display)
}