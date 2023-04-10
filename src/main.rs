use std::sync::{Arc, Mutex};

use glium::glutin::event::{Event, WindowEvent};
use glium::glutin::event_loop::{ControlFlow};
use glium::Surface;
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use imgui_glium_renderer::Renderer;
use imgui_winit_support::WinitPlatform;
use winit::event::{ElementState};
use imgui::Condition::Appearing;

mod promod;
mod notes;
mod sound;
mod synth;
mod gui;
mod input;
mod dsp;

use sound::{Generator};


struct Synthesizer {
    adsr_params: sound::ADSRParams,
    waveform_kind: synth::WaveformKind,
}

impl Synthesizer {
    fn new() -> Self {
        Self {
            adsr_params: sound::ADSRParams {
                a: 0.0,
                d: 0.2,
                s_level: 1.0,
                r: 0.1,
            },
            waveform_kind: synth::WaveformKind::Sine,
        }
    }

    fn imgui_draw(&mut self, ui: &imgui::Ui) {
        if imgui::CollapsingHeader::new("Synthesizer Options").default_open(true).build(ui) {
            ui.radio_button("Sine", &mut self.waveform_kind, synth::WaveformKind::Sine);
            ui.same_line();
            ui.radio_button("Square", &mut self.waveform_kind, synth::WaveformKind::Square);

            ui.slider("A", 0.0, 1.0, &mut self.adsr_params.a);
            ui.slider("D", 0.0, 1.0, &mut self.adsr_params.d);
            ui.slider("S", 0.0, 1.0, &mut self.adsr_params.s_level);
            ui.slider("R", 0.0, 1.0, &mut self.adsr_params.r);
        }
    }
}

struct Tracker {
    player: Option<promod::Player>,
    sample_rate: u32,

    selected_pattern: usize,
}

impl Tracker {
    fn new(sample_rate: u32,) -> Self {
        Self {
            player: None,
            sample_rate,

            selected_pattern: 0,
        }
    }
    fn imgui_draw_main_window(&mut self, ui: &imgui::Ui) {
        if imgui::CollapsingHeader::new("Tracker").default_open(true).build(ui) {
            if let Some(_) = &self.player{
                if ui.button("Close") {
                    self.player = None;
                }
            } else {
                if ui.button(format!("Load...")) {
                    let m = Arc::new(promod::Module::load(std::path::Path::new("/home/q3k/Downloads/h0ffman_-_eon.mod")).unwrap());
                    self.player = Some(promod::Player::new(&m, self.sample_rate as f32));
                }
            }
            if let Some(p) = &mut self.player{
                if p.playing {
                    ui.same_line();
                    if ui.button("Pause") {
                        p.playing = false;
                    }
                    ui.same_line();
                    if ui.button("Stop") {
                        p.playing = false;
                        p.row = 0;
                        p.program = 0;
                        p.pattern = 0;
                    }
                } else {
                    ui.same_line();
                    if ui.button("Play") {
                        p.playing = true
                    }
                }
            }
        }
    }
    fn imgui_draw(&mut self, ui: &imgui::Ui) {
        if let Some(player) = &self.player {
            let module = &player.module;
            ui.window(format!("{} - Samples", module.title)).size([440.0, 900.0], Appearing).position([0.0, 100.0], Appearing)
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
                        let id = ui.push_id(format!("sample {}", i));
                        gui::draw_sample(ui, &sample.data);
                        if ui.button("Play") {
                            //*sp = Some(sample.clone());
                        }
                        id.end();
                    }
                }
            });

            ui.window(format!("{} - Patterns", module.title)).size([600.0, 1300.0], Appearing).position([500.0, 0.0], Appearing).build(|| {
                let items = (0..module.patterns.len()).collect::<Vec<usize>>();
                let cur_row = player.row;
                if let Some(_) = ui.begin_combo("Pattern", format!("{}", self.selected_pattern)) {
                    for cur in &items {
                        if self.selected_pattern == *cur {
                            ui.set_item_default_focus();
                        }
                        let clicked = ui.selectable_config(format!("{}", cur))
                            .selected(self.selected_pattern == *cur)
                            .build();
                        if clicked {
                            self.selected_pattern = *cur;
                        }
                    }
                }
                if self.selected_pattern < module.patterns.len() {
                    for (i, row) in module.patterns[self.selected_pattern].rows.iter().enumerate() {
                        let mut cur = "  ";
                        if cur_row == i {
                            cur = "->";
                        }
                        let row_data: Vec<String> = row.channels.iter().map(|c| {
                            let note = c.snote();
                            let sn = c.sample_number();
                            let sample = if sn == 0 {
                                format!("..")
                            } else if sn < 16 {
                                format!(".{:X}", sn)
                            } else {
                                format!("{:02X}", sn)
                            };
                            format!("{}{}....",  note, sample)
                        }).collect();
                        ui.text(format!("{}{:02x} | {}", cur, i, row_data.join(" | ")))
                    }
                }
            });
        }
    }
}

