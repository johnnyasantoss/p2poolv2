// Copyright (C) 2024, 2026 P2Poolv2 Developers (see AUTHORS)
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

use std::error::Error;

use bitcoindrpc::BitcoindRpcClient;
use p2poolv2_lib::node::Config;
use tracing::debug;

/// Execute the gen-genesis command
pub async fn execute(
    config: &Config,
    public_key: Option<String>,
    network: &str,
) -> Result<(), Box<dyn Error>> {
    let bitcoinrpc_client = BitcoindRpcClient::new(
        &config.bitcoinrpc.url,
        &config.bitcoinrpc.username,
        &config.bitcoinrpc.password,
    )?;

    let best_blockhash = bitcoinrpc_client.getbestblockhash().await?;
    let best_height: u64 = bitcoinrpc_client.getblockstats(&best_blockhash).await?["height"]
        .to_string()
        .parse()?;

    debug!(%best_blockhash, %best_height, "Using current best block hash");

    let bitcoin_block_hex = bitcoinrpc_client.getblock(&best_blockhash).await?;
    let bitcoin_block: bitcoin::Block =
        bitcoin::consensus::deserialize(bitcoin_block_hex.as_bytes())?;

    let public_key = match public_key {
        Some(pk) => pk,
        None => "02ac493f2130ca56cb5c3a559860cef9a84f90b5a85dfe4ec6e6067eeee17f4d2d".into(),
    };

    let timestamp = bitcoin_block.header.time;

    println!("Bitcoin block hex (copy into the genesis file):");
    println!("{}", bitcoin_block_hex);

    println!();
    println!(
        "Add using ({timestamp}, {best_height}, include_str!(\"{network}.rs\").into()) at fn genesis_data"
    );
    println!("See function genesis_data at p2poolv2_lib/src/shares/genesis/mod.rs");

    Ok(())
}
