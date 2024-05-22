use futures::StreamExt;
use std::env;
use std::str;

mod merkle;
mod rpc;

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

    let rpc_client = rpc::RpcClient::new(
        "http://127.0.0.1:8332".to_string(),
        "pragmaxim".to_string(),
        password,
    );

    rpc_client
        .fetch_blocks(start_height, end_height)
        .map(|txs| match txs {
            Ok(transactions) => {
                merkle_sum_tree.update_balances(transactions).unwrap();
            }
            Err(e) => {
                panic!("Error fetching block: {}", e);
            }
        })
        .count()
        .await;

    println!("Top 10 richest addresses:");

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
