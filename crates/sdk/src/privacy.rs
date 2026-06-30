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
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct PrivacyPreflightConfirmation {
    pub fields: Vec<ProductSensitivityField>,
}

#[cfg(feature = "runtime")]
impl PrivacyPreflightConfirmation {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn confirm(mut self, field: ProductSensitivityField) -> Self {
        self.fields.push(field);
        self.fields.sort();
        self.fields.dedup();
        self
    }

    pub fn confirms(&self, field: ProductSensitivityField) -> bool {
        self.fields.contains(&field)
    }
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

    pub fn require_public_publish_allowed(
        &self,
        operation: impl Into<String>,
        confirmation: &PrivacyPreflightConfirmation,
    ) -> Result<(), crate::error::RadrootsSdkError> {
        match self.status {
            PrivacyPreflightStatus::Ok => Ok(()),
            PrivacyPreflightStatus::ForbiddenPublicFields => {
                Err(crate::error::RadrootsSdkError::PrivacyPreflight {
                    operation: operation.into(),
                    status: self.status,
                    fields: self.fields.clone(),
                })
            }
            PrivacyPreflightStatus::ExplicitConfirmationRequired => {
                let missing_fields = self
                    .fields
                    .iter()
                    .copied()
                    .filter(|field| privacy_field_requires_confirmation(*field))
                    .filter(|field| !confirmation.confirms(*field))
                    .collect::<Vec<_>>();
                if missing_fields.is_empty() {
                    Ok(())
                } else {
                    Err(crate::error::RadrootsSdkError::PrivacyPreflight {
                        operation: operation.into(),
                        status: self.status,
                        fields: missing_fields,
                    })
                }
            }
        }
    }
}

#[cfg(feature = "runtime")]
fn privacy_field_requires_confirmation(field: ProductSensitivityField) -> bool {
    matches!(field, ProductSensitivityField::PublicButSensitiveNotes)
}

#[cfg(test)]
#[cfg(feature = "runtime")]
mod tests {
    use super::{
        PrivacyPreflightConfirmation, PrivacyPreflightReceipt, PrivacyPreflightStatus,
        ProductSensitivityField,
    };

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

    #[test]
    fn privacy_confirmation_allows_only_confirmable_public_fields() {
        let confirmation = PrivacyPreflightConfirmation::new()
            .confirm(ProductSensitivityField::PublicButSensitiveNotes);
        PrivacyPreflightReceipt::evaluate([ProductSensitivityField::PublicButSensitiveNotes])
            .require_public_publish_allowed("trade.test", &confirmation)
            .expect("confirmed public note");
        PrivacyPreflightReceipt::evaluate([
            ProductSensitivityField::PublicButSensitiveNotes,
            ProductSensitivityField::ProtocolMinimizedInventoryFields,
        ])
        .require_public_publish_allowed("trade.test", &confirmation)
        .expect("confirmed public note with protocol inventory");

        let missing =
            PrivacyPreflightReceipt::evaluate([ProductSensitivityField::PublicButSensitiveNotes])
                .require_public_publish_allowed("trade.test", &PrivacyPreflightConfirmation::new())
                .expect_err("missing confirmation");
        assert_eq!(missing.code(), "privacy_preflight");

        let forbidden = PrivacyPreflightReceipt::evaluate([
            ProductSensitivityField::SensitiveFulfillmentDetails,
        ])
        .require_public_publish_allowed("trade.test", &confirmation)
        .expect_err("forbidden cannot be confirmed");
        assert_eq!(forbidden.code(), "privacy_preflight");
    }
}
