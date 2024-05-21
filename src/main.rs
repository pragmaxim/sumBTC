mod merkle;
mod rpc;
use std::env;

use futures::StreamExt;
use std::str;

#[tokio::main]
async fn main() {
    // Create a new MerkleSumTree instance
    let mut merkle_sum_tree = merkle::MerkleSumTree::new("/tmp/grove.db").unwrap();

    let password = match env::var("BITCOIN_RPC_PASSWORD") {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Error: Bitcoin RPC password environment variable not set");
            return;
        }
    };

    let start_height = 0;
    let end_height = 127000; // For example, to get blocks from height 0 to 10

    rpc::fetch_blocks(
        "http://127.0.0.1:8332".to_string(),
        "pragmaxim".to_string(),
        password,
        start_height,
        end_height,
    )
    .map(|block| match block {
        Ok(block) => {
            merkle_sum_tree.update_balances(&block.txdata).unwrap();
        }
        Err(e) => {
            panic!("Error fetching block: {}", e);
        }
    })
    .count()
    .await;

    for (address, balance) in merkle_sum_tree
        .top_richest_address()
        .unwrap()
        .iter()
        .take(10)
    {
        println!(
            "Address: {}, Balance: {}",
            str::from_utf8(&address).unwrap(),
            balance
        );
    }
}
