
pub mod raft_service;
pub mod raft_service_grpc;

use raft::{
    log::{Collection, Entry, Storage},
    node::{Config, Error, LocalNode, Peer, StateMachine, Vote},
    term::Term,
};

use raft_service_grpc::*;
use raft_service::*;

use futures::executor;

use grpc::{ClientStub,ClientConf,ClientStubExt};

use std::marker::PhantomData;

struct GrpcPeer<LogType> {
    id: usize,
    phantom: PhantomData<LogType>,
    client: RaftClient,
}

impl<LogType> GrpcPeer<LogType> {
    fn new(id:usize,client:RaftClient) -> Self {
        Self {
            id,
            phantom: PhantomData,
            client,
        }
    }
}

impl<'entry, LogType: 'entry> Peer<'entry, LogType>
    for GrpcPeer<LogType>
{
    fn id(&self) -> usize {
        self.id
    }

    fn request_vote(
        &self,
        term: usize,
        candidate_id: usize,
        last_log_index: usize,
        last_log_term: usize,
    ) -> Result<Vote, Error> {
        let opts = Default::default();

        let mut req = RequestVoteMsg::new();
        req.set_term(term as u32);
        req.set_candidate_id(candidate_id as u32);
        req.set_last_log_index(last_log_index as u32);
        req.set_last_log_term(last_log_term as u32);

        let resp = self.client.request_vote(opts, req).join_metadata_result();
        let resp = executor::block_on(resp);

        match resp {
            Ok((_, msg, _)) => 
                Ok(if msg.vote_granted { Vote::For } else { Vote::Against }),
            Err(_) => Err(Error::UnknownError),
        }
    }

    fn append_entries(
        &self,
        term: Term,
        leader_id: usize,
        prev_log_index: usize,
        prev_log_term: Term,
        entries: Collection<'entry, LogType>,
        leader_commit: usize,
    ) -> Result<(), Error> {
        let opts = Default::default();

        let serialized_entries = bincode::serialize(&entries).expect("must serialize");

        let mut req = AppendEntriesMsg::new();
        req.set_term(term as u32);
        req.set_leader_id(leader_id as u32);
        req.set_prev_log_index(prev_log_index as u32);
        req.set_prev_log_term(prev_log_term as u32);
        req.set_entries(serialized_entries);
        req.set_leader_commit(leader_commit as u32);

        let resp = self.client.append_entries(opts, req).join_metadata_result();
        let resp = executor::block_on(resp);

        match resp {
            Ok((_, msg, _)) => Ok(()),
            Err(_) => Err(Error::UnknownError),
        }
    }
}

