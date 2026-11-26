// SPDX-FileCopyrightText: 2024-2026 P2Poolv2 Developers (see AUTHORS)
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::stratum::difficulty_adjuster::DifficultyAdjusterTrait;
use crate::stratum::error::Error;
use crate::stratum::messages::{Id, Message, MiningConfigure, Request, Response};
use crate::stratum::session::Session;
use tracing::debug;

/// Handle the "mining.configure" message.
///
/// For the version-rolling extension the reply mask is the BIP 310
/// intersection of the configured server mask and the miner's offer; the
/// negotiated mask replaces `session.version_mask` for the rest of the
/// session. Extensions other than version-rolling get no reply.
pub async fn handle_configure<'a, D: DifficultyAdjusterTrait>(
    request: Request<'a>,
    session: &mut Session<D>,
) -> Result<Vec<Message<'a>>, Error> {
    debug!("Handling mining.configure message");
    match request {
        Request::MiningConfigureRequest(configure) => {
            debug!("Received mining.configure request: {:?}", configure);
            handle_mining_configure(configure, session)
        }
        _ => Err(Error::InvalidParams(
            "Invalid method for mining.configure".into(),
        )),
    }
}

fn handle_mining_configure<'a>(
    request: MiningConfigure<'a>,
    session: &mut Session<impl DifficultyAdjusterTrait>,
) -> Result<Vec<Message<'a>>, Error> {
    let MiningConfigure { id, params, .. } = request;
    let (extensions, configure_params) = &*params;
    if !extensions
        .iter()
        .any(|extension| extension.as_str() == "version-rolling")
    {
        // return Ok, so we don't disconnect client. Also, we don't send any message back for unsupported configure methods.
        return Ok(vec![]);
    }

    let miner_mask = match configure_params.version_rolling_mask.as_deref() {
        // BIP 310: an absent mask defaults to "ffffffff".
        None => u32::MAX,
        Some(mask) => match u32::from_str_radix(mask, 16) {
            Ok(miner_mask) => miner_mask,
            Err(_) => return Ok(vec![version_rolling_decline(id)]),
        },
    };

    let negotiated = session.configured_version_mask as u32 & miner_mask;
    if negotiated == 0 {
        // No common bits: decline the extension, leave the session mask untouched.
        return Ok(vec![version_rolling_decline(id)]);
    }

    session.version_mask = negotiated as i32;
    Ok(vec![Message::Response(Response::new_ok(
        Some(id),
        serde_json::json!({
            "version-rolling": true,
            "version-rolling.mask": format!("{negotiated:08x}"),
        }),
    ))])
}

/// BIP 310 decline of the version-rolling extension.
fn version_rolling_decline<'a>(id: Id) -> Message<'a> {
    Message::Response(Response::new_ok(
        Some(id),
        serde_json::json!({ "version-rolling": false }),
    ))
}

#[cfg(test)]
mod mining_configure_response_tests {
    use p2poolv2_config::DEFAULT_VERSION_MASK;

    use std::borrow::Cow;

    use super::*;
    use crate::{
        stratum::difficulty_adjuster::DifficultyAdjuster,
        stratum::messages::{Id, Message, MiningConfigure, MiningConfigureParams},
    };

    #[tokio::test]
    async fn test_handle_configure_valid_request() {
        let message = MiningConfigure::new_version_rolling_configure(
            1,
            Some("ffffffff".to_string()),
            None,
            None,
        );

        let mut session =
            Session::<DifficultyAdjuster>::new(1, 1, Some(1000), DEFAULT_VERSION_MASK);

        let result = handle_configure(message, &mut session).await;
        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);

