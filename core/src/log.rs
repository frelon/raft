use alloc::vec::Vec;

use crate::term::Term;

#[derive(Debug)]
pub struct Entry<LogType> {
    pub term: Term,
    pub log: LogType,
}

pub type Collection<LogType> = Vec<Entry<LogType>>;

pub trait Storage<LogType> {
    fn get(&self, term: Term, index: usize) -> Result<LogType, ()>;
    fn write(&mut self, log: Entry<LogType>) -> Result<(), ()>;
}
