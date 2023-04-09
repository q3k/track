use std::{collections::BTreeMap};
use crate::notes::{Note,NoteApprox};

pub trait Generator: Sized {
    fn next(&mut self) -> f32;

    fn envelope<E: Envelope>(self, e: E, sample_rate: u32) -> EnvelopedGenerator<Self, E> {
        EnvelopedGenerator {
            sample_rate: sample_rate as f32,
            g: self,
            e,
        }
    }
}

pub trait Envelope {
    fn trigger_start(&mut self);
    fn trigger_end(&mut self);
    fn next(&mut self, delta: f32) -> Option<f32>;
}

enum ADSRState {
    Inactive,
    AttackDecay,
    Sustain,
    Release,
}

pub struct ADSR {
    t: f32,
    state: ADSRState,

    p: ADSRParams,
}

#[derive(Clone)]
pub struct ADSRParams {
    pub a: f32,
    pub d: f32,
    pub s_level: f32,
    pub r: f32,
}

impl ADSR {
    pub fn new(p: &ADSRParams) -> Self {
        Self {
            t: 0.0,
            state: ADSRState::Inactive,
            p: p.clone(),
        }
    }
}

fn lerp(a: f32, b: f32, v: f32) -> f32 {
    (b - a) * v + a
}

impl Envelope for ADSR {
    fn trigger_start(&mut self) {
        self.t = 0.0;
        self.state = ADSRState::AttackDecay;
    }
    fn trigger_end(&mut self) {
        self.t = 0.0;
        self.state = ADSRState::Release;
    }
    fn next(&mut self, delta: f32) -> Option<f32> {
        let t = self.t;
        let p = &self.p;
        match self.state {
            ADSRState::Inactive => return None,
            ADSRState::AttackDecay => {
                self.t += delta;
                if t < p.a {
                    let v = t/ p.a;
                    return Some(lerp(0.0, 1.0, v));
                }
                let t = t - p.a;
                if t < p.d {
                    let v = t / p.d;
                    return Some(lerp(1.0, p.s_level, v));
                }
                self.state = ADSRState::Sustain;
                return Some(p.s_level);
            },
            ADSRState::Sustain => return Some(p.s_level),
            ADSRState::Release => {
                self.t += delta;
                if t >= p.r {
                    self.state = ADSRState::Inactive;
                    return None;
                }
                let v = t / p.r;
                return Some(lerp(p.s_level, 0.0, v));
            },
        }
    }
}

pub trait Enveloped: Generator {
    fn trigger_start(&mut self);
    fn trigger_end(&mut self);
}

pub struct EnvelopedGenerator<G: Generator, E: Envelope> {
    sample_rate: f32,
    g: G,
    e: E,
}

impl<G: Generator, E: Envelope> Generator for EnvelopedGenerator<G, E> {
    fn next(&mut self) -> f32 {
        let v = self.e.next(1.0/self.sample_rate);
        match v {
            None => 0.0,
            Some(v) => self.g.next() * v,
        }
    }
}

impl<G: Generator, E: Envelope> Enveloped for EnvelopedGenerator<G, E> {
    fn trigger_start(&mut self) {
        self.e.trigger_start();
    }
    fn trigger_end(&mut self) {
        self.e.trigger_end();
    }
}

pub struct PolyphonicGenerator<E: Enveloped, F: Fn(Note) -> E> {
    f: F,

    generators: BTreeMap<NoteApprox, E>,
    pub scopes: BTreeMap<NoteApprox, Vec<f32>>, 
    scope_ix: usize,
}

impl<E: Enveloped, F: Fn(Note) -> E> PolyphonicGenerator<E, F> {
    pub fn new(f: F) -> Self {
        Self {
            f,
            generators: BTreeMap::new(),
            scopes: BTreeMap::new(),
            scope_ix: 0,
        }
    }

    pub fn start(&mut self, n: Note) {
        let nap: NoteApprox = n.into();
        if self.generators.contains_key(&nap) {
            self.generators.remove(&nap);
            self.scopes.remove(&nap);
        }

        self.scopes.insert(nap, vec![0.0; 512]);

        let gen = (self.f)(n);
        self.generators.entry(nap).or_insert(gen).trigger_start();
    }

    pub fn stop(&mut self, n: Note) {
        let nap: NoteApprox = n.into();
        if !self.generators.contains_key(&nap) {
            return
        }

        self.generators.get_mut(&nap).unwrap().trigger_end();
    }
}

impl<E: Enveloped, F: Fn(Note) -> E> Generator for PolyphonicGenerator<E, F> {
    fn next(&mut self) -> f32 {
        if self.scope_ix >= 512 {
            self.scope_ix = 0;
        }
        let ix = self.scope_ix;
        self.scope_ix += 1;
        let mut res = 0.0f32;
        for (k, g) in self.generators.iter_mut() {
            let v =  g.next();
            self.scopes.get_mut(k).unwrap()[ix] = v;
            res += v * 0.3;
        }

        res
    }
}