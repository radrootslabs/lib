#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProductSensitivityField {
    ExactLocation,
    SensitiveFulfillmentDetails,
    PublicButSensitiveNotes,
    ProtocolMinimizedInventoryFields,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrivacyPreflightStatus {
    Ok,
    ExplicitConfirmationRequired,
    ForbiddenPublicFields,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PrivacyPreflightReceipt {
    pub status: PrivacyPreflightStatus,
    pub fields: Vec<ProductSensitivityField>,
}

#[cfg(feature = "runtime")]
impl PrivacyPreflightReceipt {
    pub fn evaluate<I>(fields: I) -> Self
    where
        I: IntoIterator<Item = ProductSensitivityField>,
    {
        let mut fields = fields.into_iter().collect::<Vec<_>>();
        fields.sort();
        fields.dedup();
        let status = if fields.iter().any(|field| {
            matches!(
                field,
                ProductSensitivityField::ExactLocation
                    | ProductSensitivityField::SensitiveFulfillmentDetails
            )
        }) {
            PrivacyPreflightStatus::ForbiddenPublicFields
        } else if fields
            .iter()
            .any(|field| matches!(field, ProductSensitivityField::PublicButSensitiveNotes))
        {
            PrivacyPreflightStatus::ExplicitConfirmationRequired
        } else {
            PrivacyPreflightStatus::Ok
        };
        Self { status, fields }
    }
}

#[cfg(test)]
#[cfg(feature = "runtime")]
mod tests {
    use super::{PrivacyPreflightReceipt, PrivacyPreflightStatus, ProductSensitivityField};

    #[test]
    fn privacy_preflight_classifies_public_sensitivity() {
        let ok = PrivacyPreflightReceipt::evaluate([
            ProductSensitivityField::ProtocolMinimizedInventoryFields,
        ]);
        assert_eq!(ok.status, PrivacyPreflightStatus::Ok);

        let confirm =
            PrivacyPreflightReceipt::evaluate([ProductSensitivityField::PublicButSensitiveNotes]);
        assert_eq!(
            confirm.status,
            PrivacyPreflightStatus::ExplicitConfirmationRequired
        );

        let forbidden = PrivacyPreflightReceipt::evaluate([
            ProductSensitivityField::ExactLocation,
            ProductSensitivityField::SensitiveFulfillmentDetails,
        ]);
        assert_eq!(
            forbidden.status,
            PrivacyPreflightStatus::ForbiddenPublicFields
        );
    }
}
