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

use anyhow::{Context, Result, anyhow, ensure};
use bitcoin::{
    self, Block, CompressedPublicKey, Network,
    consensus::{deserialize, encode::serialize_hex},
};
use bitcoindrpc::BitcoindRpcClient;
use hex::decode;
use p2poolv2_lib::{
    node::Config,
    shares::{
        genesis::{DEFAULT_MINER_PK, GenesisData},
        share_block::ShareBlock,
    },
};
use tracing::info;

/// Execute the gen-genesis command
pub async fn execute(config: &Config, public_key: Option<String>, network: &str) -> Result<()> {
    let rpc_client = BitcoindRpcClient::new(
        &config.bitcoinrpc.url,
        &config.bitcoinrpc.username,
        &config.bitcoinrpc.password,
    )
    .context("Failed to create Bitcoin RPC client from the configured credentials")?;

    let best_blockhash = rpc_client
        .getbestblockhash()
        .await
        .context("Failed to fetch the current best Bitcoin block hash")?;
    let bitcoin_height: u64 =
        rpc_client
            .getblockstats(&best_blockhash)
            .await
            .with_context(|| format!("Failed to fetch Bitcoin block stats for {best_blockhash}"))?
            ["height"]
            .to_string()
            .parse()
            .context("Bitcoin RPC returned block stats without a valid numeric height")?;

    ensure!(
        bitcoin_height > 0,
        "Block height must be greater than 0. Node in IBD?"
    );

    info!(%best_blockhash, %bitcoin_height, "Using current best block hash");

    let bitcoin_block_hex = rpc_client
        .getblock(&best_blockhash)
        .await
        .with_context(|| format!("Failed to fetch Bitcoin block {best_blockhash}"))?
        .trim_matches('"')
        .to_string();
    let bitcoin_block_bytes = decode(&bitcoin_block_hex)
        .with_context(|| format!("Bitcoin RPC returned non-hex block data for {best_blockhash}"))?;
    let bitcoin_block: Block = deserialize(&bitcoin_block_bytes)
        .with_context(|| format!("Bitcoin RPC returned invalid block data for {best_blockhash}"))?;

    let public_key = public_key.unwrap_or_else(|| DEFAULT_MINER_PK.into());
    public_key
        .parse::<CompressedPublicKey>()
        .context("Miner public key must be a compressed public key encoded as 33-byte hex")?;

    let timestamp = bitcoin_block.header.time;
    let genesis_data = GenesisData {
        public_key,
        bitcoin_block_hex,
        bitcoin_height,
        timestamp,
    };
    let network: Network = Network::from_core_arg(network).with_context(|| {
        format!(
            "Invalid Bitcoin network '{network}'. Expected bitcoin, testnet4, signet, or regtest"
        )
    })?;
    let block = ShareBlock::build_genesis(&genesis_data, network)
        .map_err(|error| anyhow!("Failed to build the genesis share block: {error}"))?;

    let block_ser = serialize_hex(&block);

    println!("Generated a shareblock (hex):");
    println!("{block_ser}");

    println!();
    println!(
        "Add using ({timestamp}, {bitcoin_height}, include_str!(\"{network}.rs\").into()) at fn genesis_data"
    );
    println!("See function genesis_data at p2poolv2_lib/src/shares/genesis/mod.rs");

    Ok(())
}
