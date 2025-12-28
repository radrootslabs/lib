use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct FarmMemberClaim {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub member_pubkey: String,
    pub farm_pubkey: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberClaimFields {
    pub member_pubkey: String,
    pub farm_pubkey: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberClaimFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub member_pubkey: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub farm_pubkey: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberClaimFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub member_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub farm_pubkey: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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
            Self::FarmPubkey { farm_pubkey } => {
                ("farm_pubkey", Value::from(farm_pubkey.clone()))
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
            Self::MemberPubkey { member_pubkey } => member_pubkey.clone(),
            Self::FarmPubkey { farm_pubkey } => farm_pubkey.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum FarmMemberClaimFindManyRel {

}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberClaimCreate",
        type = "IFarmMemberClaimFields"
    )
)]
pub struct IFarmMemberClaimCreateTs;
pub type IFarmMemberClaimCreate = IFarmMemberClaimFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberClaimCreateResolve",
        type = "IResult<FarmMemberClaim>"
    )
)]
pub struct IFarmMemberClaimCreateResolveTs;
pub type IFarmMemberClaimCreateResolve = IResult<FarmMemberClaim>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberClaimFindOneArgs {
    pub on: FarmMemberClaimQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberClaimFindOneRelArgs {
    pub rel: FarmMemberClaimFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmMemberClaimFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmMemberClaimFindOne {
    On(IFarmMemberClaimFindOneArgs),
    Rel(IFarmMemberClaimFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberClaimFindOneResolve",
        type = "IResult<FarmMemberClaim>"
    )
)]
pub struct IFarmMemberClaimFindOneResolveTs;
pub type IFarmMemberClaimFindOneResolve = IResult<Option<FarmMemberClaim>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmMemberClaimFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberClaimFindManyArgs {
    pub filter: Option<IFarmMemberClaimFieldsFilter>,
}
pub type IFarmMemberClaimFindMany = IFarmMemberClaimFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberClaimFindManyResolve",
        type = "IResultList<FarmMemberClaim>"
    )
)]
pub struct IFarmMemberClaimFindManyResolveTs;
pub type IFarmMemberClaimFindManyResolve = IResultList<FarmMemberClaim>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberClaimDelete",
        type = "IFarmMemberClaimFindOne"
    )
)]
pub struct IFarmMemberClaimDeleteTs;
pub type IFarmMemberClaimDelete = IFarmMemberClaimFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberClaimDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IFarmMemberClaimDeleteResolveTs;
pub type IFarmMemberClaimDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts", rename = "IFarmMemberClaimUpdate"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberClaimUpdateArgs {
    pub on: FarmMemberClaimQueryBindValues,
    pub fields: IFarmMemberClaimFieldsPartial,
}
pub type IFarmMemberClaimUpdate = IFarmMemberClaimUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberClaimUpdateResolve",
        type = "IResult<FarmMemberClaim>"
    )
)]
pub struct IFarmMemberClaimUpdateResolveTs;
pub type IFarmMemberClaimUpdateResolve = IResult<FarmMemberClaim>;
