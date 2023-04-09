use crate::sound;

pub trait Waveform {
    fn render(&self, i: f32) -> f32;
    fn period(&self) -> f32;
}

pub struct SineWave {
    freq: f32,
}

impl SineWave {
    pub fn new(freq: f32) -> Self {
        Self {
            freq,
        }
    }
}


impl Waveform for SineWave {
    fn render(&self, i: f32) -> f32 {
        return (i * self.freq * 2.0 * 3.141519).sin()
    }
    fn period(&self) -> f32 {
        return 1.0 / self.freq;
    }
}

pub struct SquareWave {
    freq: f32,
}

impl SquareWave {
    pub fn new(freq: f32) -> Self {
        Self {
            freq,
        }
    }
}

impl Waveform for SquareWave {
    fn render(&self, i: f32) -> f32 {
        let v = (i * self.freq) % 1.0;
        if v >= 0.5 {
            return 1.0;
        }
        return -1.0;
    }
    fn period(&self) -> f32 {
        return 1.0 / self.freq;
    }
}

#[derive(PartialEq,Eq,Clone,Copy)]
pub enum WaveformKind {
    Sine,
    Square,
}

pub enum AnyWaveform {
    Sine(SineWave),
    Square(SquareWave),
}


impl WaveformKind {
    pub fn new(&self, freq: f32) -> AnyWaveform {
        match self {
            WaveformKind::Sine => AnyWaveform::Sine(SineWave::new(freq)),
            WaveformKind::Square => AnyWaveform::Square(SquareWave::new(freq)),
        }
    }
}

impl Waveform for AnyWaveform {
    fn period(&self) -> f32 {
        match self {
            AnyWaveform::Sine(s) => s.period(),
            AnyWaveform::Square(s) => s.period(),
        }
    }
    fn render(&self, i: f32) -> f32 {
        match self {
            AnyWaveform::Sine(s) => s.render(i),
            AnyWaveform::Square(s) => s.render(i),
        }
    }
}

pub struct Oscillator<W: Waveform> {
    sample_rate: f32,
    cur: f32,
    volume: f32,

    waveform: W,
}

impl<W: Waveform> Oscillator<W> {
    pub fn new(sample_rate: u32, w: W) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            cur: 0.0,
            volume: 0.9,

            waveform: w,
        }
    }
}

impl <W: Waveform> sound::Generator for Oscillator<W> {
    fn next(&mut self) -> f32 {
        let res = self.waveform.render(self.cur) * self.volume;
        self.cur += 1.0 / self.sample_rate;
        self.cur %= self.waveform.period();
        res
    }
}
