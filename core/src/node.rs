use alloc::vec::Vec;

use core::time::Duration;

use log::trace;

use crate::{
    log::{Collection, Entry, Storage},
    term::Term,
};

#[derive(Debug)]
pub enum Error {
    UnknownError,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vote {
    For,
    Against,
}

#[derive(Debug, PartialEq)]
pub enum State {
    Follower,
    Candidate,
    Leader,
}

pub trait Peer<LogType> {
    fn request_vote(
        &self,
        term: Term,
        candidate_id: usize,
        last_log_index: usize,
        last_log_term: Term,
    ) -> Result<Vote, Error>;
    fn append_entries(
        &self,
        term: Term,
        leader_id: usize,
        prev_log_index: usize,
        prev_log_term: Term,
        entries: Collection<LogType>,
        leader_commit: usize,
    ) -> Result<(), Error>;
}

pub trait StateMachine<LogType> {
    fn apply(&mut self, log: LogType) -> Result<(), Error>;
}

pub struct LocalNode<'state_machine, 'peers, 'log_storage, LogType> {
    config: NodeConfig,
    current_term: Option<usize>,
    voted_for: Option<usize>,
    leader_id: Option<usize>,
    state: State,

    peers: Vec<&'peers dyn Peer<LogType>>,
    logs: &'log_storage dyn Storage<LogType>,
    state_machine: &'state_machine dyn StateMachine<LogType>,
}

pub struct NodeConfig {
    id: usize,
    election_timeout: Duration,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            id: usize::default(),
            election_timeout: Duration::default(),
        }
    }
}

impl NodeConfig {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            election_timeout: Duration::default(),
        }
    }
}

