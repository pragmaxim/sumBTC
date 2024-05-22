use futures::StreamExt;
use std::env;
use std::str;

mod merkle;
mod rpc;

#[tokio::main]
async fn main() {
    // read bitcoin url from arguments
    let bitcoin_url = env::args()
        .nth(1)
        .unwrap_or("http://127.0.0.1:8332".to_string());

    // Create a new MerkleSumTree instance

    let (username, password) = match (
        env::var("BITCOIN_RPC_USERNAME"),
        env::var("BITCOIN_RPC_PASSWORD"),
    ) {
        (Ok(user), Ok(pass)) => (user, pass),
        _ => {
            eprintln!("Error: Bitcoin RPC BITCOIN_RPC_PASSWORD or BITCOIN_RPC_USERNAME environment variable not set");
            return;
        }
    };

    let mut merkle_sum_tree = merkle::MerkleSumTree::new("/tmp/grove.db").unwrap();
    let rpc_client = rpc::RpcClient::new(bitcoin_url, username, password);

    let last_height = merkle_sum_tree.get_last_height();
    let from_height: u64 = last_height + 1;
    let end_height: u64 = 844566;

    println!("Initiating syncing from {} to {}", from_height, end_height);
    rpc_client
        .fetch_blocks(from_height, end_height)
        .map(|result| match result {
            Ok((height, transactions)) => {
                merkle_sum_tree
                    .update_balances(height, transactions)
                    .unwrap();
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
