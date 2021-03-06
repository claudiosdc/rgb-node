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

use std::collections::BTreeMap;

use lnpbp::bitcoin::util::psbt::PartiallySignedTransaction as Psbt;
use lnpbp::bitcoin::OutPoint;
use lnpbp::bp::blind::{OutpointHash, OutpointReveal};
use lnpbp::rgb::{Consignment, ContractId, NodeId, Transition};

#[derive(Clone, Debug, Display, LnpApi)]
#[lnp_api(encoding = "strict")]
#[display(Debug)]
#[non_exhaustive]
pub enum Request {
    #[lnp_api(type = 0x0101)]
    AddSchema(::lnpbp::rgb::Schema),

    #[lnp_api(type = 0x0103)]
    ListSchemata(),

    #[lnp_api(type = 0x0105)]
    ReadSchema(::lnpbp::rgb::SchemaId),

    #[lnp_api(type = 0x0201)]
    AddGenesis(::lnpbp::rgb::Genesis),

    #[lnp_api(type = 0x0203)]
    ListGeneses(),

    #[lnp_api(type = 0x0205)]
    ReadGenesis(::lnpbp::rgb::ContractId),

    #[lnp_api(type = 0x0301)]
    ReadTransitions(Vec<::lnpbp::rgb::NodeId>),

    #[lnp_api(type = 0x0401)]
    Consign(crate::api::stash::ConsignRequest),

    #[lnp_api(type = 0x0403)]
    Validate(::lnpbp::rgb::Consignment),

    #[lnp_api(type = 0x0405)]
    Merge(crate::api::stash::MergeRequest),

    #[lnp_api(type = 0x0407)]
    Forget(Vec<(::lnpbp::rgb::NodeId, u16)>),
}

#[derive(Clone, StrictEncode, StrictDecode, Debug, Display)]
#[display(Debug)]
pub struct ConsignRequest {
    pub contract_id: ContractId,
    pub inputs: Vec<OutPoint>,
    pub transition: Transition,
    pub other_transition_ids: BTreeMap<ContractId, NodeId>,
    pub outpoints: Vec<OutpointHash>,
    pub psbt: Psbt,
}

#[derive(Clone, StrictEncode, StrictDecode, Debug, Display)]
#[display(Debug)]
pub struct MergeRequest {
    pub consignment: Consignment,
    pub reveal_outpoints: Vec<OutpointReveal>,
}
