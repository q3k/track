use std::{ops::{Deref, Index}, marker::PhantomData};

pub trait Sample: Copy {
    fn mult_weigh(&self, w: f32) -> Self;
    fn add_saturated(&self, o: Self) -> Self;
    fn zero() -> Self;
}

impl Sample for i8 {
    fn mult_weigh(&self, w: f32) -> Self {
        ((*self as f32) * w) as i8
    }
    fn add_saturated(&self, o: Self) -> Self {
        *self + o
    }
    fn zero() -> Self {
        0
    }
}
impl Sample for f32 {
    fn mult_weigh(&self, w: f32) -> Self {
        return self * w
    }
    fn add_saturated(&self, o: Self) -> Self {
        return self + o
    }
    fn zero() -> Self {
        0.0
    }
}

pub trait SampleConvertFrom<T: Sample>: Sample {
    fn sample_convert_from(t: T) -> Self;
}

pub trait SampleConvertTo<T: Sample>: Sample {
    fn sample_convert_to(&self) -> T;
}

impl <U: Sample, T: SampleConvertFrom<U>> SampleConvertTo<T> for U {
    fn sample_convert_to(&self) -> T {
        T::sample_convert_from(*self)
    }
}

impl SampleConvertFrom<i8> for f32 {
    fn sample_convert_from(t: i8) -> Self {
        let f = t as f32; // -128 to 127
        let f = f + 128.0; // 0 to 255
        let f = f / 255.0; // 0 to 1.0
        let f = f - 0.5; // -0.5 to 0.5
        let f = f * 2.0; // -1.0 to 1.0
        f
    }
}

pub trait Signal {
    type Sample: Sample;

    fn length(&self) -> usize;
    fn get(&self, ix: usize) -> Self::Sample;
    fn iter<'s>(&'s self) -> SignalIterator<'s, Self> where Self: Sized {
        return SignalIterator { signal: self, ix: 0 }
    }
    fn resample(self, target_length: usize) -> Interpolator<Self> where Self: Sized {
        return Interpolator { signal: self, length: target_length }
    }
    fn convert<O: Sample>(self) -> Converter<Self, O> where Self: Sized {
        return Converter { signal: self, _phantom_o: PhantomData }
    }
    fn volume(self, volume: f32) -> Volume<Self> where Self: Sized {
        return Volume { signal: self, volume }
    }
}

pub struct SignalIterator<'s, S: Signal> {
    signal: &'s S,
    ix: usize,
}

impl <'s, S: Signal> Iterator for SignalIterator<'s, S> {
    type Item = S::Sample;
    fn next(&mut self) -> Option<Self::Item> {
        let ix = self.ix;
        if ix >= self.signal.length() {
            return None;
        }
        self.ix += 1;
        Some(self.signal.get(ix))
    }
}

impl <T: Sample> Signal for Vec<T> {
    type Sample = T;
    fn length(&self) -> usize {
        self.len()
    }
    fn get(&self, ix: usize) -> T {
        *self.index(ix)
    }
}

impl <S: Signal> Signal for std::sync::Arc<S> {
    type Sample = S::Sample;
    fn length(&self) -> usize {
        self.deref().length()
    }
    fn get(&self, ix: usize) -> Self::Sample {
        self.deref().get(ix)
    }
}

pub struct Interpolator<S: Signal> {
    signal: S,
    length: usize,
}

impl <S: Signal> Signal for Interpolator<S> {
    type Sample = S::Sample;
    fn length(&self) -> usize {
        return self.length
    }
    fn get(&self, ix: usize) -> Self::Sample {
        if self.signal.length() == 0 {
            return Self::Sample::zero();
        }
        // Ratio >1 is the interpolator is 'stretching' the underlying signal.
        let ratio = ((self.length - 1) as f32) / ((self.signal.length() - 1) as f32);
        // Underlying ix, as a floating point. Might fall between two underlying
        // sample indices.
        let uix = (ix as f32) / ratio;
        // The 'left' and 'right' side closest integer indices into the
        // underlying sample.
        let uix0 = uix.floor() as usize;
        let uix1 = uix0 + 1;
        // If uix1 is past the range of the underlying sample, it means we're on
        // the right hand side and the weight for uix0 is ~1 and uix1 is ~0.
        // Short circuit and return the value at uix0.
        if uix0 == self.signal.length() - 1 {
            return self.signal.get(uix0);
        }
        // Distances of uix from uix0 and uix1, used for weighted sum.
        let duix0 = uix - (uix0 as f32);
        let duix1 = 1.0 - duix0;
        // Values at uix0 and uix1, used for weighted sum.
        let uv0 = self.signal.get(uix0);
        let uv1 = self.signal.get(uix1);
        // Weighted sum. duix0/1 are swapped because distance == 1.0 - weight.
        uv0.mult_weigh(duix1).add_saturated(uv1.mult_weigh(duix0))
    }
}

pub struct Converter<S: Signal, O: Sample> {
    signal: S,
    _phantom_o: PhantomData<O>,
}

impl <S, O> Signal for Converter<S, O>
where
    S: Signal,
    O: SampleConvertFrom<S::Sample>,
{
    type Sample = O;
    fn length(&self) -> usize {
        self.signal.length()
    }
    fn get(&self, ix: usize) -> O {
        self.signal.get(ix).sample_convert_to()
    }
}

pub struct Volume<S: Signal> {
    signal: S,
    volume: f32,
}

impl <S: Signal> Signal for Volume<S> {
    type Sample = S::Sample;
    fn length(&self) -> usize {
        self.signal.length()
    }
    fn get(&self, ix: usize) -> Self::Sample {
        self.signal.get(ix).mult_weigh(self.volume)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_i8() {
        let input = vec![
            0i8, 0i8, 0i8, 0i8,
            127i8, 127i8, 127i8, 127i8
        ];
        let resampled = input.resample(10);
        assert_eq!(resampled.length(), 10);
        let resampled = resampled.iter().collect::<Vec<i8>>();
        assert_eq!(resampled.length(), 10);
        assert_eq!(resampled, vec![
            0i8, 0i8, 0i8, 0i8, 14i8,
            112i8, 126i8, 126i8, 126i8, 127i8,
        ]);
    }

    #[test]
    fn test_convert_i8_f32() {
        let input = vec![
            -128i8, -128i8, -128i8, -128i8,
            0i8, 0i8, 0i8, 0i8,
            127i8, 127i8, 127i8, 127i8
        ];
        let converted: Converter<_, f32> = input.convert();
        assert_eq!(converted.length(), 12);
        let converted = converted.iter().collect::<Vec<f32>>();
        assert_eq!(converted.length(), 12);
        assert_eq!(converted[0..4], vec![
            -1.0f32, -1.0f32, -1.0f32, -1.0f32,
        ]);
        assert_eq!(converted[8..12], vec![
            1.0f32, 1.0f32, 1.0f32, 1.0f32,
        ]);
    }
}