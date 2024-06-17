use grovedb::{Element, GroveDb};
use grovedb::{PathQuery, Query};
use std::str::{self, FromStr};
use std::vec;
use sum_btc::model::{SumTx, Utxo};

pub const BALANCE_LEAF: &[u8] = b"balance_leaf";
pub const TX_CACHE_LEAF: &[u8] = b"tx_cache_leaf";
pub const LAST_HEIGHT_KEY: &[u8] = b"last_height";

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
            .insert_if_not_exists(root_path, BALANCE_LEAF, Element::empty_tree(), None)
            .unwrap()?;
        new_db
            .insert_if_not_exists(root_path, TX_CACHE_LEAF, Element::empty_tree(), None)
            .unwrap()?;
        Ok(MerkleSumTree { db: new_db })
    }

    // Method to insert or update a UTXO for an address
    fn insert_utxo(
        &self,
        tx_id: &str,
        utxo: &Utxo,
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

        let tx_id_with_index = format!("{}:{}", utxo.index, tx_id);
        self.db
            .insert(
                &[BALANCE_LEAF, addr_bytes],
                tx_id_with_index.as_bytes(),
                Element::new_sum_item(utxo.value as i64),
                None,
                Some(db_tx),
            )
            .unwrap()?;

        self.db
            .insert(
                &[TX_CACHE_LEAF],
                tx_id_with_index.as_bytes(),
                Element::new_item(utxo.to_string().into_bytes()),
                None,
                Some(db_tx),
            )
            .unwrap()?;
        Ok(())
    }

    // Method to process the outputs of a transaction
    fn process_outputs(
        &self,
        sum_tx: &SumTx,
        db_tx: &grovedb::Transaction,
    ) -> Result<(), grovedb::Error> {
        for utxo in sum_tx.outs.iter() {
            self.insert_utxo(&sum_tx.txid, utxo, db_tx)?;
        }
        Ok(())
    }

    // Method to process the inputs of a transaction
    fn process_inputs(
        &self,
        sum_tx: SumTx,
        db_tx: &grovedb::Transaction,
    ) -> Result<(), grovedb::Error> {
        for indexed_txid in sum_tx.ins {
            let tx_cache_key = indexed_txid.to_string();
            let utxo_str = self
                .db
                .get(&[TX_CACHE_LEAF], tx_cache_key.as_bytes(), Some(db_tx))
                .unwrap()?;

            let utxo: Utxo =
                Utxo::from_str(str::from_utf8(utxo_str.as_item_bytes().unwrap()).unwrap())?;

            self.db
                .delete(&[TX_CACHE_LEAF], tx_cache_key.as_bytes(), None, Some(db_tx))
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
        }
        Ok(())
    }

    pub fn get_last_height(&self) -> u64 {
        return self
            .db
            .get_aux(LAST_HEIGHT_KEY, None)
            .unwrap()
            .unwrap()
            .map_or(0, |height| {
                String::from_utf8(height).unwrap().parse::<u64>().unwrap()
            });
    }

    fn store_block_height(
        &self,
        height: u64,
        db_tx: &grovedb::Transaction,
    ) -> Result<(), grovedb::Error> {
        self.db
            .put_aux(
                LAST_HEIGHT_KEY,
                height.to_string().as_bytes(),
                None,
                Some(db_tx),
            )
            .unwrap()?;
        Ok(())
    }

    pub fn update_balances(&mut self, height: u64, txs: Vec<SumTx>) -> Result<(), grovedb::Error> {
        let db_tx = self.db.start_transaction();
        for tx in txs {
            self.process_outputs(&tx, &db_tx)?;
            if !tx.is_coinbase {
                self.process_inputs(tx, &db_tx)?;
            }
        }
        self.store_block_height(height, &db_tx)?;
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
