mod reader;
mod writer;

pub use self::{reader::BufReader, writer::BufWriter};

const DEFAULT_BUF_SIZE: usize = 1024;
