use bitcoin::Transaction;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use chrono::DateTime;
use futures::stream::StreamExt;
use sum_btc::model;
use tokio::task;
use tokio_stream::Stream; // Add this line to import the `model` module

pub fn fetch_blocks(
    rpc_url: String,
    username: String,
    password: String,
    start_height: u64,
    end_height: u64,
) -> impl Stream<Item = Result<Vec<model::SumTx>, String>> {
    let heights = start_height..=end_height;
    tokio_stream::iter(heights)
        .map(move |height| {
            let user_pass = Auth::UserPass(username.to_string(), password.to_string());
            let rpc_url = rpc_url.to_string();

            task::spawn_blocking(move || {
                // Connect to the local Bitcoin Core node
                let rpc = Client::new(&rpc_url, user_pass).map_err(|e| e.to_string())?;

                // Get the block hash at the specified height
                let block_hash = rpc.get_block_hash(height).map_err(|e| e.to_string())?;

                // Get the block by its hash
                let block = rpc.get_block(&block_hash).map_err(|e| e.to_string())?;

                // print the block hash if height is divisible by 1000
                if height % 1000 == 0 {
                    let datetime = DateTime::from_timestamp(block.header.time as i64, 0).unwrap();
                    let readable_date = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
                    println!("Block @ {} : {} : {}", readable_date, height, block_hash);
                }

                Ok::<bitcoin::Block, String>(block)
            })
        })
        .buffered(32) // Process up to 16 blocks in parallel
        // process_txs
        .map(|result| async {
            match result {
                Ok(Ok(block)) => {
                    let txs = block.txdata;
                    let sum_txs = process_txs(txs).await;
                    Ok(sum_txs)
                }
                Ok(Err(e)) => Err(e),
                Err(e) => Err(e.to_string()),
            }
        })
        .buffered(32)
}

pub async fn process_txs(txs: Vec<Transaction>) -> Vec<model::SumTx> {
    let futures: Vec<tokio::task::JoinHandle<model::SumTx>> = txs
        .into_iter()
        .enumerate()
        .map(|(index, tx)| tokio::task::spawn_blocking(move || model::SumTx::from((index, tx))))
        .collect();

    let sum_txs: Vec<model::SumTx> = futures::future::join_all(futures)
        .await
        .into_iter()
        .map(|res| res.expect("Task panicked"))
        .collect();

    sum_txs
}
