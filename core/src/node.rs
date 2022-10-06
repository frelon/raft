use alloc::vec::Vec;

use core::time::Duration;

use log::{trace,error};

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
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

pub trait Peer<'entry, LogType> {
    fn id(&self) -> usize;
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
        entries: Collection<'entry, LogType>,
        leader_commit: usize,
    ) -> Result<(), Error>;
}

pub trait StateMachine<LogType> {
    fn apply(&mut self, log: LogType) -> Result<(), Error>;
}

pub struct LocalNode<'state_machine, 'peers, 'log_storage, 'entry, LogType> {
    config: Config,
    current_term: Option<usize>,
    voted_for: Option<usize>,
    leader_id: Option<usize>,
    role: Role,

    peers: Vec<&'peers dyn Peer<'entry, LogType>>,
    log_storage: &'log_storage mut dyn Storage<'entry, LogType>,
    state_machine: &'state_machine dyn StateMachine<LogType>,
}

pub struct Config {
    id: usize,
    election_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: usize::default(),
            election_timeout: Duration::default(),
        }
    }
}

impl Config {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            election_timeout: Duration::default(),
        }
    }
}

impl<'state_machine, 'peers, 'log_storage, 'entry, LogType: 'entry>
    LocalNode<'state_machine, 'peers, 'log_storage, 'entry, LogType>
{
    pub fn new(
        config: Config,
        state_machine: &'state_machine dyn StateMachine<LogType>,
        log_storage: &'log_storage mut dyn Storage<'entry, LogType>,
    ) -> Self {
        trace!("Creating new LocalNode");

        LocalNode::<LogType> {
            current_term: None,
            voted_for: None,
            leader_id: None,
            peers: vec![],
            log_storage,
            role: Role::Follower,
            state_machine,
            config,
        }
    }

    pub fn id(&self) -> usize {
        self.config.id
    }

    pub fn with_term(mut self, term: usize) -> Self {
        self.current_term = Some(term);
        self
    }

    pub fn with_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    pub fn role(&self) -> &Role {
        &self.role
    }

    pub fn storage(&self) -> &dyn Storage<'entry, LogType> {
        self.log_storage
    }

    pub fn add_peer(&mut self, peer: &'peers dyn Peer<'entry, LogType>) {
        self.peers.push(peer)
    }

    /// Candidates (§5.2):
    /// On conversion to candidate, start election:
    /// - Increment currentTerm
    /// - Vote for self
    /// - Reset election timer
    /// - Send RequestVote RPCs to all other servers
    /// - If votes received from majority of servers: become leader
    /// - If AppendEntries RPC received from new leader: convert to follower
    /// - If election timeout elapses: start new election
    pub fn run_election(&mut self) -> Result<(), Error> {
        let new_term = match self.current_term {
            Some(term) => term + 1,
            None => 0,
        };

        trace!("Starting new election for term {}", new_term);

        self.current_term = Some(new_term);
        self.voted_for = Some(self.id());
        self.role = Role::Candidate;

        let mut votes = 1;

        trace!("Sending request_vote to {} peers", self.peers.len());
        for peer in self.peers.iter_mut() {
            match peer.request_vote(new_term, self.config.id, 0, 0) {
                Ok(Vote::For) => {
                    trace!("peer {} voted FOR", peer.id());
                    votes += 1;
                },
                Ok(Vote::Against) => {
                    trace!("peer {} voted AGAINST", peer.id());
                }
                Err(err) => {
                    error!("requested vote from peer {}, but got error {:?}", peer.id(), err);
                    return Err(Error::UnknownError);
                }
            }
        }

        trace!("Got {} votes", votes);

        if votes > ((self.peers.len() + 1) / 2) {
            trace!("Won election for term {}", new_term);
            self.role = Role::Leader;
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
        } else {
            trace!("Lost election for term {new_term}, becoming Follower");
            self.role = Role::Follower;
        }

        trace!("Election concluded for term {new_term}");
        Ok(())
    }

    /// Receiver implementation:
    /// 1. Reply false if term < currentTerm (§5.1)
    /// 2. If votedFor is null or candidateId, and candidate’s log is at least as up-to-date as
    ///    receiver’s log, grant vote (§5.2, §5.4)
    pub fn request_vote(
        &mut self,
        term: Term,
        candidate_id: usize,
        last_log_index: usize,
        last_log_term: Term,
    ) -> Result<Vote, Error> {
        trace!("Received request_vote for term {term} from {candidate_id}");

        if self.current_term.is_none() {
            trace!("no term, voting for {candidate_id}!");
            return Ok(Vote::For);
        }

        let current_term = self.current_term.expect("is not none");

        if term < current_term {
            trace!("Term {term} < {current_term}, voting against {candidate_id}");
            return Ok(Vote::Against);
        }

        let Ok(log_entry) = self.log_storage.get(last_log_term, last_log_index) else {
            trace!("Log does not contain entry at {last_log_index},{last_log_term}, voting against {candidate_id}");
            return Ok(Vote::Against);
        };

        trace!("Voting for {candidate_id} in term {term}");
        Ok(Vote::For)
    }

    /// Receiver implementation:
    /// 1. Reply false if term < currentTerm (§5.1)
    /// 2. Reply false if log doesn’t contain an entry at prevLogIndex whose term matches
    ///    prevLogTerm (§5.3)
    /// 3. If an existing entry conflicts with a new one (same index but different terms), delete
    ///    the existing entry and all that follow it (§5.3)
    /// 4. Append any new entries not already in the log
    /// 5. If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of last new
    ///    entry)
    pub fn append_entries(
        &mut self,
        term: Term,
        leader_id: usize,
        prev_log_index: usize,
        prev_log_term: Term,
        entries: Collection<'entry, LogType>,
        leader_commit: usize,
    ) -> Result<(), Error> {
        trace!("Received append_entries for term {term}");

        if term > self.current_term.unwrap_or_default() {
            trace!("RPC with higher term received");
            self.role = Role::Follower;
            self.current_term = Some(term);
            self.voted_for = None;
            self.leader_id = Some(leader_id);
        }

        for entry in entries {
            self.log_storage.write(&entry);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    #[non_exhaustive]
    enum Command {
        #[default] 
        Noop,
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
                _ => {},
            }

            Ok(())
        }
    }

    #[derive(Default)]
    struct InMemoryLogStorage<'entry, LogType> {
        logs: Collection<'entry, LogType>,
    }

    impl<'entry, LogType> Storage<'entry, LogType> for InMemoryLogStorage<'entry, LogType> {
        fn get(&self, term: Term, index: usize) -> Result<LogType, ()> {
            if let Some(log) = self.logs.get(index) {}

            Err(())
        }

        fn write(&mut self, log: &'entry Entry<LogType>) -> Result<(), Error> {
            self.logs.push(log);
            Ok(())
        }
    }

    struct Voter {
        id: usize,
        vote: Vote,
    }
    impl<'entry, LogType> Peer<'entry, LogType> for Voter {
        fn id(&self) -> usize {
            self.id 
        }

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
        fn new(id: usize, vote: Vote) -> Self {
            Self { id, vote }
        }
    }

    #[test]
    fn new_node() {
        let mut storage = InMemoryLogStorage::default();
        let _node = LocalNode::<Command>::new(
            Config::default(),
            &SimpleStateMachine { value: 20 },
            &mut storage,
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
        let mut storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            Config::default(),
            &SimpleStateMachine { value: 0 },
            &mut storage,
        );
        let node2 = Voter::new(2, Vote::For);
        let node3 = Voter::new(3, Vote::For);

        node1.add_peer(&node2);
        node1.add_peer(&node3);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(0));
        assert_eq!(node1.role, Role::Leader);
    }

    #[test]
    fn election_peers_votes_no() {
        let mut storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            Config::default(),
            &SimpleStateMachine { value: 0 },
            &mut storage,
        );
        let node2 = Voter::new(2, Vote::Against);
        let node3 = Voter::new(3, Vote::Against);

        node1.add_peer(&node2);
        node1.add_peer(&node3);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(0));
        assert_eq!(node1.role, Role::Follower);
    }

    #[test]
    fn election_peers_small_majority() {
        let mut storage = InMemoryLogStorage::default();
        let mut node1 =
            LocalNode::<Command>::new(Config::new(1), &SimpleStateMachine { value: 0 }, &mut storage);
        let node2 = Voter::new(2, Vote::Against);
        let node3 = Voter::new(3, Vote::Against);
        let node4 = Voter::new(4, Vote::For);
        let node5 = Voter::new(5, Vote::For);

        node1.add_peer(&node2);
        node1.add_peer(&node3);
        node1.add_peer(&node4);
        node1.add_peer(&node5);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(0));
        assert_eq!(node1.role, Role::Leader);
        assert_eq!(node1.leader_id, Some(1));
    }

    #[test]
    fn election_deadlock_no_progress() {
        let mut storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            Config::default(),
            &SimpleStateMachine { value: 0 },
            &mut storage,
        );
        let node2 = Voter::new(2, Vote::Against);

        node1.add_peer(&node2);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(0));
        assert_eq!(node1.role, Role::Follower);
    }

    #[test]
    fn request_vote_returns_against_for_smaller_term() {
        let mut storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            Config::default(),
            &SimpleStateMachine { value: 0 },
            &mut storage,
        )
        .with_term(10);

        assert!(matches!(node1.request_vote(9, 1, 0, 0), Ok(Vote::Against)));
    }

    #[test]
    fn heartbeat_with_higher_term_sets_follower() {
        let mut storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            Config::default(),
            &SimpleStateMachine { value: 0 },
            &mut storage,
        )
        .with_term(10)
        .with_role(Role::Leader);

        assert!(matches!(
            node1.append_entries(11, 2, 0, 0, vec![], 0),
            Ok(())
        ));
        assert_eq!(node1.role, Role::Follower);
        assert_eq!(node1.current_term, Some(11));
    }

    #[test]
    fn append_entries_adds_entries() {
        let mut storage = InMemoryLogStorage::default();
        let mut node1 = LocalNode::<Command>::new(
            Config::default(),
            &SimpleStateMachine { value: 0 },
            &mut storage,
        )
        .with_term(10)
        .with_role(Role::Follower);

        assert!(matches!(
            node1.append_entries(10, 2, 0, 0, vec![], 0),
            Ok(())
        ));
    }
}
