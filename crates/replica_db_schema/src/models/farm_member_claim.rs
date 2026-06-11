use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct FarmMemberClaim {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub member_pubkey: String,
    pub farm_pubkey: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberClaimFields {
    pub member_pubkey: String,
    pub farm_pubkey: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberClaimFieldsPartial {
    pub member_pubkey: Option<serde_json::Value>,
    pub farm_pubkey: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberClaimFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub member_pubkey: Option<String>,
    pub farm_pubkey: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FarmMemberClaimQueryBindValues {
    Id { id: String },
    MemberPubkey { member_pubkey: String },
    FarmPubkey { farm_pubkey: String },
}
impl FarmMemberClaimQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::MemberPubkey { member_pubkey } => {
                ("member_pubkey", Value::from(member_pubkey.clone()))
            }
            Self::FarmPubkey { farm_pubkey } => ("farm_pubkey", Value::from(farm_pubkey.clone())),
        }
    }

    pub fn primary_key(&self) -> Option<String> {
        match self {
            Self::Id { id } => Some(id.clone()),
            _ => None,
        }
    }

    pub fn lookup_key(&self) -> String {
        match self {
            Self::Id { id } => id.clone(),
            Self::MemberPubkey { member_pubkey } => member_pubkey.clone(),
            Self::FarmPubkey { farm_pubkey } => farm_pubkey.clone(),
        }
    }
}

pub struct IFarmMemberClaimCreateTs;
pub type IFarmMemberClaimCreate = IFarmMemberClaimFields;
pub struct IFarmMemberClaimCreateResolveTs;
pub type IFarmMemberClaimCreateResolve = IResult<FarmMemberClaim>;
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberClaimFindOneArgs {
    pub on: FarmMemberClaimQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmMemberClaimFindOne {
    On(IFarmMemberClaimFindOneArgs),
}

pub struct IFarmMemberClaimFindOneResolveTs;
pub type IFarmMemberClaimFindOneResolve = IResult<Option<FarmMemberClaim>>;
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberClaimFindManyArgs {
    pub filter: Option<IFarmMemberClaimFieldsFilter>,
}
pub type IFarmMemberClaimFindMany = IFarmMemberClaimFindManyArgs;
pub struct IFarmMemberClaimFindManyResolveTs;
pub type IFarmMemberClaimFindManyResolve = IResultList<FarmMemberClaim>;
pub struct IFarmMemberClaimDeleteTs;
pub type IFarmMemberClaimDelete = IFarmMemberClaimFindOne;
pub struct IFarmMemberClaimDeleteResolveTs;
pub type IFarmMemberClaimDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberClaimUpdateArgs {
    pub on: FarmMemberClaimQueryBindValues,
    pub fields: IFarmMemberClaimFieldsPartial,
}
pub type IFarmMemberClaimUpdate = IFarmMemberClaimUpdateArgs;
pub struct IFarmMemberClaimUpdateResolveTs;
pub type IFarmMemberClaimUpdateResolve = IResult<FarmMemberClaim>;