impl<'state_machine, 'peers, 'log_storage, LogType>
    LocalNode<'state_machine, 'peers, 'log_storage, LogType>
{
    pub fn new(
        config: NodeConfig,
        state_machine: &'state_machine dyn StateMachine<LogType>,
        log_storage: &'log_storage dyn Storage<LogType>,
    ) -> Self {
        trace!("Creating new LocalNode");

        LocalNode::<LogType> {
            current_term: None,
            voted_for: None,
            leader_id: None,
            peers: vec![],
            logs: log_storage,
            state: State::Follower,
            state_machine,
            config,
        }
    }

    pub fn with_term(mut self, term: usize) -> Self {
        self.current_term = Some(term);
        self
    }

    pub fn with_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn logs(&self) -> &dyn Storage<LogType> {
        self.logs
    }

    pub fn add_peer(&mut self, peer: &'peers dyn Peer<LogType>) {
        self.peers.push(peer)
    }

    pub fn run_election(&mut self) -> Result<(), Error> {
        let new_term = match self.current_term {
            Some(term) => term + 1,
            None => 0,
        };

        trace!("Starting new election for term {}", new_term);

        let mut votes = 1;

        trace!("Sending request_vote to {} peers", self.peers.len());
        for peer in self.peers.iter_mut() {
            match peer.request_vote(new_term, self.config.id, 0, 0) {
                Ok(Vote::For) => votes += 1,
                Ok(Vote::Against) => {}
                Err(_) => {
                    return Err(Error::UnknownError);
                }
            }
        }

        trace!("Got {} votes", votes);

        if votes > ((self.peers.len() + 1) / 2) {
            trace!("Won election for term {}", new_term);
            self.current_term = Some(new_term);
            self.state = State::Leader;
            self.leader_id = Some(self.config.id);

            trace!("Sending heartbeat to {} peers", self.peers.len());
            for peer in self.peers.iter_mut() {
                match peer.append_entries(new_term, self.config.id, 0, 0, vec![], 0) {
                    Ok(()) => {}
                    Err(_) => {
                        return Err(Error::UnknownError);
                    }
                }
            }
        }

        trace!("Election concluded for term {new_term}");
        Ok(())
    }

    pub fn request_vote(
        &mut self,
        term: Term,
        candidate_id: usize,
        last_log_index: usize,
        last_log_term: Term,
    ) -> Result<Vote, Error> {
        trace!("Received request_vote for term {term}");

        let current_term = self.current_term.unwrap_or(0);

        if term < current_term {
            trace!("Term {term} < {current_term}, voting against");
            return Ok(Vote::Against);
        }

        let Ok(log_entry) = self.logs.get(last_log_term, last_log_index) else {
            trace!("Log does not contain entry at {last_log_index},{last_log_term}");
            return Ok(Vote::Against);
        };

        trace!("Voting for {candidate_id} in term {term}");
        Ok(Vote::For)
    }

    pub fn append_entries(
        &mut self,
        term: Term,
        leader_id: usize,
        prev_log_index: usize,
        prev_log_term: Term,
        entries: Vec<Entry<LogType>>,
        leader_commit: usize,
    ) -> Result<(), Error> {
        trace!("Received append_entries for term {term}");

        if term > self.current_term.unwrap_or_default() {
            trace!("RPC with higher term received");
            self.state = State::Follower;
            self.current_term = Some(term);
            self.voted_for = None;
            self.leader_id = Some(leader_id);
        }

        if entries.len() != 0 {
            todo!("Append entries");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Command {
        Set(u32),
        Incr,
        Decr,
    }

    struct SimpleStateMachine {
        value: u32,
    }

    impl StateMachine<Command> for SimpleStateMachine {
        fn apply(&mut self, log: Command) -> Result<(), Error> {
            match log {
                Command::Set(x) => self.value = x,
                Command::Incr => self.value += 1,
                Command::Decr => self.value -= 1,
            }

            Ok(())
        }
    }

    struct InMemoryLogStorage<LogType> {
        logs: Collection<LogType>,
    }

    impl<LogType> Storage<LogType> for InMemoryLogStorage<LogType> {
        fn get(&self, term: Term, index: usize) -> Result<LogType, ()> {
            if let Some(log) = self.logs.get(index) {}

            Err(())
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

    struct Voter {
        vote: Vote,
    }
    impl<LogType> Peer<LogType> for Voter {
        fn request_vote(
            &self,
            term: Term,
            candidate_id: usize,
            last_log_index: usize,
            last_log_term: Term,
        ) -> Result<Vote, Error> {
            Ok(self.vote)
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
            Ok(())
        }
    }

    impl Voter {
        fn new(vote: Vote) -> Self {
            Self { vote }
        }
    }

    #[test]
    fn new_node() {
        let _node = LocalNode::<Command>::new(
            NodeConfig::default(),
            &SimpleStateMachine { value: 20 },
            &InMemoryLogStorage::default(),
        );
        assert!(true)
    }

    #[test]
    fn simple_state_machine() {
        let mut machine = SimpleStateMachine { value: 0 };

        assert!(matches!(machine.apply(Command::Set(10)), Ok(())));
        assert_eq!(machine.value, 10);
        assert!(matches!(machine.apply(Command::Incr), Ok(())));
        assert_eq!(machine.value, 11);
        assert!(matches!(machine.apply(Command::Incr), Ok(())));
        assert_eq!(machine.value, 12);
        assert!(matches!(machine.apply(Command::Decr), Ok(())));
        assert_eq!(machine.value, 11);
    }

    #[test]
    fn election_peers_votes_yes() {
        let storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            NodeConfig::default(),
            &SimpleStateMachine { value: 0 },
            &storage,
        );
        let node2 = Voter::new(Vote::For);
        let node3 = Voter::new(Vote::For);

        node1.add_peer(&node2);
        node1.add_peer(&node3);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(0));
        assert_eq!(node1.state, State::Leader);
    }

    #[test]
    fn election_peers_votes_no() {
        let storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            NodeConfig::default(),
            &SimpleStateMachine { value: 0 },
            &storage,
        );
        let node2 = Voter::new(Vote::Against);
        let node3 = Voter::new(Vote::Against);

        node1.add_peer(&node2);
        node1.add_peer(&node3);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, None);
        assert_eq!(node1.state, State::Follower);
    }

    #[test]
    fn election_peers_small_majority() {
        let storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            NodeConfig::new(1),
            &SimpleStateMachine { value: 0 },
            &storage,
        );
        let node2 = Voter::new(Vote::Against);
        let node3 = Voter::new(Vote::Against);
        let node4 = Voter::new(Vote::For);
        let node5 = Voter::new(Vote::For);

        node1.add_peer(&node2);
        node1.add_peer(&node3);
        node1.add_peer(&node4);
        node1.add_peer(&node5);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(0));
        assert_eq!(node1.state, State::Leader);
        assert_eq!(node1.leader_id, Some(1));
    }

    #[test]
    fn election_deadlock_no_progress() {
        let storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            NodeConfig::default(),
            &SimpleStateMachine { value: 0 },
            &storage,
        );
        let node2 = Voter::new(Vote::Against);

        node1.add_peer(&node2);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, None);
        assert_eq!(node1.state, State::Follower);
    }

    #[test]
    fn request_vote_returns_against_for_smaller_term() {
        let storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            NodeConfig::default(),
            &SimpleStateMachine { value: 0 },
            &storage,
        )
        .with_term(10);

        assert!(matches!(node1.request_vote(9, 1, 0, 0), Ok(Vote::Against)));
    }

    #[test]
    fn heartbeat_with_higher_term_sets_follower() {
        let storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            NodeConfig::default(),
            &SimpleStateMachine { value: 0 },
            &storage,
        )
        .with_term(10)
        .with_state(State::Leader);

        assert!(matches!(
            node1.append_entries(11, 2, 0, 0, vec![], 0),
            Ok(())
        ));
        assert_eq!(node1.state, State::Follower);
        assert_eq!(node1.current_term, Some(11));
    }

    #[test]
    fn append_entries_adds_entries() {
        let storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            NodeConfig::default(),
            &SimpleStateMachine { value: 0 },
            &storage,
        )
        .with_term(10)
        .with_state(State::Follower);

        assert!(matches!(
            node1.append_entries(10, 2, 0, 0, vec![], 0),
            Ok(())
        ));
    }
}
