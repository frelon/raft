use log::{
    info,
    error,
    LevelFilter,
};

use raft::node::{
    Error,
    StateMachine,
    LocalNode,
    NodeConfig,
};

enum SimpleCommand {
    Set(u32),
    Incr,
    Decr,
}

struct SimpleStateMachine {
    value:u32,
}

impl StateMachine<SimpleCommand> for SimpleStateMachine {
    fn apply(&mut self,log:SimpleCommand) -> Result<(), Error> {
        match log {
            SimpleCommand::Set(v) => { self.value = v; },
            SimpleCommand::Incr => { self.value += 1; },
            SimpleCommand::Decr => { self.value -= 1; },
        }

        Ok(())
    }
}

fn main() {
    env_logger::Builder::from_default_env().filter(None, LevelFilter::Trace).init();

    let state = SimpleStateMachine{value:0};
    let mut n = LocalNode::<SimpleCommand>::new(NodeConfig::default(), &state);
    
    match n.run_election() {
        Ok(()) => { info!("Election finished!"); },
        Err(err) => { error!("Election error: {:?}", err); },
    }

    info!("Quitting");
}
