use std::sync::{Arc, Mutex};

use log::{error, info, LevelFilter};

use raft::{
    log::{Collection, Entry, Storage},
    node::{Config, Error, LocalNode, Peer, StateMachine, Vote},
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
    fn get(&self, term: usize, index: usize) -> Result<LogType, ()> {
        if let Some(log) = self.logs.get(index) {}

        Err(())
    }

    fn write(&mut self, log: Entry<LogType>) -> Result<(), Error> {
        self.logs.push(log);
        Ok(())
    }
}

impl<LogType> Default for InMemoryLogStorage<LogType> {
    fn default() -> Self {
        Self { logs: vec![] }
    }
}

struct LocalPeer<'state_machine, 'peers, 'log_storage, LogType> {
    remote: Arc<Mutex<LocalNode<'state_machine, 'peers, 'log_storage, LogType>>>,
}

impl<'state_machine, 'peers, 'log_storage, LogType>
    LocalPeer<'state_machine, 'peers, 'log_storage, LogType>
{
    fn new(node: Arc<Mutex<LocalNode<'state_machine, 'peers, 'log_storage, LogType>>>) -> Self {
        Self { remote: node }
    }
}

impl<'state_machine, 'peers, 'log_storage, LogType> Peer<LogType>
    for LocalPeer<'state_machine, 'peers, 'log_storage, LogType>
{
    fn request_vote(
        &self,
        term: usize,
        candidate_id: usize,
        last_log_index: usize,
        last_log_term: usize,
    ) -> Result<Vote, Error> {
        self.remote
            .lock()
            .unwrap()
            .request_vote(term, candidate_id, last_log_index, last_log_term)
    }

    fn append_entries(
        &self,
        term: Term,
        leader_id: usize,
        prev_log_index: usize,
        prev_log_term: Term,
        entries: Collection<LogType>,
        leader_commit: usize,
    ) -> Result<(), Error> {
        self.remote.lock().unwrap().append_entries(
            term,
            leader_id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit,
        )
    }
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter(None, LevelFilter::Trace)
        .init();

    let state1 = SimpleStateMachine { value: 0 };
    let storage1 = InMemoryLogStorage::default();
    let node1 = Arc::new(Mutex::new(LocalNode::<SimpleCommand>::new(
        Config::default(),
        &state1,
        &storage1,
    )));

    let state2 = SimpleStateMachine { value: 0 };
    let storage2 = InMemoryLogStorage::default();
    let node2 = Arc::new(Mutex::new(LocalNode::<SimpleCommand>::new(
        Config::default(),
        &state2,
        &storage2,
    )));

    let state3 = SimpleStateMachine { value: 0 };
    let storage3 = InMemoryLogStorage::default();
    let node3 = Arc::new(Mutex::new(LocalNode::<SimpleCommand>::new(
        Config::default(),
        &state3,
        &storage3,
    )));

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
        Ok(()) => {
            info!("Election finished!");
        }
        Err(err) => {
            error!("Election error: {:?}", err);
        }
    }

    info!("Quitting");
}
