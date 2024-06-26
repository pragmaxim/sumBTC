use bitcoin::{blockdata::transaction::Transaction, Address, Network};
use std::str::FromStr;
use std::{fmt, str};

#[derive(Debug)]
pub struct SumTx {
    pub is_coinbase: bool,
    pub txid: String,
    pub ins: Vec<IndexedTxid>,
    pub outs: Vec<Utxo>,
}

impl From<Transaction> for SumTx {
    fn from(tx: Transaction) -> Self {
        SumTx {
            is_coinbase: tx.is_coinbase(),
            txid: tx.compute_txid().to_string(),
            ins: tx
                .input
                .iter()
                .map(|input| IndexedTxid {
                    index: input.previous_output.vout as usize,
                    tx_id: input.previous_output.txid.to_string(),
                })
                .collect(),
            outs: tx
                .output
                .iter()
                .enumerate()
                .flat_map(|(out_index, out)| {
                    let address_opt = if let Ok(address) =
                        Address::from_script(out.script_pubkey.as_script(), Network::Bitcoin)
                    {
                        Some(address)
                    } else if let Some(pk) = out.script_pubkey.p2pk_public_key() {
                        Some(bitcoin::Address::p2pkh(
                            pk.pubkey_hash(),
                            bitcoin::Network::Bitcoin,
                        ))
                    } else {
                        println!(
                            "Invalid script in tx {} of value {}",
                            tx.compute_txid(),
                            out.value
                        );
                        None
                    };

                    match address_opt {
                        Some(address) => Some(Utxo {
                            index: out_index,
                            address: address.to_string(),
                            value: out.value.to_sat(),
                        }),
                        None => None,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct Utxo {
    pub index: usize,
    pub address: String,
    pub value: u64,
}

impl fmt::Display for Utxo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.index, self.address, self.value)
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
        if parts.len() != 3 {
            return Err(grovedb::Error::CorruptedData(format!(
                "Invalid UTXO : {}",
                s
            )));
        }

        let out_index = parts[0].parse::<usize>().map_err(|err| {
            grovedb::Error::CorruptedData(format!("Invalid UTXO index : {} {}", parts[0], err))
        })?;
        let address = parts[1].to_string();
        let value = parts[2].parse::<u64>().map_err(|err| {
            grovedb::Error::CorruptedData(format!("Invalid UTXO value : {} {}", parts[1], err))
        })?;
        Ok(Utxo {
            index: out_index,
            address,
            value,
        })
    }
}

#[derive(Debug)]
pub struct IndexedTxid {
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