#[derive(PartialEq,Eq,Clone,Copy)]
enum LiveSoundSource {
    Module,
    Synthesizer,
}

struct AudioSink {
    poly: sound::PolyphonicGenerator,
    tracker: Tracker,
    config: cpal::SupportedStreamConfig,
    device: cpal::Device,
}

impl AudioSink {
    fn new() -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("no output device available");
        log::info!("Audio device: {}", device.name().unwrap_or("UNKNOWN".into()));
        let configs = device.supported_output_configs().expect("no output configs");
        let config = configs.filter(|c| c.channels() == 2 && c.max_sample_rate().0 >= 44100 && c.sample_format() == cpal::SampleFormat::I16).next();
        let config = config.expect("no good audio config").with_sample_rate(cpal::SampleRate(44100));
        log::info!("Audio output config: {:?}", config);

        Self {
            poly: sound::PolyphonicGenerator::new(),
            tracker: Tracker::new(config.sample_rate().0),
            config,
            device,
        }
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate().0
    }

    fn channels(&self) -> usize {
        self.config.channels() as usize
    }

    fn fill_sound_buffer(&mut self, data: &mut [i16], _info: &cpal::OutputCallbackInfo) {
        for frame in data.chunks_mut(self.channels()) {
            let v_p = self.poly.next();
            let v_t = self.tracker.player.as_mut().map(|p| p.next()).unwrap_or(0.0);

            let v = v_p + v_t;
            for sample in frame.iter_mut() {
                *sample = (v * 32767.0) as i16;
            }
        }
    }
}
struct Application {
    keyboard: input::Keyboard,
    piano_keyboard: input::PianoKeyboard,
    synthesizer: Synthesizer,
    live_sound_source: LiveSoundSource,

    audio_sink: Arc<Mutex<AudioSink>>,

    last_frame: std::time::Instant,
}

struct EventLoopContext<'a> {
    imgui_context: &'a mut imgui::Context,
    winit_platform: &'a mut WinitPlatform,
    display: &'a glium::Display,
    renderer: &'a mut Renderer,
}

impl Application {
    fn new() -> Self {
        Self {
            keyboard: input::Keyboard::new(),
            piano_keyboard: input::PianoKeyboard::new(),
            synthesizer: Synthesizer::new(),
            live_sound_source: LiveSoundSource::Synthesizer,

            audio_sink: Arc::new(Mutex::new(AudioSink::new())),

            last_frame: std::time::Instant::now(),
        }
    }

