use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct FarmMember {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub farm_id: String,
    pub member_pubkey: String,
    pub role: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberFields {
    pub farm_id: String,
    pub member_pubkey: String,
    pub role: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub farm_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub member_pubkey: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub role: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmMemberFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub farm_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub member_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub role: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum FarmMemberFindManyRel {

}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberCreate",
        type = "IFarmMemberFields"
    )
)]
pub struct IFarmMemberCreateTs;
pub type IFarmMemberCreate = IFarmMemberFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberCreateResolve",
        type = "IResult<FarmMember>"
    )
)]
pub struct IFarmMemberCreateResolveTs;
pub type IFarmMemberCreateResolve = IResult<FarmMember>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberFindOneArgs {
    pub on: FarmMemberQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberFindOneRelArgs {
    pub rel: FarmMemberFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmMemberFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmMemberFindOne {
    On(IFarmMemberFindOneArgs),
    Rel(IFarmMemberFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberFindOneResolve",
        type = "IResult<FarmMember>"
    )
)]
pub struct IFarmMemberFindOneResolveTs;
pub type IFarmMemberFindOneResolve = IResult<Option<FarmMember>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmMemberFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberFindManyArgs {
    pub filter: Option<IFarmMemberFieldsFilter>,
}
pub type IFarmMemberFindMany = IFarmMemberFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberFindManyResolve",
        type = "IResultList<FarmMember>"
    )
)]
pub struct IFarmMemberFindManyResolveTs;
pub type IFarmMemberFindManyResolve = IResultList<FarmMember>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberDelete",
        type = "IFarmMemberFindOne"
    )
)]
pub struct IFarmMemberDeleteTs;
pub type IFarmMemberDelete = IFarmMemberFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IFarmMemberDeleteResolveTs;
pub type IFarmMemberDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts", rename = "IFarmMemberUpdate"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmMemberUpdateArgs {
    pub on: FarmMemberQueryBindValues,
    pub fields: IFarmMemberFieldsPartial,
}
pub type IFarmMemberUpdate = IFarmMemberUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmMemberUpdateResolve",
        type = "IResult<FarmMember>"
    )
)]
pub struct IFarmMemberUpdateResolveTs;
pub type IFarmMemberUpdateResolve = IResult<FarmMember>;
