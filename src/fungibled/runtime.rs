// RGB standard library
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use core::borrow::Borrow;
use core::convert::TryFrom;
use std::collections::BTreeMap;
use std::path::PathBuf;

use bitcoin::OutPoint;
use internet2::zmqsocket::ZmqType;
use internet2::TypedEnum;
use internet2::{
    session, transport, CreateUnmarshaller, PlainTranscoder, Session,
    Unmarshall, Unmarshaller,
};
use lnpbp::client_side_validation::Conceal;
use microservices::node::TryService;
use microservices::FileFormat;
use rgb::{Assignments, Consignment, ContractId, Genesis, Node};
use rgb20::schema::OwnedRightsType;
use rgb20::{schema, AccountingAmount, Asset, OutpointCoins};

use super::cache::{Cache, FileCache, FileCacheConfig};
use super::Config;
use crate::error::{
    ApiErrorType, BootstrapError, RuntimeError, ServiceError,
    ServiceErrorDomain, ServiceErrorSource,
};
use crate::rpc::{
    self,
    fungible::{AcceptApi, Issue, Request, TransferApi},
    reply,
    stash::ConsignRequest,
    stash::MergeRequest,
    Reply,
};
use crate::util::ToBech32Data;

pub struct Runtime {
    /// Original configuration object
    config: Config,

    /// Request-response API session
    session_rpc:
        session::Raw<PlainTranscoder, transport::zmqsocket::Connection>,

    /// Publish-subscribe API session
    session_pub:
        session::Raw<PlainTranscoder, transport::zmqsocket::Connection>,

    /// Stash RPC client session
    stash_rpc: session::Raw<PlainTranscoder, transport::zmqsocket::Connection>,

    /// Publish-subscribe API socket
    stash_sub: session::Raw<PlainTranscoder, transport::zmqsocket::Connection>,

    /// RGB fungible assets data cache: relational database sharing the client-
    /// friendly asset information with clients
    cacher: FileCache,

    /// Unmarshaller instance used for parsing RPC request
    unmarshaller: Unmarshaller<Request>,

    /// Unmarshaller instance used for parsing RPC request
    reply_unmarshaller: Unmarshaller<Reply>,
}

impl Runtime {
    /// Internal function for avoiding index-implementation specific function
    /// use and reduce number of errors. Cacher may be switched with compile
    /// configuration options and, thus, we need to make sure that the structure
    /// we use corresponds to certain trait and not specific type.
    fn cache(&self) -> &impl Cache {
        &self.cacher
    }

    pub fn init(config: Config) -> Result<Self, BootstrapError> {
        let cacher = FileCache::new(FileCacheConfig {
            data_dir: PathBuf::from(&config.cache),
            data_format: config.format,
        })
        .map_err(|err| {
            error!("{}", err);
            err
        })?;

        let session_rpc = session::Raw::with_zmq_unencrypted(
            ZmqType::Rep,
            &config.rpc_endpoint,
            None,
            None,
        )?;

        let session_pub = session::Raw::with_zmq_unencrypted(
            ZmqType::Pub,
            &config.pub_endpoint,
            None,
            None,
        )?;

        let stash_rpc = session::Raw::with_zmq_unencrypted(
            ZmqType::Req,
            &config.stash_rpc,
            None,
            None,
        )?;

        let stash_sub = session::Raw::with_zmq_unencrypted(
            ZmqType::Sub,
            &config.stash_sub,
            None,
            None,
        )?;

        Ok(Self {
            config,
            session_rpc,
            session_pub,
            stash_rpc,
            stash_sub,
            cacher,
            unmarshaller: Request::create_unmarshaller(),
            reply_unmarshaller: Reply::create_unmarshaller(),
        })
    }
}

impl TryService for Runtime {
    type ErrorType = RuntimeError;

    fn try_run_loop(mut self) -> Result<(), RuntimeError> {
        debug!("Registering RGB20 schema");
        self.register_schema().map_err(|_| {
            error!("Unable to register RGB20 schema");
            RuntimeError::Internal(
                "Unable to register RGB20 schema".to_string(),
            )
        })?;

        loop {
            match self.run() {
                Ok(_) => debug!("API request processing complete"),
                Err(err) => {
                    error!("Error processing API request: {}", err);
                    Err(err)?;
                }
            }
        }
    }
}

