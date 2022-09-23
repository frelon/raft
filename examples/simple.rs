use log::{error, info, LevelFilter};

use raft::{
    log::{Collection, Entry, Storage},
    node::{Error, LocalNode, NodeConfig, StateMachine},
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

struct InMemoryLogStorage<LogType> {
    logs: Collection<LogType>,
}

impl<LogType> Storage<LogType> for InMemoryLogStorage<LogType> {
    fn get(&self, term: Term, index: usize) -> Result<LogType, ()> {
        todo!()
    }

    fn write(&mut self, log: Entry<LogType>) -> Result<(), ()> {
        self.logs.push(log);
        Ok(())
    }
}

impl<LogType> Default for InMemoryLogStorage<LogType> {
    fn default() -> Self {
        Self { logs: vec![] }
    }
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter(None, LevelFilter::Trace)
        .init();

    let state = SimpleStateMachine { value: 0 };
    let storage = InMemoryLogStorage::default();
    let mut n = LocalNode::<SimpleCommand>::new(NodeConfig::default(), &state, &storage);

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
