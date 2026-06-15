// Copyright (C) 2024-2026 P2Poolv2 Developers (see AUTHORS)
//
// This file is part of P2Poolv2
//
// P2Poolv2 is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.
//
// P2Poolv2 is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// P2Poolv2. If not, see <https://www.gnu.org/licenses/>.

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