impl Runtime {
    fn run(&mut self) -> Result<(), RuntimeError> {
        trace!("Awaiting for ZMQ RPC requests...");
        let raw = self.session_rpc.recv_raw_message()?;
        let reply = self.rpc_process(raw).unwrap_or_else(|err| err);
        trace!("Preparing ZMQ RPC reply: {:?}", reply);
        let data = reply.serialize();
        trace!(
            "Sending {} bytes back to the client over ZMQ RPC",
            data.len()
        );
        self.session_rpc.send_raw_message(&data)?;
        Ok(())
    }

    fn rpc_process(&mut self, raw: Vec<u8>) -> Result<Reply, Reply> {
        trace!(
            "Got {} bytes over ZMQ RPC: {:?}",
            raw.len(),
            raw.to_bech32data()
        );
        let message = &*self.unmarshaller.unmarshall(&raw).map_err(|err| {
            error!("Error unmarshalling the data: {}", err);
            ServiceError::from_rpc(
                ServiceErrorSource::Contract(s!("fungible")),
                err,
            )
        })?;
        debug!("Received ZMQ RPC request: {:?}", message);
        Ok(match message {
            Request::Issue(issue) => self.rpc_issue(issue),
            Request::Transfer(transfer) => self.rpc_transfer(transfer),
            Request::Validate(consignment) => self.rpc_validate(consignment),
            Request::Accept(accept) => self.rpc_accept(accept),
            Request::Forget(outpoint) => self.rpc_forget(outpoint),
            Request::ImportAsset(genesis) => self.rpc_import_asset(genesis),
            Request::ExportAsset(asset_id) => self.rpc_export_asset(asset_id),
            Request::Sync(data_format) => self.rpc_sync(*data_format),
            Request::Assets(outpoint) => self.rpc_outpoint_assets(*outpoint),
            Request::Allocations(contract_id) => {
                self.rpc_asset_allocations(*contract_id)
            }
        }
        .map_err(|err| ServiceError::contract(err, "fungible"))?)
    }

    fn rpc_issue(
        &mut self,
        issue: &Issue,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got ISSUE {}", issue);

        let issue = issue.clone();
        let precision = issue.precision;
        let (asset, genesis) = rgb20::issue(
            self.config.network.clone(),
            issue.ticker,
            issue.name,
            issue.description,
            issue.precision,
            issue
                .allocation
                .into_iter()
                .map(|OutpointCoins { coins, outpoint }| {
                    (outpoint, AccountingAmount::transmutate(precision, coins))
                })
                .collect(),
            issue.inflation.into_iter().fold(
                BTreeMap::new(),
                |mut map, OutpointCoins { coins, outpoint }| {
                    // We may have only a single secondary issuance right per
                    // outpoint, so folding all outpoints
                    let coins = AccountingAmount::transmutate(precision, coins);
                    map.entry(outpoint)
                        .and_modify(|amount| *amount += coins)
                        .or_insert(coins);
                    map
                },
            ),
            issue.renomination,
            issue.epoch,
        )?;

        self.import_asset(asset.clone(), genesis)?;

        // TODO: Send push request to client informing about cache update

        Ok(Reply::Asset(asset))
    }

    fn rpc_transfer(
        &mut self,
        transfer: &TransferApi,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got TRANSFER {}", transfer);

        // TODO: Check inputs that they really exist and have sufficient amount
        //       of asset for the transfer operation

        trace!("Looking for asset information");
        let mut asset = self.cacher.asset(transfer.contract_id)?.clone();
        debug!("Transferring asset {}", asset);

        trace!("Preparing state transition");
        let transition = rgb20::transfer(
            &mut asset,
            transfer.inputs.clone(),
            transfer.ours.clone(),
            transfer.theirs.clone(),
        )?;
        debug!("State transition: {}", transition);

        trace!("Requesting consignment from stash daemon");
        let reply = self.consign(ConsignRequest {
            contract_id: transfer.contract_id,
            inputs: transfer.inputs.clone(),
            transition,
            // TODO: Collect blank state transitions and pass it here
            other_transition_ids: bmap![],
            outpoints: transfer
                .theirs
                .iter()
                .map(|o| (o.seal_confidential))
                .collect(),
            psbt: transfer.psbt.clone(),
        })?;

        Ok(reply)
    }

