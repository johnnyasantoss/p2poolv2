// SPDX-FileCopyrightText: 2024-2026 P2Poolv2 Developers (see AUTHORS)
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! IPC communication with Bitcoin node using the Cap'n'Proto and libmultiprocess
//! to support fast template fetching

use std::{error::Error, path::Path};

use bitcoin::Network;
use bitcoin_capnp::BitcoinIpc;

use crate::stratum::work::notify::NotifySender;

// TODO: rename to capnp_mp / something like that

/// Listens to templates changes from the bitcoin node using capnproto
///
/// # Examples
///
/// ```
/// use p2poolv2_lib::stratum::work::ipc::start_ipc;
///
/// let result = start_capnp_rpc(result_tx, timeout_secs, network);
/// assert_eq!(result, );
/// ```
pub async fn start_capnp_rpc(
    result_tx: NotifySender,
    timeout_secs: u32,
    network: Network,
    node_socket_path: &Path,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if !node_socket_path.try_exists()? {
        return Err("Capnproto RPC unix socket does not exist".into());
    }

    let mut ipc = BitcoinIpc::new(node_socket_path)?;
    ipc.start_monitor();

    result_tx.send(template);

    todo!()
}