        match &messages[0] {
            Message::Response(response) => {
                assert_eq!(response.id, Some(Id::Number(1)));
                assert!(response.error.is_none());
                assert!(response.result.is_some());
                let result = response.result.as_ref().unwrap();
                assert_eq!(
                    result,
                    &serde_json::json!({
                        "version-rolling": true,
                        "version-rolling.mask": "1fffffe0"
                    })
                );
            }
            _ => panic!("Expected a Response message"),
        }
    }

    #[tokio::test]
    async fn test_configure_replies_bip320_offer_to_bip323_server() {
        let message = MiningConfigure::new_version_rolling_configure(
            1,
            Some("1fffe000".to_string()),
            None,
            None,
        );

        let mut session = Session::<DifficultyAdjuster>::new(1, 1, None, 0x1fffffe0);

        let result = handle_configure(message, &mut session).await;
        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);

        match &messages[0] {
            Message::Response(response) => {
                assert_eq!(response.id, Some(Id::Number(1)));
                assert!(response.error.is_none());
                assert_eq!(
                    response.result.as_ref().unwrap(),
                    &serde_json::json!({
                        "version-rolling": true,
                        "version-rolling.mask": "1fffe000"
                    })
                );
            }
            _ => panic!("Expected a Response message"),
        }
        assert_eq!(session.version_mask, 0x1fffe000);
        assert_eq!(session.configured_version_mask, 0x1fffffe0);
    }

    #[tokio::test]
    async fn test_configure_replies_server_mask_when_miner_omits_mask() {
        let message = MiningConfigure::new_version_rolling_configure(1, None, None, None);

        let mut session = Session::<DifficultyAdjuster>::new(1, 1, None, 0x1fffffe0);

        let result = handle_configure(message, &mut session).await;
        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);

        match &messages[0] {
            Message::Response(response) => {
                assert!(response.error.is_none());
                assert_eq!(
                    response.result.as_ref().unwrap(),
                    &serde_json::json!({
                        "version-rolling": true,
                        "version-rolling.mask": "1fffffe0"
                    })
                );
            }
            _ => panic!("Expected a Response message"),
        }
        assert_eq!(session.version_mask, 0x1fffffe0);
        assert_eq!(session.configured_version_mask, 0x1fffffe0);
    }

    #[tokio::test]
    async fn test_configure_sends_no_reply_without_version_rolling_extension() {
        let request = Request::MiningConfigureRequest(MiningConfigure {
            id: Id::Number(1),
            method: Cow::Owned("mining.configure".to_string()),
            params: Cow::Owned((
                vec!["minimum-difficulty".to_string()],
                MiningConfigureParams {
                    minimum_difficulty_value: Some(100),
                    version_rolling_mask: Cow::Owned(None),
                    version_rolling_min_bit_count: None,
                },
            )),
        });

        let mut session = Session::<DifficultyAdjuster>::new(1, 1, None, 0x1fffffe0);

        let result = handle_configure(request, &mut session).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_configure_declines_when_intersection_is_zero() {
        let message = MiningConfigure::new_version_rolling_configure(
            1,
            Some("00000001".to_string()),
            None,
            None,
        );

        let mut session = Session::<DifficultyAdjuster>::new(1, 1, None, 0x1fffffe0);

        let result = handle_configure(message, &mut session).await;
        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);

        match &messages[0] {
            Message::Response(response) => {
                assert_eq!(
                    response.result.as_ref().unwrap(),
                    &serde_json::json!({ "version-rolling": false })
                );
            }
            _ => panic!("Expected a Response message"),
        }
        assert_eq!(session.version_mask, 0x1fffffe0);
        assert_eq!(session.configured_version_mask, 0x1fffffe0);
    }

    #[tokio::test]
    async fn test_configure_declines_on_malformed_mask() {
        let message =
            MiningConfigure::new_version_rolling_configure(1, Some("xyz".to_string()), None, None);

        let mut session = Session::<DifficultyAdjuster>::new(1, 1, None, 0x1fffffe0);

        let result = handle_configure(message, &mut session).await;
        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);

        match &messages[0] {
            Message::Response(response) => {
                assert_eq!(
                    response.result.as_ref().unwrap(),
                    &serde_json::json!({ "version-rolling": false })
                );
            }
            _ => panic!("Expected a Response message"),
        }
        assert_eq!(session.version_mask, 0x1fffffe0);
        assert_eq!(session.configured_version_mask, 0x1fffffe0);
    }
}