    fn rpc_validate(
        &mut self,
        consignment: &Consignment,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got VALIDATE");
        self.validate(consignment.clone())
    }

    fn rpc_accept(
        &mut self,
        accept: &AcceptApi,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got ACCEPT");
        Ok(self.accept(accept.clone())?)
    }

    fn rpc_forget(
        &mut self,
        outpoint: &OutPoint,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got FORGET");
        Ok(self.forget(outpoint.clone())?)
    }

    fn rpc_sync(
        &mut self,
        data_format: FileFormat,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got SYNC");
        let data = self.cacher.export(Some(data_format))?;
        Ok(Reply::Sync(reply::SyncFormat(data_format, data)))
    }

    fn rpc_outpoint_assets(
        &mut self,
        outpoint: OutPoint,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got ASSETS");
        let data = self.cacher.outpoint_assets(outpoint)?;
        Ok(Reply::OutpointAssets(data))
    }

    fn rpc_asset_allocations(
        &mut self,
        contract_id: ContractId,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got ALLOCATIONS");
        let data = self.cacher.asset_allocations(contract_id)?;
        Ok(Reply::AssetAllocations(data))
    }

    fn rpc_import_asset(
        &mut self,
        genesis: &Genesis,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got IMPORT_ASSET");
        self.import_asset(Asset::try_from(genesis.clone())?, genesis.clone())?;
        Ok(Reply::Success)
    }

    fn rpc_export_asset(
        &mut self,
        asset_id: &ContractId,
    ) -> Result<Reply, ServiceErrorDomain> {
        debug!("Got EXPORT_ASSET");
        let genesis = self.export_asset(asset_id.clone())?;
        Ok(Reply::Genesis(genesis))
    }

    fn register_schema(&mut self) -> Result<(), ServiceErrorDomain> {
        match self
            .stash_req_rep(rpc::stash::Request::AddSchema(schema::schema()))?
        {
            Reply::Success => Ok(()),
            _ => Err(ServiceErrorDomain::Api(ApiErrorType::UnexpectedReply)),
        }
    }

    fn import_asset(
        &mut self,
        asset: Asset,
        genesis: Genesis,
    ) -> Result<bool, ServiceErrorDomain> {
        match self.stash_req_rep(rpc::stash::Request::AddGenesis(genesis))? {
            Reply::Success => Ok(self.cacher.add_asset(asset)?),
            _ => Err(ServiceErrorDomain::Api(ApiErrorType::UnexpectedReply)),
        }
    }

    fn export_asset(
        &mut self,
        asset_id: ContractId,
    ) -> Result<Genesis, ServiceErrorDomain> {
        match self.stash_req_rep(rpc::stash::Request::ReadGenesis(asset_id))? {
            Reply::Genesis(genesis) => Ok(genesis.clone()),
            _ => Err(ServiceErrorDomain::Api(ApiErrorType::UnexpectedReply)),
        }
    }

    fn consign(
        &mut self,
        consign_req: ConsignRequest,
    ) -> Result<Reply, ServiceErrorDomain> {
        let reply =
            self.stash_req_rep(rpc::stash::Request::Consign(consign_req))?;
        if let Reply::Transfer(_) = reply {
            Ok(reply)
        } else {
            Err(ServiceErrorDomain::Api(ApiErrorType::UnexpectedReply))
        }
    }

    fn validate(
        &mut self,
        consignment: Consignment,
    ) -> Result<Reply, ServiceErrorDomain> {
        let reply =
            self.stash_req_rep(rpc::stash::Request::Validate(consignment))?;

        match reply {
            Reply::ValidationStatus(_) => Ok(reply),
            _ => Err(ServiceErrorDomain::Api(ApiErrorType::UnexpectedReply)),
        }
    }

