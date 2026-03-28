use crate::error::RadrootsSimplexSmpCryptoError;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsSimplexSmpRatchetRole {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpRatchetHeader {
    pub previous_sending_chain_length: u32,
    pub message_number: u32,
    pub dh_public_key: Vec<u8>,
    pub pq_public_key: Option<Vec<u8>>,
    pub pq_ciphertext: Option<Vec<u8>>,
}

impl RadrootsSimplexSmpRatchetHeader {
    pub fn validate(&self) -> Result<(), RadrootsSimplexSmpCryptoError> {
        if self.dh_public_key.is_empty() {
            return Err(RadrootsSimplexSmpCryptoError::MissingRatchetKey(
                "dh_public_key",
            ));
        }
        if self.pq_public_key.is_some() != self.pq_ciphertext.is_some() {
            return Err(RadrootsSimplexSmpCryptoError::IncompletePqHeader);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpRatchetState {
    pub role: RadrootsSimplexSmpRatchetRole,
    pub root_epoch: u64,
    pub previous_sending_chain_length: u32,
    pub sending_chain_length: u32,
    pub receiving_chain_length: u32,
    pub local_dh_public_key: Vec<u8>,
    pub remote_dh_public_key: Vec<u8>,
    pub current_pq_public_key: Option<Vec<u8>>,
    pub remote_pq_public_key: Option<Vec<u8>>,
    pub pending_outbound_pq_ciphertext: Option<Vec<u8>>,
    pub pending_inbound_pq_ciphertext: Option<Vec<u8>>,
    pub current_pq_shared_secret: Option<Vec<u8>>,
}

impl RadrootsSimplexSmpRatchetState {
    pub fn initiator(
        local_dh_public_key: Vec<u8>,
        remote_dh_public_key: Vec<u8>,
        remote_pq_public_key: Option<Vec<u8>>,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        validate_public_key(&local_dh_public_key)?;
        validate_public_key(&remote_dh_public_key)?;
        if let Some(key) = remote_pq_public_key.as_deref() {
            validate_public_key(key)?;
        }

        Ok(Self {
            role: RadrootsSimplexSmpRatchetRole::Initiator,
            root_epoch: 0,
            previous_sending_chain_length: 0,
            sending_chain_length: 0,
            receiving_chain_length: 0,
            local_dh_public_key,
            remote_dh_public_key,
            current_pq_public_key: None,
            remote_pq_public_key,
            pending_outbound_pq_ciphertext: None,
            pending_inbound_pq_ciphertext: None,
            current_pq_shared_secret: None,
        })
    }

    pub fn responder(
        local_dh_public_key: Vec<u8>,
        remote_dh_public_key: Vec<u8>,
        local_pq_public_key: Option<Vec<u8>>,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        validate_public_key(&local_dh_public_key)?;
        validate_public_key(&remote_dh_public_key)?;
        if let Some(key) = local_pq_public_key.as_deref() {
            validate_public_key(key)?;
        }

        Ok(Self {
            role: RadrootsSimplexSmpRatchetRole::Responder,
            root_epoch: 0,
            previous_sending_chain_length: 0,
            sending_chain_length: 0,
            receiving_chain_length: 0,
            local_dh_public_key,
            remote_dh_public_key,
            current_pq_public_key: local_pq_public_key,
            remote_pq_public_key: None,
            pending_outbound_pq_ciphertext: None,
            pending_inbound_pq_ciphertext: None,
            current_pq_shared_secret: None,
        })
    }

    pub fn stage_outbound_pq_step(
        &mut self,
        pq_public_key: Vec<u8>,
        pq_ciphertext: Vec<u8>,
        shared_secret: Vec<u8>,
    ) -> Result<(), RadrootsSimplexSmpCryptoError> {
        validate_public_key(&pq_public_key)?;
        if pq_ciphertext.is_empty() {
            return Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(0));
        }
        if shared_secret.is_empty() {
            return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(0));
        }

        self.current_pq_public_key = Some(pq_public_key);
        self.pending_outbound_pq_ciphertext = Some(pq_ciphertext);
        self.current_pq_shared_secret = Some(shared_secret);
        self.root_epoch = self.root_epoch.saturating_add(1);
        Ok(())
    }

    pub fn next_outbound_header(
        &mut self,
    ) -> Result<RadrootsSimplexSmpRatchetHeader, RadrootsSimplexSmpCryptoError> {
        validate_public_key(&self.local_dh_public_key)?;
        let header = RadrootsSimplexSmpRatchetHeader {
            previous_sending_chain_length: self.previous_sending_chain_length,
            message_number: self.sending_chain_length,
            dh_public_key: self.local_dh_public_key.clone(),
            pq_public_key: self.current_pq_public_key.clone(),
            pq_ciphertext: self.pending_outbound_pq_ciphertext.clone(),
        };
        header.validate()?;
        self.sending_chain_length = self.sending_chain_length.saturating_add(1);
        Ok(header)
    }

    pub fn apply_inbound_header(
        &mut self,
        header: &RadrootsSimplexSmpRatchetHeader,
        next_local_dh_public_key: Option<Vec<u8>>,
    ) -> Result<bool, RadrootsSimplexSmpCryptoError> {
        header.validate()?;
        let dh_advanced = header.dh_public_key != self.remote_dh_public_key;

        if dh_advanced {
            self.previous_sending_chain_length = self.sending_chain_length;
            self.sending_chain_length = 0;
            self.remote_dh_public_key = header.dh_public_key.clone();
            if let Some(next_local_key) = next_local_dh_public_key {
                validate_public_key(&next_local_key)?;
                self.local_dh_public_key = next_local_key;
            }
            self.root_epoch = self.root_epoch.saturating_add(1);
        } else if header.message_number < self.receiving_chain_length {
            return Err(RadrootsSimplexSmpCryptoError::RatchetMessageRegression {
                received: header.message_number,
                current: self.receiving_chain_length,
            });
        }

        self.receiving_chain_length = header.message_number.saturating_add(1);
        if let Some(public_key) = header.pq_public_key.as_ref() {
            self.remote_pq_public_key = Some(public_key.clone());
        }
        if let Some(ciphertext) = header.pq_ciphertext.as_ref() {
            self.pending_inbound_pq_ciphertext = Some(ciphertext.clone());
        }

        Ok(dh_advanced)
    }

    pub fn complete_inbound_pq_step(
        &mut self,
        shared_secret: Vec<u8>,
    ) -> Result<(), RadrootsSimplexSmpCryptoError> {
        if shared_secret.is_empty() {
            return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(0));
        }
        self.current_pq_shared_secret = Some(shared_secret);
        self.pending_inbound_pq_ciphertext = None;
        self.root_epoch = self.root_epoch.saturating_add(1);
        Ok(())
    }
}

fn validate_public_key(value: &[u8]) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if value.is_empty() {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(0));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_outbound_pq_state_and_emits_header() {
        let mut state = RadrootsSimplexSmpRatchetState::responder(
            b"bob-dh".to_vec(),
            b"alice-dh".to_vec(),
            Some(b"bob-pq".to_vec()),
        )
        .unwrap();
        state
            .stage_outbound_pq_step(
                b"bob-pq-next".to_vec(),
                b"ciphertext".to_vec(),
                b"shared-secret".to_vec(),
            )
            .unwrap();

        let header = state.next_outbound_header().unwrap();
        assert_eq!(header.message_number, 0);
        assert_eq!(header.pq_public_key, Some(b"bob-pq-next".to_vec()));
        assert_eq!(header.pq_ciphertext, Some(b"ciphertext".to_vec()));
        assert_eq!(state.sending_chain_length, 1);
    }

    #[test]
    fn applies_inbound_dh_and_pq_transition() {
        let mut state = RadrootsSimplexSmpRatchetState::initiator(
            b"alice-dh".to_vec(),
            b"bob-dh".to_vec(),
            Some(b"bob-pq".to_vec()),
        )
        .unwrap();
        state.sending_chain_length = 4;

        let advanced = state
            .apply_inbound_header(
                &RadrootsSimplexSmpRatchetHeader {
                    previous_sending_chain_length: 2,
                    message_number: 0,
                    dh_public_key: b"bob-dh-next".to_vec(),
                    pq_public_key: Some(b"bob-pq-next".to_vec()),
                    pq_ciphertext: Some(b"ciphertext".to_vec()),
                },
                Some(b"alice-dh-next".to_vec()),
            )
            .unwrap();

        assert!(advanced);
        assert_eq!(state.previous_sending_chain_length, 4);
        assert_eq!(state.sending_chain_length, 0);
        assert_eq!(state.receiving_chain_length, 1);
        assert_eq!(state.remote_pq_public_key, Some(b"bob-pq-next".to_vec()));
        assert_eq!(
            state.pending_inbound_pq_ciphertext,
            Some(b"ciphertext".to_vec())
        );

        state
            .complete_inbound_pq_step(b"shared-secret".to_vec())
            .unwrap();
        assert_eq!(
            state.current_pq_shared_secret,
            Some(b"shared-secret".to_vec())
        );
        assert_eq!(state.pending_inbound_pq_ciphertext, None);
    }

    #[test]
    fn rejects_incomplete_pq_header() {
        let header = RadrootsSimplexSmpRatchetHeader {
            previous_sending_chain_length: 0,
            message_number: 0,
            dh_public_key: b"dh".to_vec(),
            pq_public_key: Some(b"pq".to_vec()),
            pq_ciphertext: None,
        };

        let error = header.validate().unwrap_err();
        assert_eq!(error, RadrootsSimplexSmpCryptoError::IncompletePqHeader);
    }
}