    fn audio_stream(&self) -> cpal::Stream {
        let s = self.audio_sink.lock().unwrap();
        let config = s.config.clone();
        let audio_sink = self.audio_sink.clone();
        let stream = s.device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], info: &cpal::OutputCallbackInfo| {
                let mut audio_sink = audio_sink.lock().unwrap();
                audio_sink.fill_sound_buffer(data, info);
            },
            move |err| {
                log::error!("Audio error: {:?}", err);
            },
            None
        ).unwrap();
        stream
    }

    fn run(mut self) {
        let (event_loop, display) = gui::create_window();
        let (mut winit_platform, mut imgui_context) = gui::imgui_init(&display);

        let mut renderer = imgui_glium_renderer::Renderer::init(&mut imgui_context, &display)
            .expect("Failed to initialize renderer");

        event_loop.run(move |event, _, control_flow| {
            let ctx = EventLoopContext {
                imgui_context: &mut imgui_context,
                winit_platform: &mut winit_platform,
                display: &display,
                renderer: &mut renderer,
            };
            self.on_event(event, control_flow, ctx);
        });
    }

    fn on_event<'a, T>(&mut self, event: winit::event::Event<'a, T>, control_flow: &mut winit::event_loop::ControlFlow, ctx: EventLoopContext<'a>) {
        match event {
            Event::NewEvents(_) => {
                let now = std::time::Instant::now();
                ctx.imgui_context.io_mut().update_delta_time(now - self.last_frame);
                self.last_frame = now;
            }
            Event::MainEventsCleared => {
                let gl_window = ctx.display.gl_window();
                ctx.winit_platform
                    .prepare_frame(ctx.imgui_context.io_mut(), gl_window.window())
                    .expect("Failed to prepare frame");
                gl_window.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                let ui = ctx.imgui_context.frame();
                self.imgui_draw(ui);

                let gl_window = ctx.display.gl_window();
                let mut target = ctx.display.draw();

                target.clear_color_srgb(0.05, 0.05, 0.05, 1.0);

                ctx.winit_platform.prepare_render(ui, gl_window.window());
                let draw_data = ctx.imgui_context.render();
                ctx.renderer
                    .render(&mut target, draw_data)
                    .expect("Rendering failed");
                target.finish().expect("Failed to swap buffers");
            },
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                if let Some(kc) = input.virtual_keycode {
                    match input.state {
                        ElementState::Pressed => {
                            self.keyboard.press(kc);
                        },
                        ElementState::Released => {
                            self.keyboard.release(kc);
                        }
                    }
                }
                let mut sink = self.audio_sink.lock().unwrap();
                match self.live_sound_source {
                    LiveSoundSource::Module => (),
                    LiveSoundSource::Synthesizer => {
                        let wk = self.synthesizer.waveform_kind.clone();
                        let sr = sink.sample_rate();
                        let params = self.synthesizer.adsr_params.clone();
                        sink.poly.set_notegen(Box::new(move |note| {
                            let osc = synth::Oscillator::new(sr, wk.new(note.freq()));
                            let envelope = sound::ADSR::new(&params);
                            Box::new(sound::envelope(osc, envelope, sr))
                        }));
                    },
                }

                loop {
                    let ev = self.keyboard.drain();
                    if ev.is_none() {
                        break
                    }
                    let ev = ev.unwrap();
                    match ev {
                        input::KeyboardEvent::Down(kc) => {
                            if let Some(n) = self.piano_keyboard.translate(&kc) {
                                sink.poly.start(n);
                            }
                        }
                        input::KeyboardEvent::Up(kc) => {
                            if let Some(n) = self.piano_keyboard.translate(&kc) {
                                sink.poly.stop(n);
                            }
                        }
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            event => {
                let gl_window = ctx.display.gl_window();
                ctx.winit_platform.handle_event(ctx.imgui_context.io_mut(), gl_window.window(), &event);
            }
        }
    }

    fn imgui_draw(&mut self, ui: &imgui::Ui) {
        let mut sink = self.audio_sink.lock().unwrap();
        ui.window("toysynth").size([300.0, 300.0], Appearing).position([0.0, 20.0], Appearing).collapsed(false, Appearing).build(|| {
            ui.text("Live Play");
            ui.radio_button("Synthesizer", &mut self.live_sound_source, LiveSoundSource::Synthesizer);
            ui.same_line();
            ui.radio_button("Module Sample", &mut self.live_sound_source, LiveSoundSource::Module);
            self.synthesizer.imgui_draw(ui);
            sink.tracker.imgui_draw_main_window(ui);
        });
        sink.tracker.imgui_draw(ui);
    }
}

fn main() {
    env_logger::init_from_env( env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));

    let app = Application::new();
    let stream = app.audio_stream();
    stream.play().unwrap();

    app.run();
}