use std::collections::{BTreeSet, BTreeMap, VecDeque};
use winit::event::{VirtualKeyCode};

use crate::notes;

#[derive(Debug)]
pub enum KeyboardEvent {
    Down(VirtualKeyCode),
    Up(VirtualKeyCode),
}

pub struct Keyboard {
    pressed: BTreeSet<VirtualKeyCode>,
    queue: VecDeque<KeyboardEvent>,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            pressed: BTreeSet::new(),
            queue: VecDeque::new(),
        }
    }
    pub fn press(&mut self, c: VirtualKeyCode) {
        if self.pressed.contains(&c) {
            return
        }
        self.pressed.insert(c.clone());
        self.queue.push_back(KeyboardEvent::Down(c));
    }
    pub fn release(&mut self, c: VirtualKeyCode) {
        if !self.pressed.contains(&c) {
            return
        }
        self.pressed.remove(&c);
        self.queue.push_back(KeyboardEvent::Up(c));
    }
    pub fn drain(&mut self) -> Option<KeyboardEvent> {
        return self.queue.pop_front();
    }
}

pub struct PianoKeyboard {
    notes: BTreeMap<VirtualKeyCode, notes::Note>,
}

impl PianoKeyboard {
    pub fn new() -> Self {
        let mut notes = BTreeMap::new();
        let oct3 = notes::chromatic(notes::A4.octave_down());
        let oct4 = notes::chromatic(notes::A4);
        notes.entry(VirtualKeyCode::A).or_insert(oct3.c);
        notes.entry(VirtualKeyCode::S).or_insert(oct3.d);
        notes.entry(VirtualKeyCode::D).or_insert(oct3.e);
        notes.entry(VirtualKeyCode::F).or_insert(oct3.f);
        notes.entry(VirtualKeyCode::G).or_insert(oct3.g);
        notes.entry(VirtualKeyCode::H).or_insert(oct4.a);
        notes.entry(VirtualKeyCode::J).or_insert(oct4.b);
        notes.entry(VirtualKeyCode::K).or_insert(oct4.c);

        notes.entry(VirtualKeyCode::W).or_insert(oct3.c.sharp());
        notes.entry(VirtualKeyCode::E).or_insert(oct3.d.sharp());

        notes.entry(VirtualKeyCode::T).or_insert(oct3.f.sharp());
        notes.entry(VirtualKeyCode::Y).or_insert(oct3.g.sharp());
        notes.entry(VirtualKeyCode::U).or_insert(oct4.a.sharp());
        Self {
            notes,
        }
    }

    pub fn translate(&self, kc: &VirtualKeyCode) -> Option<notes::Note> {
        self.notes.get(kc).cloned()
    }
}
