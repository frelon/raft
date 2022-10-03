use log::{error, info, LevelFilter};

use raft::{
    log::{Collection, Entry, Storage},
    node::{Config, Error, LocalNode, StateMachine},
    term::Term,
};

enum SimpleCommand {
    Set(u32),
    Incr,
    Decr,
}

struct SimpleStateMachine {
    value: u32,
}

impl StateMachine<SimpleCommand> for SimpleStateMachine {
    fn apply(&mut self, log: SimpleCommand) -> Result<(), Error> {
        match log {
            SimpleCommand::Set(v) => {
                self.value = v;
            }
            SimpleCommand::Incr => {
                self.value += 1;
            }
            SimpleCommand::Decr => {
                self.value -= 1;
            }
        }

        Ok(())
    }
}

struct InMemoryLogStorage<'entry, LogType> {
    logs: Collection<'entry, LogType>,
}

impl<'entry, LogType> Storage<'entry, LogType> for InMemoryLogStorage<'entry, LogType> {
    fn get(&self, term: Term, index: usize) -> Result<LogType, ()> {
        todo!()
    }

    fn write(&mut self, log: &'entry Entry<LogType>) -> Result<(), Error> {
        self.logs.push(log);
        Ok(())
    }
}

impl<'entry, LogType> Default for InMemoryLogStorage<'entry, LogType> {
    fn default() -> Self {
        Self { logs: vec![] }
    }
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter(None, LevelFilter::Trace)
        .init();

    let state = SimpleStateMachine { value: 0 };
    let mut storage = InMemoryLogStorage::default();
    let mut n = LocalNode::<SimpleCommand>::new(Config::default(), &state, &mut storage);

    match n.run_election() {
        Ok(()) => {
            info!("Election finished!");
        }
        Err(err) => {
            error!("Election error: {:?}", err);
        }
    }

    info!("Quitting");
}
