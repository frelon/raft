use log::{error, info, trace, LevelFilter};

use raft::{
    log::{Entry, Storage},
    node::{Config, Error, LocalNode, StateMachine},
    term::Term,
};

use sled::Db;

use std::{convert::TryInto, io::ErrorKind};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum KvCommand {
    Set(String, u32),
    Incr(String),
    Decr(String),
}

struct KvMachine<'db> {
    db: &'db Db,
}

impl<'db> KvMachine<'db> {
    fn new(db: &'db Db) -> Self {
        Self { db }
    }
}

fn incr(old: Option<&[u8]>) -> Option<Vec<u8>> {
    add(old, 1)
}

fn decr(old: Option<&[u8]>) -> Option<Vec<u8>> {
    add(old, -1)
}

fn add(old: Option<&[u8]>, num: i32) -> Option<Vec<u8>> {
    let number = match old {
        Some(bytes) => {
            let array: [u8; 4] = bytes.try_into().unwrap();
            let number = u32::from_be_bytes(array);
            number as i32 + num
        }
        None => 0,
    };

    Some(number.to_be_bytes().to_vec())
}

impl<'db> StateMachine<KvCommand> for KvMachine<'db> {
    fn apply(&mut self, log: KvCommand) -> Result<(), Error> {
        match log {
            KvCommand::Set(k, v) => {
                trace!("setting '{k}' to {v}");
                self.db.insert(k, &v.to_be_bytes())
            }
            KvCommand::Incr(k) => {
                trace!("incr '{k}'");
                self.db.update_and_fetch(k, incr)
            }
            KvCommand::Decr(k) => {
                trace!("decr '{k}'");
                self.db.update_and_fetch(k, decr)
            }
        }
        .map(|_| Ok(()))
        .map_err(|_| Error::UnknownError)?
    }
}

struct SledLogStorage<'db> {
    db: &'db Db,
}

impl<'db> SledLogStorage<'db> {
    fn new(db: &'db Db) -> Self {
        Self { db }
    }
}

impl<'db, 'entry> Storage<'entry, KvCommand> for SledLogStorage<'db> {
    fn get(&self, term: Term, index: usize) -> Result<KvCommand, ()> {
        todo!()
    }

    fn write(&mut self, log: &'entry Entry<KvCommand>) -> Result<(), Error> {
        trace!("Writing log entry {} to disk", log.index);
        self.db
            .insert(log.index.to_be_bytes(), bincode::serialize(&log.log).expect("must serialize"))
            .map(|_| ())
            .map_err(|_| Error::UnknownError)
    }
}

fn main() -> Result<(), std::io::Error> {
    env_logger::Builder::from_default_env()
        .filter(None, LevelFilter::Trace)
        .init();

    let db_path = std::env::var("DB_PATH").unwrap_or("db".to_string());
    let db: sled::Db = sled::open(db_path).unwrap();

    let state = KvMachine::new(&db);
    let mut storage = SledLogStorage::new(&db);

    let mut n = LocalNode::<KvCommand>::new(Config::default(), &state, &mut storage);

    match n.run_election() {
        Ok(()) => {
            info!("Election finished, became {:?}", n.role());
        }
        Err(err) => {
            error!("Election error: {:?}", err);
            return Err(std::io::Error::new(ErrorKind::Other, "election error!"));
        }
    }

    info!("Quitting");

    Ok(())
}
