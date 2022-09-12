use alloc::vec::Vec;

use core::time::Duration;

use log::{
    trace,
};

#[derive(Debug)]
pub enum Error {
    UnknownError,
}

#[derive(Clone)]
pub enum Vote {
    For,
    Against,
}

enum State {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug)]
pub struct LogEntry<LogType> {
    term: usize,
    log: LogType,
}

pub struct LocalNode<'state_machine, 'peers, LogType> {
    current_term: Option<usize>,
    voted_for: Option<usize>,
    state: State,

    peers: Vec<&'peers mut dyn Peer<LogType>>,
    logs: Vec<LogEntry<LogType>>,
    state_machine: &'state_machine dyn StateMachine<LogType>,
}

pub struct NodeConfig {
    election_timeout:Duration, 
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
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
         }
    }

    pub fn add_peer(&mut self, peer:&'peers mut dyn Peer<LogType>) {
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
            match peer.request_vote(new_term) {
                Ok(Vote::For) => { votes += 1 },
                Ok(Vote::Against) => {},
                Err(_) => {
                    return Err(Error::UnknownError);
                }
            }
        }

        trace!("Got {} votes", votes);

        if votes > (self.peers.len()/2) {
            trace!("Won election for term {}", new_term);
            self.current_term = Some(new_term);

            trace!("Sending heartbeat to {} peers", self.peers.len());
            for peer in self.peers.iter_mut() {
                match peer.append_entries(new_term, vec![]) {
                    Ok(()) => {},
                    Err(_) => {
                        return Err(Error::UnknownError);
                    }
                }
            }
        }

        trace!("Election finished");
        Ok(())
    }
}

pub trait Peer<LogType> {
    fn request_vote(&mut self, term:usize) -> Result<Vote,Error>;
    fn append_entries(&mut self, term:usize, entries:Vec<LogEntry<LogType>>) -> Result<(),Error>;
}

pub trait StateMachine<LogType> {
    fn apply(&mut self,log:LogType) -> Result<(), Error>;
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

    impl<'state_machine, 'peers, LogType> Peer<LogType> for LocalNode<'state_machine, 'peers, LogType> {
        fn append_entries(&mut self, term:usize, entries: Vec<LogEntry<LogType>>) -> Result<(), Error> {
            if term > self.current_term.unwrap_or_default() {
                self.state = State::Follower;
                self.current_term = Some(term);
                return Ok(());
            }

            todo!()
        }

        fn request_vote(&mut self, term:usize) -> Result<Vote, Error> {
            if self.voted_for.is_some() {
                return Ok(Vote::Against);
            }

            if matches!(self.current_term, Some(t) if term < t) {
                return Ok(Vote::Against);
            }

            self.voted_for = Some(1);

            Ok(Vote::For)
        }
    }

    struct Voter {
        vote:Vote,
    }
    impl<LogType> Peer<LogType> for Voter {
        fn request_vote(&mut self, term:usize) -> Result<Vote,Error> {
            Ok(self.vote.clone())
        }

        fn append_entries(&mut self, term:usize, entries:Vec<LogEntry<LogType>>) -> Result<(),Error> {
            todo!()
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
    fn election() {
        let mut node1 = LocalNode::<Command>::new(NodeConfig::default(), &SimpleStateMachine{value:0});
        let mut node2 = LocalNode::<Command>::new(NodeConfig::default(), &SimpleStateMachine{value:0});

        node1.add_peer(&mut node2);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(1));
        assert_eq!(node2.current_term, Some(1));
    }
}
