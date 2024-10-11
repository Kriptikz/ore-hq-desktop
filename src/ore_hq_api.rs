use std::{mem::size_of, ops::Range};

use solana_sdk::pubkey::Pubkey;

#[derive(Debug)]
pub enum ServerMessage {
    StartMining([u8; 32], Range<u64>, u64),
    PoolSubmissionResult(ServerMessagePoolSubmissionResult),
}

#[derive(Debug)]
pub struct ServerMessagePoolSubmissionResult {
    difficulty: u32,
    total_balance: f64,
    total_rewards: f64,
    top_stake: f64,
    multiplier: f64,
    active_miners: u32,
    challenge: [u8; 32],
    best_nonce: u64,
    miner_supplied_difficulty: u32,
    miner_earned_rewards: f64,
    miner_percentage: f64
}

impl ServerMessagePoolSubmissionResult {
    pub fn new_from_bytes(b: Vec<u8>) -> Self {
        let mut b_index = 1;

        let data_size = size_of::<u32>();
        let mut data_bytes = [0u8; size_of::<u32>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let difficulty = u32::from_le_bytes(data_bytes);

        let data_size = size_of::<f64>();
        let mut data_bytes = [0u8; size_of::<f64>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let total_balance = f64::from_le_bytes(data_bytes);

        let data_size = size_of::<f64>();
        let mut data_bytes = [0u8; size_of::<f64>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let total_rewards = f64::from_le_bytes(data_bytes);

        let data_size = size_of::<f64>();
        let mut data_bytes = [0u8; size_of::<f64>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let top_stake = f64::from_le_bytes(data_bytes);

        let data_size = size_of::<f64>();
        let mut data_bytes = [0u8; size_of::<f64>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let multiplier = f64::from_le_bytes(data_bytes);

        let data_size = size_of::<u32>();
        let mut data_bytes = [0u8; size_of::<u32>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let active_miners = u32::from_le_bytes(data_bytes);

        let data_size = 32;
        let mut data_bytes = [0u8; 32];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let challenge = data_bytes.clone();

        let data_size = size_of::<u64>();
        let mut data_bytes = [0u8; size_of::<u64>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let best_nonce = u64::from_le_bytes(data_bytes);

        let data_size = size_of::<u32>();
        let mut data_bytes = [0u8; size_of::<u32>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let miner_supplied_difficulty = u32::from_le_bytes(data_bytes);

        let data_size = size_of::<f64>();
        let mut data_bytes = [0u8; size_of::<f64>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        b_index += data_size;
        let miner_earned_rewards = f64::from_le_bytes(data_bytes);

        let data_size = size_of::<f64>();
        let mut data_bytes = [0u8; size_of::<f64>()];
        for i in 0..data_size {
            data_bytes[i] = b[i + b_index];
        }
        //b_index += data_size;
        let miner_percentage = f64::from_le_bytes(data_bytes);

        ServerMessagePoolSubmissionResult {
            difficulty,
            total_balance,
            total_rewards,
            top_stake,
            multiplier,
            active_miners,
            challenge,
            best_nonce,
            miner_supplied_difficulty,
            miner_earned_rewards,
            miner_percentage
        }
    }

    pub fn to_message_binary(&self) -> Vec<u8> {
        let mut bin_data = Vec::new();
        bin_data.push(1u8);
        bin_data.extend_from_slice(&self.difficulty.to_le_bytes());
        bin_data.extend_from_slice(&self.total_balance.to_le_bytes());
        bin_data.extend_from_slice(&self.total_rewards.to_le_bytes());
        bin_data.extend_from_slice(&self.top_stake.to_le_bytes());
        bin_data.extend_from_slice(&self.multiplier.to_le_bytes());
        bin_data.extend_from_slice(&self.active_miners.to_le_bytes());
        bin_data.extend_from_slice(&self.challenge);
        bin_data.extend_from_slice(&self.best_nonce.to_le_bytes());
        bin_data.extend_from_slice(&self.miner_supplied_difficulty.to_le_bytes());
        bin_data.extend_from_slice(&self.miner_earned_rewards.to_le_bytes());
        bin_data.extend_from_slice(&self.miner_percentage.to_le_bytes());

        bin_data
    }
}

#[derive(Debug)]
pub struct ServerStartMining {
    challenge: [u8; 32],
    cutoff: u64,
    nonce_start: u64,
    nonce_end: u64
}

impl ServerStartMining {
    pub fn new_from_bytes(b: Vec<u8>) -> Result<Self, ()> {
        if b.len() < 49 {
            println!("Invalid data for Message StartMining");
            return Err(())
        }
        let mut challenge = [0u8; 32];
        // extract 256 bytes (32 u8's) from data for hash
        let mut b_index = 1;
        for i in 0..32 {
            challenge[i] = b[i + b_index];
        }
        b_index += 32;

        // extract 64 bytes (8 u8's)
        let mut cutoff_bytes = [0u8; 8];
        for i in 0..8 {
            cutoff_bytes[i] = b[i + b_index];
        }
        b_index += 8;
        let cutoff = u64::from_le_bytes(cutoff_bytes);

        let mut nonce_start_bytes = [0u8; 8];
        for i in 0..8 {
            nonce_start_bytes[i] = b[i + b_index];
        }
        b_index += 8;
        let nonce_start = u64::from_le_bytes(nonce_start_bytes);

        let mut nonce_end_bytes = [0u8; 8];
        for i in 0..8 {
            nonce_end_bytes[i] = b[i + b_index];
        }
        let nonce_end = u64::from_le_bytes(nonce_end_bytes);

        Ok(ServerStartMining {
            challenge,
            cutoff,
            nonce_start,
            nonce_end,
        })
    }

    pub fn to_message_binary(&self) -> Vec<u8> {
        let mut bin_data = Vec::new();
        bin_data.push(0u8);
        bin_data.extend_from_slice(&self.challenge);
        bin_data.extend_from_slice(&self.cutoff.to_le_bytes());
        bin_data.extend_from_slice(&self.nonce_start.to_le_bytes());
        bin_data.extend_from_slice(&self.nonce_end.to_le_bytes());

        bin_data
    }
}

#[derive(Debug)]
pub enum ClientMessage {
    Ready(),
    BestSolution(ServerMessagePoolSubmissionResult),
}

#[derive(Debug)]
pub struct ClientMessageReady;

impl ClientMessageReady {
    pub fn new() -> Self {
        ClientMessageReady
    }

    pub fn new_from_bytes(b: Vec<u8>) -> Result<Self, ()> {
        if b.len() < 1 {
            return Err(())
        }
        if b[0] != 1 {
            return Err(())
        }
        Ok(ClientMessageReady)
    }

    pub fn to_message_binary(&self) -> Vec<u8> {
        let mut bin_data = Vec::new();
        bin_data.push(1u8);

        bin_data
    }
}

#[derive(Debug)]
pub struct ClientMessageBestSolution {
    best_hash: [u8; 16],
    best_nonce: u64,
    pubkey: Pubkey,
    signature: Vec<u8>
}

impl ClientMessageBestSolution {
    pub fn new(best_hash: [u8; 16], best_nonce: u64, pubkey: Pubkey, signature: Vec<u8>) -> Self {
        ClientMessageBestSolution {
            best_hash,
            best_nonce,
            pubkey,
            signature
        }
    }

    pub fn new_from_bytes(b: Vec<u8>) -> Result<Self, ()> {
        if b.len() < 1 {
            return Err(())
        }
        if b[0] != 2 {
            return Err(())
        }

        let mut solution_bytes = [0u8; 16];
        // extract (16 u8's) from data for hash digest
        let mut b_index = 1;
        for i in 0..16 {
            solution_bytes[i] = b[i + b_index];
        }
        b_index += 16;

        // extract 64 bytes (8 u8's)
        let mut nonce = [0u8; 8];
        for i in 0..8 {
            nonce[i] = b[i + b_index];
        }
        b_index += 8;

        let mut pubkey = [0u8; 32];
        for i in 0..32 {
            pubkey[i] = b[i + b_index];
        }

        b_index += 32;

        let signature_bytes = b[b_index..].to_vec();
        let pubkey = Pubkey::new_from_array(pubkey);
        Ok(ClientMessageBestSolution {
            best_hash: solution_bytes,
            best_nonce: u64::from_be_bytes(nonce),
            pubkey,
            signature: signature_bytes,
        })
    }

    pub fn to_message_binary(&self) -> Vec<u8> {
        let mut bin_data = Vec::new();
        bin_data.push(2u8);
        bin_data.extend_from_slice(&self.best_hash);
        bin_data.extend_from_slice(&self.best_nonce.to_le_bytes());
        bin_data.extend_from_slice(&self.pubkey.to_bytes());
        bin_data.extend_from_slice(&self.signature);

        bin_data
    }
}
