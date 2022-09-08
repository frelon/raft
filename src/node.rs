use alloc::vec::Vec;

enum RaftError {
    UnknownError,
}

enum State {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug)]
struct LogEntry<LogType> {
    term: usize,
    log: LogType,
}

struct LocalNode<'state_machine, 'peers, LogType> {
    current_term: Option<usize>,
    voted_for: Option<usize>,
    state: State,

    peers: Vec<&'peers mut dyn Peer<LogType>>,
    logs: Vec<LogEntry<LogType>>,
    state_machine: &'state_machine dyn StateMachine<LogType>,
}

impl<'state_machine, 'peers, LogType> LocalNode<'state_machine, 'peers, LogType> {
    fn new(state_machine:&'state_machine dyn StateMachine<LogType>) -> Self {
         LocalNode::<LogType>{
            current_term: None,
            voted_for: None,
            peers: vec![],
            logs: vec![],
            state: State::Follower,
            state_machine,
         }
    }

    fn add_peer(&mut self, peer:&'peers mut dyn Peer<LogType>) {
        self.peers.push(peer)
    }

    fn run_election(&mut self) -> Result<(), RaftError> {
        let new_term = match self.current_term {
            Some(term) => term+1,
            None => 1,
        };

        let mut votes = 1;

        for peer in self.peers.iter_mut() {
            match peer.request_vote(new_term) {
                Ok(true) => { votes += 1 },
                Ok(false) => {},
                Err(_) => return Err(RaftError::UnknownError),
            }
        }

        if votes > (self.peers.len()/2) {
            self.current_term = Some(new_term);

            for peer in self.peers.iter_mut() {
                match peer.heartbeat(new_term) {
                    Ok(()) => {},
                    Err(_) => return Err(RaftError::UnknownError),
                }
            }
        }

        Ok(())
    }
}

trait Peer<LogType> {
    fn request_vote(&mut self, term:usize) -> Result<bool,RaftError>;
    fn append_entries(&mut self, term:usize, entries:Vec<LogEntry<LogType>>) -> Result<(),RaftError>;
    fn heartbeat(&mut self, term:usize) -> Result<(), RaftError>;
}

trait StateMachine<LogType> {
    fn apply(&mut self,log:LogType) -> Result<(), RaftError>;
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
        fn apply(&mut self,log:Command) -> Result<(), RaftError> {
            match log {
                Command::Set(x) => { self.value = x },
                Command::Incr => { self.value += 1 },
                Command::Decr => { self.value -= 1 },
            }

            Ok(())
        }
    }

    impl<'state_machine, 'peers, LogType> Peer<LogType> for LocalNode<'state_machine, 'peers, LogType> {
        fn append_entries(&mut self, term:usize, entries: Vec<LogEntry<LogType>>) -> Result<(), RaftError> {
            todo!()
        }

        fn request_vote(&mut self, term:usize) -> Result<bool, RaftError> {
            if self.voted_for.is_some() {
                return Ok(false);
            }

            if matches!(self.current_term, Some(t) if term < t) {
                return Ok(false);
            }

            self.voted_for = Some(1);

            Ok(true)
        }

        fn heartbeat(&mut self, term:usize) -> Result<(), RaftError> {
            if term > self.current_term.unwrap_or_default() {
                self.state = State::Follower;
                self.current_term = Some(term);
            }

            Ok(())
        }
    }


    #[test]
    fn new_node() {
        let _node = LocalNode::<Command>::new(&SimpleStateMachine{value:20});
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
        let mut node1 = LocalNode::<Command>::new(&SimpleStateMachine{value:0});
        let mut node2 = LocalNode::<Command>::new(&SimpleStateMachine{value:0});

        node1.add_peer(&mut node2);

        assert!(node1.run_election().is_ok());

        assert_eq!(node1.current_term, Some(1));
        assert_eq!(node2.current_term, Some(1));
    }
}
