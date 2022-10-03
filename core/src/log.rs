use alloc::vec::Vec;

use crate::{node::Error, term::Term};

#[derive(Debug)]
pub struct Entry<LogType> {
    pub term: Term,
    pub index: usize,
    pub log: LogType,
}

pub type Collection<'entry, LogType> = Vec<&'entry Entry<LogType>>;

pub trait Storage<'entry, LogType> {
    fn get(&self, term: Term, index: usize) -> Result<LogType, ()>;
    fn write(&mut self, log: &'entry Entry<LogType>) -> Result<(), Error>;
}