    fn accept(
        &mut self,
        accept: AcceptApi,
    ) -> Result<Reply, ServiceErrorDomain> {
        let reply =
            self.stash_req_rep(rpc::stash::Request::Merge(MergeRequest {
                consignment: accept.consignment.clone(),
                reveal_outpoints: accept.reveal_outpoints.clone(),
            }))?;
        if let Reply::Success = reply {
            let asset_id = accept.consignment.genesis.contract_id();
            let mut asset = if self.cacher.has_asset(asset_id)? {
                self.cacher.asset(asset_id)?.clone()
            } else {
                Asset::try_from(accept.consignment.genesis)?
            };

            for (_, transition) in &accept.consignment.state_transitions {
                let set =
                    transition.owned_rights_by_type(*OwnedRightsType::Assets);
                for variant in set {
                    if let Assignments::DiscreteFiniteField(set) = variant {
                        for (index, assignment) in set.into_iter().enumerate() {
                            if let Some(seal) =
                                accept.reveal_outpoints.iter().find(|op| {
                                    op.conceal()
                                        == assignment
                                            .seal_definition_confidential()
                                })
                            {
                                if let Some(assigned_state) =
                                    assignment.assigned_state()
                                {
                                    asset.add_allocation(
                                        seal.clone().into(),
                                        transition.node_id(),
                                        index as u16,
                                        assigned_state.clone(),
                                    );
                                } else {
                                    Err(ServiceErrorDomain::Internal(
                                        "Consignment structure is broken"
                                            .to_string(),
                                    ))?
                                }
                            }
                        }
                    }
                }
            }

            self.cacher.add_asset(asset)?;
            Ok(reply)
        } else if let Reply::Failure(_) = &reply {
            Ok(reply)
        } else {
            Err(ServiceErrorDomain::Api(ApiErrorType::UnexpectedReply))
        }
    }

    fn forget(
        &mut self,
        outpoint: OutPoint,
    ) -> Result<Reply, ServiceErrorDomain> {
        let mut removal_list = Vec::<_>::new();
        let assets = self
            .cacher
            .assets()?
            .into_iter()
            .map(Clone::clone)
            .collect::<Vec<_>>();
        for asset in assets {
            let mut asset = asset.clone();
            for allocation in asset.clone().allocations(&outpoint) {
                asset.remove_allocation(
                    outpoint,
                    *allocation.node_id(),
                    *allocation.index(),
                    allocation.confidential_amount().clone(),
                );
                removal_list.push((*allocation.node_id(), *allocation.index()));
            }
            self.cacher.add_asset(asset)?;
        }
        if removal_list.is_empty() {
            return Ok(Reply::Nothing);
        }

        let reply =
            self.stash_req_rep(rpc::stash::Request::Forget(removal_list))?;

        match reply {
            Reply::Success | Reply::Failure(_) => Ok(reply),
            _ => Err(ServiceErrorDomain::Api(ApiErrorType::UnexpectedReply)),
        }
    }

    fn stash_req_rep(
        &mut self,
        request: rpc::stash::Request,
    ) -> Result<Reply, ServiceErrorDomain> {
        let data = request.serialize();
        trace!(
            "Sending {} bytes to stashd: {}",
            data.len(),
            data.to_bech32data()
        );
        self.stash_rpc.send_raw_message(data.borrow())?;
        let raw = self.stash_rpc.recv_raw_message()?;
        let reply = &*self.reply_unmarshaller.unmarshall(&raw)?.clone();
        if let Reply::Failure(ref failmsg) = reply {
            error!("Stash daemon has returned failure code: {}", failmsg);
            Err(ServiceErrorDomain::Stash)?
        }
        Ok(reply.clone())
    }
}

pub fn main_with_config(config: Config) -> Result<(), BootstrapError> {
    let runtime = Runtime::init(config)?;
    runtime.run_or_panic("Fungible contract runtime");

    unreachable!()
}
