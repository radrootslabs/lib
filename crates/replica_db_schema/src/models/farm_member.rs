use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct FarmMember {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub farm_id: String,
    pub member_pubkey: String,
    pub role: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberFields {
    pub farm_id: String,
    pub member_pubkey: String,
    pub role: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberFieldsPartial {
    pub farm_id: Option<serde_json::Value>,
    pub member_pubkey: Option<serde_json::Value>,
    pub role: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub farm_id: Option<String>,
    pub member_pubkey: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FarmMemberQueryBindValues {
    Id { id: String },
    FarmId { farm_id: String },
    MemberPubkey { member_pubkey: String },
}
impl FarmMemberQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::FarmId { farm_id } => ("farm_id", Value::from(farm_id.clone())),
            Self::MemberPubkey { member_pubkey } => {
                ("member_pubkey", Value::from(member_pubkey.clone()))
            }
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
            Self::FarmId { farm_id } => farm_id.clone(),
            Self::MemberPubkey { member_pubkey } => member_pubkey.clone(),
        }
    }
}

pub struct IFarmMemberCreateTs;
pub type IFarmMemberCreate = IFarmMemberFields;
pub struct IFarmMemberCreateResolveTs;
pub type IFarmMemberCreateResolve = IResult<FarmMember>;
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberFindOneArgs {
    pub on: FarmMemberQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmMemberFindOne {
    On(IFarmMemberFindOneArgs),
}

pub struct IFarmMemberFindOneResolveTs;
pub type IFarmMemberFindOneResolve = IResult<Option<FarmMember>>;
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberFindManyArgs {
    pub filter: Option<IFarmMemberFieldsFilter>,
}
pub type IFarmMemberFindMany = IFarmMemberFindManyArgs;
pub struct IFarmMemberFindManyResolveTs;
pub type IFarmMemberFindManyResolve = IResultList<FarmMember>;
pub struct IFarmMemberDeleteTs;
pub type IFarmMemberDelete = IFarmMemberFindOne;
pub struct IFarmMemberDeleteResolveTs;
pub type IFarmMemberDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberUpdateArgs {
    pub on: FarmMemberQueryBindValues,
    pub fields: IFarmMemberFieldsPartial,
}
pub type IFarmMemberUpdate = IFarmMemberUpdateArgs;
pub struct IFarmMemberUpdateResolveTs;
pub type IFarmMemberUpdateResolve = IResult<FarmMember>;
