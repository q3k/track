use std::io::Read;
use std::sync::Arc;

use byteorder::{BigEndian, ReadBytesExt};

use crate::{notes, sound, sound::{Enveloped}};
use crate::dsp::{Signal, Interpolator};

#[derive(Debug)]
pub enum Error {
    IOError(std::io::Error),
    ParseError(&'static str),
    SampleError {
        sample: usize,
        inner: Box<Error>,
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Module {
    pub title: String,

    pub samples: Vec<Arc<Sample>>,

    pub patterns: Vec<Pattern>,

    pub program: Vec<u8>,
}

impl Module {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let mut f = std::fs::File::open(path)?;

        let mut title = vec![0u8; 20];
        f.read_exact(&mut title)?;
        let title = std::str::from_utf8(&title).or(Err(Error::ParseError("invalid title")))?.trim_end_matches(char::from(0));

        let mut samples = (0..31)
            .map(|i| {
                Sample::parse_header(&mut f)
                    .map_err(|e| {
                        Error::SampleError { sample: i, inner: e.into() }
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        let _npos = f.read_u8()?;
        let _unused = f.read_u8()?;

        let mut ptable = vec![0u8; 128];
        f.read_exact(&mut ptable)?;
        let ptable: Vec<u8> = Vec::from(ptable);

        let mut signature = vec![0u8; 4];
        f.read_exact(&mut signature)?;

        let npatterns = ptable.iter().max().unwrap() + 1;
        let mut patterns: Vec<Pattern> = vec![];
        for _ in 0..npatterns {
            let mut pattern = Pattern {
                rows: vec![],
            };

            for _rid in 0..64 {
                let mut row = Row {
                    channels: vec![],
                };
                for _cid in 0..4 {
                    let cell = f.read_u32::<BigEndian>()?;
                    row.channels.push(Data(cell));
                }
                pattern.rows.push(row);
            }
            patterns.push(pattern);
        }

        for (i, sample) in samples.iter_mut().enumerate() {
            let mut data: Vec<i8> = vec![];
            for _ in 0..(sample.data.len()) {
                let v = f.read_i8().map_err(|e| {
                    Error::SampleError { sample: i, inner: Box::new(e.into()) }
                })?;
                data.push(v);
            }
            sample.set_data(data);
        }

        Ok(Self {
            title: title.into(),
            samples: samples.into_iter().map(Arc::new).collect(),
            patterns,
            program: ptable,
        })
    }
}

#[derive(Debug)]
pub struct Pattern {
    pub rows: Vec<Row>,
}

#[derive(Debug)]
pub struct Row {
    pub channels: Vec<Data>,
}


#[derive(Debug)]
pub struct Data(u32);

impl Data {
    pub fn sample_number(&self) -> u8 {
        let hi = (self.0 >> 28) & 0xF;
        let lo = (self.0 >> 12) & 0xF;
        return ((hi << 4) | lo) as u8;
    }
    pub fn period(&self) -> u16 {
        ((self.0 >> 16) & 0xfff) as u16
    }
    pub fn snote(&self) -> String {
        let mut period = self.period();
        let mut oct = 1;
        if period == 0 {
            return "...".into()
        }
        if period  > 856 {
            period /= 2;
            oct = 0;
        } else if period < 113 {
            period *= 8;
            oct = 4;
        } else if period < 226 {
            period *= 4;
            oct = 3;
        } else if period < 453 {
            period *= 2;
            oct = 2;
        }
        let mul = 856.0f32 / (period as f32);
        let hs = (mul.log(1.0594630943592953f32) + 0.5).floor() as usize;
        let notes: [&'static str; 12] = [
            "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
        ];
        return format!("{}{}", notes[hs], oct+2);
    }
    pub fn effect(&self) -> Effect {
        Effect::from((self.0 & 0xfff) as u16)
    }
    pub fn note(&self) -> notes::Note {
        let period = self.period();
        let freq = (440.0f32 * 254.0f32) / (period as f32);
        notes::Note::new(freq)
    }
}

#[derive(Debug)]
pub enum Effect {
    Unknown,
    VolumeSlide {
        up: u8,
        down: u8,
    },
    SetVolume {
        volume: u16,
    },
    PatternBreak {
        division: usize,
    },
    FineVolumeSlideUp {
        up: u8,
    },
    FineVolumeSlideDown {
        down: u8,
    },
    SetTicksPerDivision {
        tpd: u16,
    },
    SetBeatsPerMinute {
        bpm: u16,
    }
}

impl Effect {
    pub fn from(v: u16) -> Self {
        let a = (v >> 8) & 0xf;
        let b = (v >> 4) & 0xf;
        let c = (v >> 0) & 0xf;
        let mut z = b * 16 + c;
        match a {
            0xa => Effect::VolumeSlide { up: b as u8, down: c as u8 },
            0xc => Effect::SetVolume { volume: z, },
            0xd => Effect::PatternBreak { division: (b * 10 + c) as usize, },
            0xe => match b {
                0xa => Effect::FineVolumeSlideUp {
                    up: c as u8,
                },
                0xb => Effect::FineVolumeSlideDown {
                    down: c as u8,
                },
                _ => Effect::Unknown,
            },
            0xf => {
                if z == 0 {
                    z = 1;
                }
                if z <= 32 {
                    Effect::SetTicksPerDivision { tpd: z }
                } else {
                    Effect::SetBeatsPerMinute { bpm: z }
                }
            },
            _ => Effect::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct Sample {
    pub name: String,
    pub length: usize,
    pub finetune: u8,
    pub volume: u8,
    pub repeat_start: usize,
    pub repeat_length: usize,

    pub data: Vec<f32>,
}

impl Sample {
    fn parse_header<T: std::io::Read>(reader: &mut T) -> Result<Self> {
        let mut name = vec![0u8; 22];
        reader.read_exact(&mut name)?;
        let name = std::str::from_utf8(&name).or(Err(Error::ParseError("invalid name")))?.trim_end_matches(char::from(0));

        let length = reader.read_u16::<BigEndian>()? as usize;
        let finetune = reader.read_u8()?;
        let volume = reader.read_u8()?;
        let repeat_start = reader.read_u16::<BigEndian>()? as usize;
        let repeat_length = reader.read_u16::<BigEndian>()? as usize;
        Ok(Self {
            name: name.into(),
            length, finetune, volume, repeat_start, repeat_length,
            data: vec![0.0f32; length * 2],
        })
    }

    fn set_data(&mut self, data: Vec<i8>) {
        let converted = data.convert::<f32>();
        self.data = converted.iter().collect();
    }

    pub fn play(self: Arc<Self>, note: notes::Note, sample_rate: u32) -> SamplePlayback<Interpolator<Arc<Self>>> {
        let diff = notes::A4.freq() / note.freq();
        let from = (7093789.2f32 / (4.0f32 * 127.0f32)) / diff;
        let to = sample_rate as f32;
        let scale = to / from;
        let length = (self.data.len() as f32) * scale;
        let length = length as usize;

        let mut repeat = None;
        if self.repeat_length > 1 {
            let r_start = (self.repeat_start as f32) * 2.0 * scale;
            let r_start = std::cmp::min(r_start as usize, length);
            let r_length = (self.repeat_length as f32) * 2.0 * scale;
            let r_length = std::cmp::min(r_length as usize, length);
            repeat = Some((r_start, r_length))
        }


        let resampled = self.clone().resample(length as usize);

        SamplePlayback {
            signal: resampled,
            volume: self.volume,
            repeat,
            state: SamplePlaybackState::Stopped,
        }
    }
}

impl Signal for Arc<Sample> {
    type Sample = f32;
    fn length(&self) -> usize {
        self.data.len()
    }
    fn get(&self, ix: usize) -> Self::Sample {
        self.data[ix]
    }
}

#[derive(Debug)]
enum SamplePlaybackState {
    Stopped,
    First {
        ix: usize,
    },
    Repeating {
        ix: usize,
    },
}

pub struct SamplePlayback<S: Signal> {
    signal: S,
    repeat: Option<(usize, usize)>,
    state: SamplePlaybackState,
    volume: u8,
}

impl <S: Signal> SamplePlayback<S> {
    fn _length(&self) -> usize {
        if let Some((st, le)) = self.repeat {
            return st + le;
        }
        self.signal.length()
    }
    fn _restart(&mut self) {
        if let Some((st, _)) = self.repeat {
            self.state = SamplePlaybackState::Repeating { ix: st };
        } else {
            self.state = SamplePlaybackState::Stopped;
        }
    }
    fn _forward(&mut self) {
        match self.state {
            SamplePlaybackState::Stopped => (),
            SamplePlaybackState::First { ix } => self.state = SamplePlaybackState::First { ix: ix + 1 },
            SamplePlaybackState::Repeating { ix } => self.state = SamplePlaybackState::Repeating { ix: ix + 1 },
        }
    }
    fn _ix(&self) -> usize {
        match self.state {
            SamplePlaybackState::Stopped => 0,
            SamplePlaybackState::First { ix } => ix,
            SamplePlaybackState::Repeating { ix } => ix,
        }
    }
}

impl <S: Signal<Sample=f32>> sound::Generator for SamplePlayback<S> {
    fn next(&mut self) -> f32 {
        if let SamplePlaybackState::Stopped = self.state {
            return 0.0;
        }

        let ix = self._ix();
        let length = self._length();
        if ix >= length {
            self._restart();
        }
        let val = self.signal.get(ix);
        self._forward();
        let volume = (self.volume as f32)/64.0;

        val * volume
    }
}

impl <S: Signal<Sample=f32>> sound::Enveloped for SamplePlayback<S> {
    fn trigger_start(&mut self) {
        self.state = SamplePlaybackState::First { ix: 0 };
    }
    fn trigger_end(&mut self) {
        self.state = SamplePlaybackState::Stopped;
    }

}

struct Channel {
    generator: Option<SamplePlayback<Interpolator<Arc<Sample>>>>,
    last_sample: Option<usize>,
    last_note: Option<notes::Note>,
    volume_slide: Option<i8>,
}

impl Channel {
    fn new() -> Self {
        Self {
            generator: None,
            last_sample: None,
            last_note: None,
            volume_slide: None,
        }
    }
}

pub struct Player {
    pub playing: bool,
    pub module: Arc<Module>,
    pub program: usize,
    pub pattern: usize,
    pub row: usize,
    tick: usize,
    native_tpd: u16,
    native_bpm: u16,

    division_left: usize,
    tick_left: usize,

    sample_rate: u32,

    incoming_break: Option<usize>,

    channels: Vec<Channel>,
}

impl Player {
    pub fn new(module: &Arc<Module>, sample_rate: f32) -> Self {
        let mut res = Self {
            playing: false,
            module: module.clone(),
            program: 0,
            pattern: 0,
            row: 0,
            tick: 0,
            native_tpd: 6,
            native_bpm: 125,
            division_left: 0,
            tick_left: 0,
            sample_rate: sample_rate as u32,

            incoming_break: None,

            channels: (0..4).map(|_| Channel::new()).collect(),
        };
        res._division_left_reset();
        res._tick_left_reset();
        res._load_row();
        res
    }

    fn _dpm(&self) -> f32 {
        (24.0 * (self.native_bpm as f32)) / (self.native_tpd as f32)
    }

    fn _tick_left_reset(&mut self) {
        let in_division = (60.0 / self._dpm()) * (self.sample_rate as f32);
        let in_tick = in_division / (self.native_tpd as f32);
        self.tick_left = in_tick as usize;
    }

    fn _division_left_reset(&mut self) {
        self.division_left = ((60.0 / self._dpm()) * (self.sample_rate as f32)) as usize;
    }

    fn _load_row(&mut self) {
        for (i, c) in self.module.patterns[self.pattern].rows[self.row].channels.iter().enumerate() {
            if c.period() == 0 && c.sample_number() == 0 {
                continue
            }
            let note = if c.period() == 0 {
                match self.channels[i].last_note {
                    Some(n) => n,
                    None => continue,
                }
            } else {
                c.note()
            };

            let mut sample = c.sample_number() as usize;
            if sample == 0 {
                sample = self.channels[i].last_sample.unwrap_or(0);
            }
            if sample == 0 {
                continue
            }

            let mut sp = self.module.samples[sample-1].clone().play(note, self.sample_rate);
            sp.trigger_start();
            self.channels[i].generator = Some(sp);
            self.channels[i].last_sample = Some(sample);
            self.channels[i].last_note = Some(note);
        }
        for c in self.channels.iter_mut() {
            c.volume_slide = None;
        }
        self.tick = 0;
        log::info!("{}, {}", self.pattern, self.row);
        self._apply_enter_effects();
    }

    fn _next_division(&mut self) {
        self._division_left_reset();
        let (next_row, advance_pattern) = if let Some(d) = self.incoming_break {
            self.incoming_break = None;
            (d, true)
        } else {
            if self.row >= 63 {
                (0, true)
            } else {
                (self.row+1, false)
            }
        };
        self.row = next_row;
        if advance_pattern {
            self.program += 1;
            if self.program >= self.module.program.len() {
                self.program = 0;
            }
            self.pattern = self.module.program[self.program] as usize;
        }
        self._load_row();
    }

    fn _next_tick(&mut self) {
        self._tick_left_reset();
        if self.tick != 0 {
            for c in self.channels.iter_mut() {
                if let Some(slide) = c.volume_slide {
                    if let Some(g) = &mut c.generator {
                        let mut volume = g.volume as i32;
                        let slide = slide as i32;
                        volume += slide;
                        if volume > 64 {
                            volume = 64;
                        }
                        if volume < 0 {
                            volume = 0;
                        }
                        g.volume = volume as u8;
                    }
                }
            }
        }
        self.tick += 1;
    }

    fn _apply_enter_effects(&mut self) {
        for (i, c) in self.module.patterns[self.pattern].rows[self.row].channels.iter().enumerate() {
            let effect = c.effect();
            match effect {
                Effect::VolumeSlide { up, down } => {
                    if up == 0 && down != 0 {
                        self.channels[i].volume_slide = Some(-(down as i8));
                    }
                    if down == 0 && up != 0 {
                        self.channels[i].volume_slide = Some(up as i8);
                    }
                },
                Effect::PatternBreak { division } => {
                    self.incoming_break = Some(division);
                },
                Effect::SetBeatsPerMinute { bpm } => {
                    self.native_bpm = bpm;
                },
                Effect::SetTicksPerDivision { tpd } => {
                    self.native_tpd = tpd;
                }
                Effect::SetVolume { volume } => {
                    if let Some(v) = &mut self.channels[i].generator {
                        v.volume = volume as u8;
                        if v.volume > 64 {
                            v.volume = 64;
                        }
                    }
                }
                Effect::FineVolumeSlideUp { up } => {
                    if let Some(v) = &mut self.channels[i].generator {
                        v.volume += up;
                        if v.volume > 64 {
                            v.volume = 64;
                        }
                    }
                }
                Effect::FineVolumeSlideDown { down } => {
                    if let Some(v) = &mut self.channels[i].generator {
                        if down > v.volume {
                            v.volume = 0;
                        } else {
                            v.volume -= down;
                        }
                    }
                }
                _ => (),
            }
        }
    }
}

impl sound::Generator for Player {
    fn next(&mut self) -> f32 {
        if self.playing == false {
            return 0.0;
        }
        if self.tick_left == 0 {
            self._next_tick();
        } else {
            self.tick_left -= 1;
        }
        if self.division_left == 0 {
            self._next_division();
        } else {
            self.division_left -= 1;
        }
        let mut v: f32 = 0.0;
        for c in self.channels.iter_mut() {
            if let Some(g) = &mut c.generator {
                v += g.next() * 0.3;
            }
        }
        v
    }
}