extern crate bitcoin;

use bitcoin::{
    blockdata::transaction::{Transaction, Txid},
    Address, Amount, Network,
};

use std::collections::HashMap;

// Struct to represent a UTXO
#[derive(Debug)]
struct Utxo {
    address: String,
    value: Amount,
}

// Define a struct to represent the Merkle Sum Tree database
pub struct MerkleSumTree {
    balances: HashMap<String, Amount>, // Address to balance mapping
    prev_txid_cache: HashMap<(Txid, usize), Utxo>,
}

impl MerkleSumTree {
    // Constructor function to create a new MerkleSumTree instance
    pub fn new() -> Self {
        MerkleSumTree {
            balances: HashMap::new(),
            prev_txid_cache: HashMap::new(),
        }
    }

    // Method to insert or update a UTXO for an address
    fn insert_utxo(
        &mut self,
        txid: &Txid,
        tx_index: usize,
        address: &str,
        amount: Amount,
    ) -> Option<Utxo> {
        let balance = self
            .balances
            .entry(address.to_string())
            .or_insert(Amount::ZERO);
        *balance += amount;
        // cache txid into prev_txid_cache
        let utxo = Utxo {
            address: address.to_string(),
            value: amount,
        };
        self.prev_txid_cache.insert((txid.clone(), tx_index), utxo)
    }

    // Method to simulate updating balances based on a transaction
    pub fn update_balances(&mut self, txs: &[Transaction]) {
        // process outputs of each transaction and inputs of all transactions except the first coinbase transaction
        for tx in txs {
            self.process_outputs(tx);
            if !tx.is_coinbase() {
                self.process_inputs(tx);
            }
        }
    }

    // Method to process the outputs of a transaction
    fn process_outputs(&mut self, tx: &Transaction) {
        let txid = tx.compute_txid();
        for (tx_index, out) in tx.output.iter().enumerate() {
            if let Ok(address) =
                Address::from_script(out.script_pubkey.as_script(), Network::Bitcoin)
            {
                self.insert_utxo(&txid, tx_index, &address.to_string(), out.value);
            } else if let Some(pk) = out.script_pubkey.p2pk_public_key() {
                let address = bitcoin::Address::p2pkh(pk.pubkey_hash(), bitcoin::Network::Bitcoin);
                self.insert_utxo(&txid, tx_index, &address.to_string(), out.value);
            } else {
                println!("Invalid script in tx {} of value {}", txid, out.value);
            }
        }
    }

    // Method to process the inputs of a transaction
    fn process_inputs(&mut self, tx: &Transaction) {
        for input in &tx.input {
            let utxo = self
                .prev_txid_cache
                .remove(&(
                    input.previous_output.txid,
                    input.previous_output.vout as usize,
                ))
                .unwrap();
            let address = &utxo.address;
            let amount = utxo.value;
            if let Some(balance) = self.balances.get_mut(address) {
                if *balance >= amount {
                    *balance -= amount;
                } else {
                    panic!(
                        "Insufficient amount {} to spend from balance {} at address {}",
                        amount, balance, address
                    );
                }
            } else {
                panic!("Address {address} not found in balance map");
            }
        }
    }

    // Method to print all address balances
    pub fn print_balances(&self) {
        println!(
            "Total balances {} and cached txs {}",
            self.balances.len(),
            self.prev_txid_cache.len()
        );
    }
}
