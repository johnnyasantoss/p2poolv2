// SPDX-FileCopyrightText: 2024-2026 P2Poolv2 Developers (see AUTHORS)
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::stratum::error::Error;
use crate::stratum::messages::SimpleRequest;
use crate::stratum::work::gbt::compute_merkle_root_from_branches;
use crate::stratum::work::tracker::JobDetails;
use bitcoin::blockdata::block::Header;
use bitcoin::consensus::Decodable;
use hex::FromHex;
use std::str::FromStr;
use tracing::{debug, info};

/// Share validation result
///
/// Captures the header and coinbase, with a boolean to signal if block meets bitcoin difficulty
pub struct ValidationResult {
    /// The block header built from the share components
    pub header: Header,
    /// The coinbase transaction built from share components
    pub coinbase: bitcoin::Transaction,
    /// Does the block meet bitcoin difficulty
    pub meets_bitcoin_difficulty: bool,
}

/// Build coinbase from the submitted share and block template components.
pub fn build_coinbase_from_components(
    coinbase1: &str,
    enonce1: &str,
    enonce2: &str,
    coinbase2: &str,
) -> Result<bitcoin::Transaction, Error> {
    // Add detailed logging to debug the issue
    let complete_tx = format!("{coinbase1}{enonce1}{enonce2}{coinbase2}");

    let tx_bytes = Vec::from_hex(&complete_tx)
        .map_err(|_| Error::InvalidParams("Invalid coinbase hex".into()))?;
    bitcoin::Transaction::consensus_decode(&mut std::io::Cursor::new(tx_bytes))
        .map_err(|_e| Error::InvalidParams("Failed to decode coinbase transaction".into()))
}

/// Composes the submitted version_bits into the header version with the
/// session's negotiated mask. BIP 310 requires rejecting a submit whose
/// version_bits fall outside the negotiated mask (`version_bits & ~mask == 0`).
/// If params don't include version_bits, it returns the original header version
fn apply_version_mask(
    header_version: i32,
    version_mask: i32,
    params: &[Option<String>],
) -> Result<i32, Error> {
    if params.len() > 5 {
        let version_bits_hex = params[5]
            .as_ref()
            .ok_or_else(|| Error::InvalidParams("Missing version bits".into()))?;
        let bits = i32::from_be_bytes(
            hex::decode(version_bits_hex)
                .map_err(|_| Error::InvalidParams("Failed to decode version bits hex".into()))?
                .as_slice()
                .try_into()
                .map_err(|_| Error::InvalidParams("Invalid version bits length".into()))?,
        );
        if bits & !version_mask != 0 {
            return Err(Error::InvalidParams(
                "Version bits outside negotiated version mask".into(),
            ));
        }
        Ok((header_version & !version_mask) | (bits & version_mask))
    } else {
        Ok(header_version)
    }
}

/// Validate the difficulty of a submitted share against the block template
///
/// We build the block header from received submission and the corresponding block template.
/// Then we check if the header's difficulty meets the target specified in the block template.
///
/// Returns ValidationResult if validation was error free, else the error.
pub fn validate_bitcoin_difficulty(
    job: &JobDetails,
    submission: &SimpleRequest<'_>,
    enonce1_hex: &str,
    version_mask: i32,
) -> Result<ValidationResult, Error> {
    let compact_target = bitcoin::CompactTarget::from_unprefixed_hex(&job.blocktemplate.bits)
        .map_err(|_| Error::InvalidParams("Failed to parse compact target".into()))?;
    let target = bitcoin::Target::from_compact(compact_target);

    let enonce2 = submission.params[2]
        .as_ref()
        .ok_or_else(|| Error::InvalidParams("Missing enonce2".into()))?
        .as_str();

    // build coinbase from submission
    let coinbase =
        build_coinbase_from_components(&job.coinbase1, enonce1_hex, enonce2, &job.coinbase2)
            .map_err(|_| Error::InvalidParams("Failed to build coinbase transaction".into()))?;

    debug!("Coinbase transaction: {:?}", coinbase);

    // Compute merkle root by walking the pre-computed branches along with the coinbase txid.
    let merkle_root =
        compute_merkle_root_from_branches(coinbase.compute_txid(), &job.template_merkle_branches);

    let ntime_str = submission.params[3]
        .as_ref()
        .ok_or_else(|| Error::InvalidParams("Missing ntime".into()))?;
    let n_time =
        u32::from_str_radix(ntime_str, 16).map_err(|_| Error::InvalidParams("Bad nTime".into()))?;

    let version = apply_version_mask(job.blocktemplate.version, version_mask, &submission.params)?;

    // build the block header from the block template and submission
    let header = Header {
        version: bitcoin::block::Version::from_consensus(version),
        prev_blockhash: bitcoin::BlockHash::from_str(&job.blocktemplate.previousblockhash).unwrap(),
        merkle_root,
        time: n_time,
        bits: compact_target,
        nonce: u32::from_str_radix(
            submission.params[4]
                .as_ref()
                .ok_or_else(|| Error::InvalidParams("Missing nonce".into()))?,
            16,
        )
        .map_err(|_| Error::InvalidParams("Bad nonce".into()))?,
    };

    let meets_bitcoin_difficulty = match header.validate_pow(target) {
        Ok(_) => {
            info!("Header meets current bitcoin target");
            true
        }
        Err(e) => {
            debug!("Header does not meet the bitcoin target: {}", e);
            false
        }
    };

    Ok(ValidationResult {
        header,
        coinbase,
        meets_bitcoin_difficulty,
    })
}

#[cfg(test)]
mod apply_version_mask_tests {
    use super::*;

    #[test]
    fn test_apply_version_mask_rejects_bits_outside_mask() {
        let params = vec![
            Some("worker".to_string()),
            Some("job_id".to_string()),
            Some("extranonce2".to_string()),
            Some("ntime".to_string()),
            Some("nonce".to_string()),
            Some("00000001".to_string()),
        ];

        let result = apply_version_mask(0x20000000, 0x1fffe000, &params);

        match result {
            Err(Error::InvalidParams(message)) => {
                assert_eq!(message, "Version bits outside negotiated version mask")
            }
            other => panic!("Expected InvalidParams error, got {:?}", other),
        }
    }

    #[test]
    fn test_apply_version_mask_accepts_bits_inside_mask() {
        let params = vec![
            Some("worker".to_string()),
            Some("job_id".to_string()),
            Some("extranonce2".to_string()),
            Some("ntime".to_string()),
            Some("nonce".to_string()),
            Some("083ac000".to_string()),
        ];

        let version = apply_version_mask(0x20000000, 0x1fffe000, &params).unwrap();

        assert_eq!(version, 0x283ac000);
    }
}
