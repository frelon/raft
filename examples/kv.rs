use log::{error, info, trace, LevelFilter};

use raft::{
    log::{Entry, Storage},
    node::{Config, Error, LocalNode, StateMachine},
    term::Term,
};

use sled::Db;

use std::{convert::TryInto, io::ErrorKind, time::Instant, time::Duration};

use serde::{Serialize, Deserialize};

use crossbeam_channel::{bounded, tick, Receiver, select};


use clap::{arg, command};

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

fn ctrl_channel() -> Result<Receiver<()>, ctrlc::Error> {
    let (sender, receiver) = bounded(100);
    ctrlc::set_handler(move || {
        let _ = sender.send(());
    })?;

    Ok(receiver)
}

fn main() -> Result<(), std::io::Error> {
    env_logger::Builder::from_default_env()
        .filter(None, LevelFilter::Trace)
        .init();


    let matches = command!()
        .arg(arg!(db_path: --db_path <PATH>).required(false).default_value("db"))
        .arg(arg!(peers: --peers <PEERS>).required(false).num_args(0..))
        .get_matches();

    let db_path = matches.get_one::<String>("db_path").expect("default value");
    trace!("Opening database {db_path}");
    let db: sled::Db = sled::open(db_path).expect("open database");

    let state = KvMachine::new(&db);
    let mut storage = SledLogStorage::new(&db);

    let mut node = LocalNode::<KvCommand>::new(Config::default(), &state, &mut storage);

    let ctrl_c_events = ctrl_channel().map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
    let ticks = tick(Duration::from_secs(1));

    let mut last = Instant::now();

    loop {
        select! {
            recv(ticks) -> _ => {
                let now = Instant::now();
                node.tick(now - last).expect("does not crash");
                last = now;
            }
            recv(ctrl_c_events) -> _ => {
                info!("SIGINT received, quitting...");
                break;
            }
        }
    }

    Ok(())
}
