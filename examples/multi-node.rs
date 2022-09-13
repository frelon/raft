use std::sync::{
    Arc,
    Mutex,
};

use log::{
    info,
    error,
    LevelFilter,
};

use raft::node::{
    Peer,
    Error,
    StateMachine,
    LocalNode,
    NodeConfig,
    Vote,
    LogEntry,

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

struct LocalPeer<'state_machine, 'peers, LogType> {
    remote:Arc<Mutex<LocalNode<'state_machine, 'peers, LogType>>>,
}

impl<'state_machine, 'peers, LogType> LocalPeer<'state_machine, 'peers, LogType> {
    fn new(node:Arc<Mutex<LocalNode<'state_machine, 'peers, LogType>>>) -> Self {
        Self{
            remote:node,
        }
    }
}

impl<'state_machine, 'peers, LogType> Peer<LogType> for LocalPeer<'state_machine, 'peers, LogType> {
    fn request_vote(&self, term:usize, candidate_id:usize, last_log_index:usize, last_log_term:usize) -> Result<Vote,Error> {
        self.remote.lock().unwrap().request_vote(term, candidate_id, last_log_index, last_log_term)
    }

    fn append_entries(&self, term:usize, leader_id:usize, prev_log_index:usize, prev_log_term:usize, entries:Vec<LogEntry<LogType>>, leader_commit:usize) -> Result<(),Error> {
        self.remote.lock().unwrap().append_entries(term, leader_id, prev_log_index, prev_log_term, entries, leader_commit)
    }
}

fn main() {
    env_logger::Builder::from_default_env().filter(None, LevelFilter::Trace).init();

    let state = SimpleStateMachine{value:0};
    let node1 = Arc::new(Mutex::new(LocalNode::<SimpleCommand>::new(NodeConfig::default(), &state)));
    let node2 = Arc::new(Mutex::new(LocalNode::<SimpleCommand>::new(NodeConfig::default(), &state)));
    let node3 = Arc::new(Mutex::new(LocalNode::<SimpleCommand>::new(NodeConfig::default(), &state)));

    let peer1 = LocalPeer::<SimpleCommand>::new(node1.clone());
    let peer2 = LocalPeer::<SimpleCommand>::new(node2.clone());
    let peer3 = LocalPeer::<SimpleCommand>::new(node3.clone());

    node1.lock().unwrap().add_peer(&peer2);
    node1.lock().unwrap().add_peer(&peer3);

    node2.lock().unwrap().add_peer(&peer1);
    node2.lock().unwrap().add_peer(&peer2);

    node3.lock().unwrap().add_peer(&peer1);
    node3.lock().unwrap().add_peer(&peer2);
    
    match node1.lock().unwrap().run_election() {
        Ok(()) => { info!("Election finished!"); },
        Err(err) => { error!("Election error: {:?}", err); },
    }

    info!("Quitting");
}

