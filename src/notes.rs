#[derive(Clone, Copy)]
pub struct Note(f32);

const SEMITONE: f32 = 1.0594630943592953;

#[allow(dead_code)]
impl Note {
    pub const fn new(f: f32) -> Self {
        Note(f)
    }
    pub fn octave_up(&self) -> Self {
        Note(self.0 * 2.0)
    }
    pub fn octave_down(&self) -> Self {
        Note(self.0 / 2.0)
    }
    pub fn mod_semitones(&self, n: i32) -> Self {
        Note(self.0 * SEMITONE.powf(n as f32))
    }
    pub fn chord<C: Into<Vec<i32>>>(&self, c: C) -> Vec<Self> {
        c.into().iter().cloned().map(|st| self.mod_semitones(st)).collect()
    }
    pub fn freq(&self) -> f32 {
        self.0
    }
    pub fn sharp(&self) -> Self {
        self.mod_semitones(1)
    }
    pub fn flat(&self) -> Self {
        self.mod_semitones(-1)
    }
}

#[allow(dead_code)]
pub const TRIAD_MAJOR: [i32; 3] = [0, 4, 7];
#[allow(dead_code)]
pub const TRIAD_MINOR: [i32; 3] = [0, 3, 7];

pub const A4: Note = Note::new(440.0);

pub struct Scale {
    pub a: Note,
    pub b: Note,
    pub c: Note,
    pub d: Note,
    pub e: Note,
    pub f: Note,
    pub g: Note,
}

#[allow(dead_code)]
pub fn chromatic(a: Note) -> Scale {
    Scale {
        a,
        b: a.mod_semitones(2),
        c: a.mod_semitones(3),
        d: a.mod_semitones(5),
        e: a.mod_semitones(7),
        f: a.mod_semitones(8),
        g: a.mod_semitones(10),
    }
}

#[derive(PartialEq,Eq,PartialOrd,Ord,Debug,Clone,Copy)]
pub struct NoteApprox(u32);

impl From<Note> for NoteApprox {
    fn from(value: Note) -> Self {
        let f = (value.freq() * 10.0) as u32;
        NoteApprox(f)
    }
}