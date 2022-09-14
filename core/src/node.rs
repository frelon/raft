use alloc::vec::Vec;

use core::time::Duration;

use log::{
    trace,
};

#[derive(Debug)]
pub enum Error {
    UnknownError,
}

#[derive(Clone, Copy)]
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

#[derive(Debug)]
pub struct LogEntry<LogType> {
    term: usize,
    log: LogType,
}

pub trait Peer<LogType> {
    fn request_vote(&self, term:usize, candidate_id:usize, last_log_index:usize, last_log_term:usize) -> Result<Vote,Error>;
    fn append_entries(&self, term:usize, leader_id:usize, prev_log_index:usize, prev_log_term:usize, entries:Vec<LogEntry<LogType>>, leader_commit:usize) -> Result<(),Error>;
}

pub trait StateMachine<LogType> {
    fn apply(&mut self,log:LogType) -> Result<(), Error>;
}

pub struct LocalNode<'state_machine, 'peers, LogType> {
    config: NodeConfig,
    current_term: Option<usize>,
    voted_for: Option<usize>,
    state: State,

    peers: Vec<&'peers dyn Peer<LogType>>,
    logs: Vec<LogEntry<LogType>>,
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
    pub fn new(id:usize) -> Self {
        Self{
            id,
            election_timeout: Duration::default(),
        }
    }
}

impl<'state_machine, 'peers, LogType> LocalNode<'state_machine, 'peers, LogType> {
    pub fn new(config:NodeConfig, state_machine:&'state_machine dyn StateMachine<LogType>) -> Self {
        trace!("Creating new LocalNode");

        LocalNode::<LogType>{
            current_term: None,
            voted_for: None,
            peers: vec![],
            logs: vec![],
            state: State::Follower,
            state_machine,
            config,
         }
    }

    pub fn state(&self) -> &State {
        &self.state 
    }

    pub fn add_peer(&mut self, peer:&'peers dyn Peer<LogType>) {
        self.peers.push(peer)
    }

    pub fn run_election(&mut self) -> Result<(), Error> {
        let new_term = match self.current_term {
            Some(term) => term+1,
            None => 1,
        };

        trace!("Starting new election for term {}", new_term);

        let mut votes = 1;

        trace!("Sending request_vote to {} peers", self.peers.len());
        for peer in self.peers.iter_mut() {
            match peer.request_vote(new_term, self.config.id, 0,0) {
                Ok(Vote::For) => { votes += 1 },
                Ok(Vote::Against) => {},
                Err(_) => {
                    return Err(Error::UnknownError);
                }
            }
        }

        trace!("Got {} votes", votes);

        if votes > ((self.peers.len()+1)/2) {
            trace!("Won election for term {}", new_term);
            self.current_term = Some(new_term);
            self.state = State::Leader;

            trace!("Sending heartbeat to {} peers", self.peers.len());
            for peer in self.peers.iter_mut() {
                match peer.append_entries(new_term, self.config.id, 0, 0, vec![], 0) {
                    Ok(()) => {},
                    Err(_) => {
                        return Err(Error::UnknownError);
                    }
                }
            }
        }

        trace!("Election concluded for term {new_term}");
        Ok(())
    }

    pub fn request_vote(&mut self, term:usize, candidate_id:usize, last_log_index:usize, last_log_term:usize) -> Result<Vote,Error> {
        trace!("Received request_vote for term {term}");

        if term > self.current_term.unwrap_or_default() {
            self.state = State::Follower;
            self.current_term = Some(term);
            self.voted_for = Some(1);
            return Ok(Vote::For);
        }

        todo!()
    }

    pub fn append_entries(&mut self, term:usize, leader_id:usize, prev_log_index:usize, prev_log_term:usize, entries:Vec<LogEntry<LogType>>, leader_commit:usize) -> Result<(),Error> {
        trace!("Received append_entries for term {term}");

        if term > self.current_term.unwrap_or_default() {
            trace!("RPC with higher term received");
            self.state = State::Follower;
            self.current_term = Some(term);
            self.voted_for = Some(1);
        }

        if entries.len() == 0 {
            return Ok(());
        }

        todo!()
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
        value:u32,
    }

    impl StateMachine<Command> for SimpleStateMachine {
        fn apply(&mut self,log:Command) -> Result<(), Error> {
            match log {
                Command::Set(x) => { self.value = x },
                Command::Incr => { self.value += 1 },
                Command::Decr => { self.value -= 1 },
            }

            Ok(())
        }
    }

    struct Voter {
        vote:Vote,
    }
    impl<LogType> Peer<LogType> for Voter {
        fn request_vote(&self, term:usize, candidate_id:usize, last_log_index:usize, last_log_term:usize) -> Result<Vote,Error> {
            Ok(self.vote)
        }

        fn append_entries(&self, term:usize, leader_id:usize, prev_log_index:usize, prev_log_term:usize, entries:Vec<LogEntry<LogType>>, leader_commit:usize) -> Result<(),Error> {
            Ok(())
        }
    }

    impl Voter {
        fn new(vote:Vote) -> Self {
            Self{
                vote,
            }
        }
    }

    #[test]
    fn new_node() {
        let _node = LocalNode::<Command>::new(NodeConfig::default(), &SimpleStateMachine{value:20});
        assert!(true)
    }

    #[test]
    fn simple_state_machine() {
        let mut machine = SimpleStateMachine{
            value: 0,
        };

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
        let mut node1 = LocalNode::<Command>::new(NodeConfig::default(), &SimpleStateMachine{value:0});
        let node2 = Voter::new(Vote::For);
        let node3 = Voter::new(Vote::For);

        node1.add_peer(&node2);
        node1.add_peer(&node3);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(1));
        assert_eq!(node1.state, State::Leader);
    }

    #[test]
    fn election_peers_votes_no() {
        let mut node1 = LocalNode::<Command>::new(NodeConfig::default(), &SimpleStateMachine{value:0});
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
        let mut node1 = LocalNode::<Command>::new(NodeConfig::default(), &SimpleStateMachine{value:0});
        let node2 = Voter::new(Vote::Against);
        let node3 = Voter::new(Vote::Against);
        let node4 = Voter::new(Vote::For);
        let node5 = Voter::new(Vote::For);

        node1.add_peer(&node2);
        node1.add_peer(&node3);
        node1.add_peer(&node4);
        node1.add_peer(&node5);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(1));
        assert_eq!(node1.state, State::Leader);
    }

    #[test]
    fn election_deadlock() {
        let mut node1 = LocalNode::<Command>::new(NodeConfig::default(), &SimpleStateMachine{value:0});
        let node2 = Voter::new(Vote::Against);

        node1.add_peer(&node2);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, None);
        assert_eq!(node1.state, State::Follower);
    }
}
