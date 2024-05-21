extern crate bitcoin;

use bitcoin::{blockdata::transaction::Transaction, Address, Network};
use grovedb::{Element, GroveDb};
use grovedb::{PathQuery, Query};
use std::str;
use std::str::FromStr;
use std::{fmt, vec};

pub const BALANCE_LEAF: &[u8] = b"balance_leaf";

#[derive(Debug)]
struct Utxo {
    address: String,
    value: u64,
}

impl fmt::Display for Utxo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.address, self.value)
    }
}

impl TryFrom<Vec<u8>> for Utxo {
    type Error = grovedb::Error;

    fn try_from(utxo_str: Vec<u8>) -> Result<Self, Self::Error> {
        let utxo = String::from_utf8(utxo_str)
            .map_err(|err| {
                grovedb::Error::CorruptedData(format!("Invalid UTXO encoding: {}", err))
            })?
            .parse()
            .map_err(|err| {
                grovedb::Error::CorruptedData(format!("Invalid UTXO format : {}", err))
            })?;
        Ok(utxo)
    }
}

impl FromStr for Utxo {
    type Err = grovedb::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(grovedb::Error::CorruptedData(format!(
                "Invalid UTXO : {}",
                s
            )));
        }

        let address = parts[0].to_string();
        let value = parts[1].parse::<u64>().map_err(|err| {
            grovedb::Error::CorruptedData(format!("Invalid UTXO value : {} {}", parts[1], err))
        })?;
        Ok(Utxo { address, value })
    }
}

#[derive(Debug)]
struct IndexedTxid {
    index: usize,
    tx_id: String,
}

impl fmt::Display for IndexedTxid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.tx_id)
    }
}

impl FromStr for IndexedTxid {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid format");
        }

        let index = parts[0].parse::<usize>().map_err(|_| "Invalid value")?;
        let tx_id = parts[1].to_string();
        Ok(IndexedTxid { index, tx_id })
    }
}

// Define a struct to represent the Merkle Sum Tree database
pub struct MerkleSumTree {
    db: GroveDb,
}

impl MerkleSumTree {
    // Constructor function to create a new MerkleSumTree instance
    pub fn new(db_path: &str) -> Result<Self, grovedb::Error> {
        let new_db = GroveDb::open(String::from(db_path))?;
        let root_path: &[&[u8]] = &[];
        new_db
            .insert(root_path, BALANCE_LEAF, Element::empty_tree(), None, None)
            .unwrap()?;
        Ok(MerkleSumTree { db: new_db })
    }

    // Method to insert or update a UTXO for an address
    fn insert_utxo(
        &self,
        tx_id: IndexedTxid,
        utxo: Utxo,
        db_tx: &grovedb::Transaction,
    ) -> Result<(), grovedb::Error> {
        let addr_bytes = utxo.address.as_bytes();
        self.db
            .insert_if_not_exists(
                &[BALANCE_LEAF],
                addr_bytes,
                Element::empty_sum_tree(),
                Some(db_tx),
            )
            .unwrap()?;

        self.db
            .insert(
                &[BALANCE_LEAF, addr_bytes],
                tx_id.to_string().as_bytes(),
                Element::new_sum_item(utxo.value as i64),
                None,
                Some(db_tx),
            )
            .unwrap()?;

        self.db
            .put_aux(
                tx_id.to_string(),
                utxo.to_string().as_bytes(),
                None,
                Some(db_tx),
            )
            .unwrap()?;
        Ok(())
    }

    // Method to process the outputs of a transaction
    fn process_outputs(
        &self,
        tx: &Transaction,
        db_tx: &grovedb::Transaction,
    ) -> Result<(), grovedb::Error> {
        let txid = tx.compute_txid();
        for (tx_index, out) in tx.output.iter().enumerate() {
            let address = if let Ok(address) =
                Address::from_script(out.script_pubkey.as_script(), Network::Bitcoin)
            {
                address
            } else if let Some(pk) = out.script_pubkey.p2pk_public_key() {
                bitcoin::Address::p2pkh(pk.pubkey_hash(), bitcoin::Network::Bitcoin)
            } else {
                panic!("Invalid script in tx {} of value {}", txid, out.value)
            };

            self.insert_utxo(
                IndexedTxid {
                    index: tx_index,
                    tx_id: txid.to_string(),
                },
                Utxo {
                    address: address.to_string(),
                    value: out.value.to_sat(),
                },
                db_tx,
            )?;
        }
        Ok(())
    }

    // Method to process the inputs of a transaction
    fn process_inputs(
        &self,
        tx: &Transaction,
        db_tx: &grovedb::Transaction,
    ) -> Result<(), grovedb::Error> {
        for input in &tx.input {
            let indexed_txid = IndexedTxid {
                index: input.previous_output.vout as usize,
                tx_id: input.previous_output.txid.to_string(),
            };

            if let Some(utxo_str) = self
                .db
                .get_aux(indexed_txid.to_string(), Some(db_tx))
                .unwrap()?
            {
                let utxo: Utxo = Utxo::try_from(utxo_str)?;

                self.db
                    .delete_aux(indexed_txid.to_string(), None, Some(db_tx))
                    .unwrap()?;

                let addr_bytes = utxo.address.as_bytes();

                self.db
                    .insert(
                        &[BALANCE_LEAF, addr_bytes],
                        indexed_txid.to_string().as_bytes(),
                        Element::new_sum_item(-(utxo.value as i64)),
                        None,
                        Some(db_tx),
                    )
                    .unwrap()?;
            } else {
                return Err(grovedb::Error::PathKeyNotFound(format!(
                    "indexed tx not found: {}",
                    indexed_txid
                )));
            }
        }
        Ok(())
    }

    pub fn update_balances(&mut self, txs: &[Transaction]) -> Result<(), grovedb::Error> {
        let db_tx = self.db.start_transaction();
        for tx in txs {
            self.process_outputs(tx, &db_tx)?;
            if !tx.is_coinbase() {
                self.process_inputs(tx, &db_tx)?;
            }
        }
        self.db.commit_transaction(db_tx).unwrap()
    }

    // find address with the highest balance
    pub fn top_richest_address(&self) -> Result<Vec<(Vec<u8>, i64)>, grovedb::Error> {
        let mut query = Query::new();
        query.insert_all();
        let path_query = PathQuery::new_unsized(vec![BALANCE_LEAF.to_vec()], query);

        let (addresses, _) = self.db.query_item_value(&path_query, true, None).unwrap()?;

        // collect all addresses and their balances into Vector
        let mut addr_balances: Vec<(Vec<u8>, i64)> = vec![];

        for address in addresses {
            let sum = self
                .db
                .get([BALANCE_LEAF].as_ref(), &address, None)
                .unwrap()?
                .sum_value_or_default();
            addr_balances.push((address, sum))
        }

        addr_balances.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(addr_balances)
    }
}
