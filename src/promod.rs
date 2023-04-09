use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

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

    pub samples: Vec<Sample>,
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

        let mut signature = vec![0u8; 4];
        f.read_exact(&mut signature)?;

        let npatterns = ptable.iter().max().unwrap() + 1;
        for _ in 0..npatterns {
            let mut pattern = vec![0u8; 1024];
            f.read_exact(&mut pattern)?;
        }

        for (i, sample) in samples.iter_mut().enumerate() {
            for j in 0..(sample.data.len()) {
                let v = f.read_i8().map_err(|e| {
                    Error::SampleError { sample: i, inner: Box::new(e.into()) }
                })?;
                sample.data[j] = v;
            }
        }

        Ok(Self {
            title: title.into(),
            samples,
        })
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

    pub data: Vec<i8>,
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
            data: vec![0i8; length * 2],
        })
    }
}
