use bitcoincore_rpc::{Auth, Client, RpcApi};
use futures::stream::StreamExt;
use tokio::task;
use tokio_stream::Stream;

pub fn fetch_blocks(
    rpc_url: String,
    username: String,
    password: String,
    start_height: u64,
    end_height: u64,
) -> impl Stream<Item = Result<bitcoin::Block, String>> {
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

                Ok(block)
            })
        })
        .buffered(16) // Process up to 16 blocks in parallel
        .then(|result| async {
            match result {
                Ok(Ok(block)) => Ok(block),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(e.to_string()),
            }
        })
}
