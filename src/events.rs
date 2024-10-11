use async_compat::Compat;
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, IoTaskPool},
};
use bip39::{Language, Mnemonic, MnemonicType, Seed};
use cocoon::Cocoon;
use crossbeam_channel::{bounded, unbounded};
use drillx_2::{Solution};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};

use crate::{
    ore_utils::
        get_ore_mint
    , tasks::{
        TaskProcessTx, TaskProcessTxData
    }, ui::{
        components::{TextGeneratedKeypair, TextInput, TextMnemonicLine1, TextMnemonicLine2, TextMnemonicLine3, TextPasswordInput, ToggleAutoMine},
        styles::{MINE_TOGGLE_OFF, MINE_TOGGLE_ON},
    }, AppConfig, AppScreenState, AppWallet, BussesResource, EntityTaskFetchUiData, EntityTaskHandler, HashStatus, HashrateResource, MinerStatusResource, MiningDataChannelMessage, MiningDataChannelResource, OreAppState, ProofAccountResource, TreasuryAccountResource, TxStatus
};

use std::{
    fs::File, io::{stdout, Write}, path::{Path, PathBuf}, str::FromStr, sync::{atomic::AtomicBool, Arc, Mutex}, time::{Duration, Instant}
};

use solana_sdk::{
    bs58, commitment_config::CommitmentLevel, compute_budget::ComputeBudgetInstruction, derivation_path::DerivationPath, keccak::{hashv, Hash as KeccakHash}, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::{read_keypair_file, Keypair, Signer}, signer::SeedDerivable, transaction::Transaction
};

// Events
#[derive(Event)]
pub struct EventStartStopMining;

#[derive(Event)]
pub struct EventGenerateWallet;

#[derive(Event)]
pub struct EventLoadKeypairFile(pub PathBuf);

#[derive(Event)]
pub struct EventSaveWallet;

#[derive(Event)]
pub struct EventMineForHash;

#[derive(Event)]
pub struct EventRequestAirdrop;

#[derive(Event)]
pub struct EventSubmitHashTx(pub (Solution, u32, u64, u64));

pub struct TxResult {
    pub sig: String,
    pub tx_time: u64,
    pub hash_time: u64,
    // TODO: create a TxStatus struct that will be able to show different colors based on status enums
    pub status: String,
}

#[derive(Event)]
pub struct EventTxResult {
    pub tx_type: String,
    pub sig: String,
    pub tx_time: u64,
    pub hash_status: Option<HashStatus>,
    pub tx_status: TxStatus,
}

#[derive(Event)]
pub struct EventFetchUiDataFromRpc;

#[derive(Event)]
pub struct EventRegisterWallet;

#[derive(Event)]
pub struct EventClaimOreRewards;

#[derive(Event)]
pub struct EventCheckSigs;

#[derive(Event)]
pub struct EventStakeOre;

#[derive(Event)]
pub struct EventProcessTx {
    pub tx_type: String,
    pub tx: Transaction,
    pub hash_status: Option<(u64, u32)>,
}

#[derive(Event)]
pub struct EventLock;

#[derive(Event)]
pub struct EventUnlock;

#[derive(Event)]
pub struct EventSaveConfig(pub AppConfig);

pub fn handle_event_start_stop_mining_clicked(
    mut ev_start_stop_mining: EventReader<EventStartStopMining>,
    mut event_writer: EventWriter<EventMineForHash>,
    mut event_writer_cancel_mining: EventWriter<EventCancelMining>,
    mut event_writer_register: EventWriter<EventRegisterWallet>,
    app_wallet: Res<AppWallet>,
    mut miner_status: ResMut<MinerStatusResource>,
    proof_account: Res<ProofAccountResource>,
    asset_server: Res<AssetServer>,
    mut query: Query<(&mut UiImage, &mut ToggleAutoMine)>,
) {
    for _ev in ev_start_stop_mining.read() {
        match miner_status.miner_status.as_str() {
            "MINING" |
            "PROCESSING" => {
                // stop mining
                miner_status.miner_status = "STOPPED".to_string();
                let (mut btn, mut toggle) = query.single_mut();
                toggle.0 = false;
                *btn = UiImage::new(asset_server.load(MINE_TOGGLE_OFF));
                event_writer_cancel_mining.send(EventCancelMining);
            
            },
            "STOPPED" => {
                // start mining
                if proof_account.challenge == "Not Found" {
                    event_writer_register.send(EventRegisterWallet);
                } else {
                    event_writer.send(EventMineForHash);
                    let (mut btn, mut toggle) = query.single_mut();
                    toggle.0 = true;
                    *btn = UiImage::new(asset_server.load(MINE_TOGGLE_ON));
                }
            },
            _ => {
                error!("Invalid Miner Status in handle_event_start_stop_mining_clicked");
            }

        }
    }
}

pub fn handle_event_mine_for_hash(
    mut commands: Commands,
    mut event_reader: EventReader<EventMineForHash>,
    app_wallet: Res<AppWallet>,
    ore_config_res: Res<TreasuryAccountResource>,
    mut miner_status: ResMut<MinerStatusResource>,
    query_task_handler: Query<Entity, With<EntityTaskHandler>>,
    mut next_state: ResMut<NextState<AppScreenState>>,
    mut mining_channels_res: ResMut<MiningDataChannelResource>,
) {
    for _ev in event_reader.read() {
        if let Ok(task_handler_entity) = query_task_handler.get_single() {
            let pool = AsyncComputeTaskPool::get();
            let wallet = if let Some(wallet) =  &app_wallet.wallet {
                wallet.clone()
            } else {
                next_state.set(AppScreenState::Unlock);
                error!("wallet is None, switching to wallet unlock screen");
                continue;
            }; 
            if mining_channels_res.sender.is_none() {
                let (sender, receiver) = bounded::<MiningDataChannelMessage>(1);
                mining_channels_res.sender = Some(sender.clone());
                mining_channels_res.receiver = Some(receiver.clone());
            }

            let sys_info = &miner_status.sys_info;
            let cpu_count = sys_info.cpus().len() as u64;
            let threads = miner_status.miner_threads.clamp(1, cpu_count);

            let channel_rec = mining_channels_res.receiver.as_ref().unwrap();
            let channel_sender = mining_channels_res.sender.as_ref().unwrap();

            let receiver = channel_rec.clone();
            let sender = channel_sender.clone();

            while let Ok(_) = receiver.try_recv() {
                // clear out any current messages
            }

            let min_difficulty = ore_config_res.min_difficulty;

            let task = pool.spawn(Compat::new(async move {
                let hash_time = Instant::now();
                // let (solution, best_difficulty, best_hash, total_nonces_checked) = find_hash_par(
                //     proof,
                //     cutoff,
                //     threads,
                //     min_difficulty as u32,
                //     receiver,
                //     sender,
                // );

                // Ok((solution, best_difficulty, hash_time.elapsed().as_secs(), total_nonces_checked))
            }));
            //miner_status.miner_status = "MINING".to_string();

            // commands
            //     .entity(task_handler_entity)
            //     .insert(TaskGenerateHash { task });
        }
    }
}

pub struct CurrentBus {
    bus: usize
}

impl Default for CurrentBus {
    fn default() -> Self {
        Self { bus: 0 }
    }
}

pub fn handle_event_submit_hash_tx(
    mut commands: Commands,
    mut ev_submit_hash_tx: EventReader<EventSubmitHashTx>,
    query_task_handler: Query<Entity, With<EntityTaskHandler>>,
    app_wallet: Res<AppWallet>,
    treasury: Res<TreasuryAccountResource>,
    mut miner_status: ResMut<MinerStatusResource>,
    mut busses_res: ResMut<BussesResource>,
    mut next_state: ResMut<NextState<AppScreenState>>,
    mut hashrate_res: ResMut<HashrateResource>,
) {
    for ev in ev_submit_hash_tx.read() {
        let wallet = if let Some(wallet) =  &app_wallet.wallet {
            wallet.clone()
        } else {
            next_state.set(AppScreenState::Unlock);
            error!("wallet is None, switching to wallet unlock screen");
            continue;
        }; 
        if let Ok(task_handler_entity) = query_task_handler.get_single() {
            let pool = IoTaskPool::get();

            info!("Hashrate: {}/second", hashrate_res.hashrate);

            let task = pool.spawn(Compat::new(async move {
                let signer = wallet;
                let mut attempts = 3;

                // Submit a hash on the websocket

                        // let process_data = TaskProcessTxData {
                        //     tx_type: "Mine".to_string(),
                        //     signature: None,
                        //     signed_tx: Some(tx),
                        //     hash_time: Some((hash_time, difficulty)),
                        // };

                        // return Ok(process_data);

                // let process_data = TaskProcessTxData {
                //     tx_type: "Mine".to_string(),
                //     signature: None,
                //     signed_tx: None,
                //     hash_time: Some((hash_time, difficulty)),
                // };
                // return Err((
                //     process_data,
                //     "Failed to get latest blockhash".to_string()
                // ));

            }));

            miner_status.miner_status = "PROCESSING".to_string();
            // commands
            //     .entity(task_handler_entity)
            //     .insert(TaskProcessTx { task });
        } else {
            error!("Failed to get task entity. handle_event_submit_hash_tx");
        }
    }
}

pub fn handle_event_fetch_ui_data_from_rpc(
    mut commands: Commands,
    app_wallet: Res<AppWallet>,
    mut event_reader: EventReader<EventFetchUiDataFromRpc>,
    query_task_handler: Query<Entity, With<EntityTaskFetchUiData>>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for _ev in event_reader.read() {
        let wallet = if let Some(wallet) =  &app_wallet.wallet {
            wallet.clone()
        } else {
            next_state.set(AppScreenState::Unlock);
            error!("wallet is None, switching to wallet unlock screen");
            continue;
        }; 
        if let Ok(task_handler_entity) = query_task_handler.get_single() {
            let pubkey = wallet.pubkey();

            let pool = IoTaskPool::get();
            let ore_mint = get_ore_mint();
            // let task = pool.spawn(Compat::new(async move {
            //     Ok(TaskUpdateAppWalletSolBalanceData {
            //         sol_balance: 0.0,
            //         ore_balance: 0.0,
            //         proof_account_data: proof_account_res_data,
            //         treasury_account_data: treasury_account_res_data,
            //         busses: busses_res_data,
            //     })
            // }));

            // commands
            //     .entity(task_handler_entity)
            //     .insert(TaskUpdateAppWalletSolBalance { task });
        } else {
            error!("Failed to get task_handler_entity. handle_event_fetch_ui_data_from_rpc");
        }
    }
}

pub fn handle_event_register_wallet(
    mut commands: Commands,
    mut event_reader: EventReader<EventRegisterWallet>,
    app_wallet: Res<AppWallet>,
    query_task_handler: Query<Entity, With<EntityTaskHandler>>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for _ev in event_reader.read() {
        let wallet = if let Some(wallet) =  &app_wallet.wallet {
            wallet.clone()
        } else {
            next_state.set(AppScreenState::Unlock);
            error!("wallet is None, switching to wallet unlock screen");
            continue;
        }; 
        if let Ok(task_handler_entity) = query_task_handler.get_single() {
            let pool = IoTaskPool::get();
            let wallet = wallet;

            // Register Wallet with HttpRequest task
            // commands
            //     .entity(task_handler_entity)
            //     .insert(TaskProcessTx { task });
        } else {
            error!("Failed to get task_entity_handler. handle_event_register_wallet");
        }
    }
}

pub fn handle_event_claim_ore_rewards(
    mut commands: Commands,
    mut event_reader: EventReader<EventClaimOreRewards>,
    app_wallet: Res<AppWallet>,
    query_task_handler: Query<Entity, With<EntityTaskHandler>>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for _ev in event_reader.read() {
        let wallet = if let Some(wallet) =  &app_wallet.wallet {
            wallet.clone()
        } else {
            next_state.set(AppScreenState::Unlock);
            error!("wallet is None, switching to wallet unlock screen");
            continue;
        }; 
        if let Ok(task_handler_entity) = query_task_handler.get_single() {

            // Send Claim request to Http Task handler

            // commands
            //     .entity(task_handler_entity)
            //     .insert(TaskProcessTx { task });
        } else {
            error!("Failed to get task_handler_entity. handle_event_claim_ore_rewards.");
        }
    }
}

pub fn handle_event_lock(
    mut commands: Commands,
    mut event_reader: EventReader<EventLock>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for _ev in event_reader.read() {
        commands.remove_resource::<AppWallet>();
        next_state.set(AppScreenState::Unlock);
    }
}

pub fn handle_event_unlock(
    mut event_reader: EventReader<EventUnlock>,
    mut app_wallet: ResMut<AppWallet>,
    query: Query<&TextInput, With<TextPasswordInput>>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for _ev in event_reader.read() {
        let text = query.get_single();
        if let Ok(text_input) = text {
            let password = text_input.text.clone();

            // TODO: use const path?
            let wallet_path = Path::new("save.data");

            let cocoon = Cocoon::new(password.as_bytes());
            let mut file = File::open(wallet_path).unwrap();
            let encoded = cocoon.parse(&mut file);
            if let Ok(encoded) = encoded {
                let wallet = Keypair::from_bytes(&encoded);
                if let Ok(wallet) = wallet {
                    let wallet = Arc::new(wallet);
                    app_wallet.wallet = Some(wallet);
                    next_state.set(AppScreenState::Mining);
                } else {
                    error!("Failed to parse keypair from bytes. (events.rs: handle_event_unlock)");
                }
            } else {
                error!("Failed to decrypt file. (events.rs: handle_event_unlock)");
            }
        } else {
            error!("Failed to get_single on TextPasswordInput (events.rs: handle_event_unlock)");
        }
    }
}

pub fn handle_event_save_config(
    mut event_reader: EventReader<EventSaveConfig>,
    mut ore_app_state: ResMut<OreAppState>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for ev in event_reader.read() {
        let new_config = ev.0.clone();
        let toml_string = toml::to_string(&new_config).unwrap();
        let data = toml_string.into_bytes();

        let mut f = File::create("config.toml").expect("Unable to create file");
        f.write_all(&data).expect("Unable to write data");


        let new_state;
        let wallet_path = Path::new("save.data");
        if wallet_path.exists() {
            new_state = AppScreenState::Mining;
        } else {
            new_state = AppScreenState::WalletSetup;
        }

        //miner_status.miner_threads = new_config.threads;
        ore_app_state.config = new_config;
        next_state.set(new_state);
    }
}

pub fn handle_event_generate_wallet(
    mut event_reader: EventReader<EventGenerateWallet>,
    // mut text_query: Query<&mut Text, With<TextGeneratedPubkey>>,
    // mut ore_app_state: ResMut<OreAppState>,
    // mut next_state: ResMut<NextState<GameState>>,
    mut set: ParamSet<(
        Query<(&mut Text, &mut TextGeneratedKeypair)>,
        Query<&mut Text, With<TextMnemonicLine1>>,
        Query<&mut Text, With<TextMnemonicLine2>>,
        Query<&mut Text, With<TextMnemonicLine3>>,
    )>,
) {
    for _ev in event_reader.read() {
        let new_mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);

        let phrase = new_mnemonic.clone().into_phrase();

        let words: Vec<&str> = phrase.split(" ").collect();

        let seed = Seed::new(&new_mnemonic, "");

        let derivation_path = DerivationPath::from_absolute_path_str("m/44'/501'/0'/0'").unwrap();

        let new_key = Keypair::from_seed_and_derivation_path(seed.as_bytes(), Some(derivation_path));
        if let Ok(new_key) = new_key {
            let new_key = Arc::new(new_key);
            let pubkey = new_key.pubkey().to_string();
            for (mut text, mut text_keypair) in set.p0().iter_mut() {
                text.sections[0].value = pubkey.clone();
                text_keypair.0 = new_key.clone();
            }
            for mut text in set.p1().iter_mut() {
                let mut value = String::new();
                for word in &words[0..4] {
                    value += word;
                    value += "     ";
                }
                text.sections[0].value = value;
            }
            for mut text in set.p2().iter_mut() {
                let mut value = String::new();
                for word in &words[4..8] {
                    value += word;
                    value += "     ";
                }
                text.sections[0].value = value;
            }
            for mut text in set.p3().iter_mut() {
                let mut value = String::new();
                for word in &words[8..12] {
                    value += word;
                    value += "     ";
                }
                text.sections[0].value = value;
            }
        } else {
            error!("Failed to generate keypair from seed as bytes");
        }
    }
}

pub fn handle_event_load_keypair_file(
    mut event_reader: EventReader<EventLoadKeypairFile>,
    // mut text_query: Query<&mut Text, With<TextGeneratedPubkey>>,
    // mut ore_app_state: ResMut<OreAppState>,
    // mut next_state: ResMut<NextState<GameState>>,
    mut set: ParamSet<(
        Query<(&mut Text, &mut TextGeneratedKeypair)>,
        Query<&mut Text, With<TextMnemonicLine1>>,
        Query<&mut Text, With<TextMnemonicLine2>>,
        Query<&mut Text, With<TextMnemonicLine3>>,
    )>,
) {
    for ev in event_reader.read() {
        let path = &ev.0;
        if let Ok(keypair) = read_keypair_file(path) {
            let keypair = Arc::new(keypair);
            let pubkey = keypair.pubkey().to_string();
            for (mut text, mut text_keypair) in set.p0().iter_mut() {
                text.sections[0].value = pubkey.clone();
                text_keypair.0 = keypair.clone();
            }
            for mut text in set.p1().iter_mut() {
                let value = String::new();
                text.sections[0].value = value;
            }
            for mut text in set.p2().iter_mut() {
                let value = String::new();
                text.sections[0].value = value;
            }
            for mut text in set.p3().iter_mut() {
                let value = String::new();
                text.sections[0].value = value;
            }
        } else {
            error!("Error: Failed to load keypair file from path: {}", path.display());
        }

    }
}

pub fn handle_event_save_wallet(
    mut event_reader: EventReader<EventSaveWallet>,
    mut set: ParamSet<(
        Query<&TextGeneratedKeypair>,
        Query<&TextInput, With<TextPasswordInput>>,
    )>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for _ev in event_reader.read() {
        let generated_keypair = set.p0().single().0.clone();

        let password = set.p1().single().text.clone();

        let wallet_path = Path::new("save.data");

        let cocoon = Cocoon::new(password.as_bytes());
        let wallet_bytes = generated_keypair.to_bytes();
        let file = File::create(wallet_path);

        if let Ok(mut file) = file {
            let container = cocoon.dump(wallet_bytes.to_vec(), &mut file);

            if let Ok(_) = container {
                // go to locked screen
                next_state.set(AppScreenState::Unlock);
            } else {
                error!("Error: Failed to save wallet file.");
            }
        } else {
            error!("Error: failed to create file at path: {}", wallet_path.display());
        }
    }
}

pub fn handle_event_request_airdrop(
    mut commands: Commands,
    mut event_reader: EventReader<EventRequestAirdrop>,
    app_wallet: Res<AppWallet>,
    query_task_handler: Query<Entity, With<EntityTaskHandler>>,
    mut next_state: ResMut<NextState<AppScreenState>>,
) {
    for _ev in event_reader.read() {
        let wallet = if let Some(wallet) =  &app_wallet.wallet {
            wallet.clone()
        } else {
            next_state.set(AppScreenState::Unlock);
            error!("wallet is None, switching to wallet unlock screen");
            continue;
        }; 
        if let Ok(task_handler_entity) = query_task_handler.get_single() {
            let pool = IoTaskPool::get();
            let task = pool.spawn(Compat::new(async move {
                let devnet_url = "https://api.devnet.solana.com".to_string();
                let client = RpcClient::new(devnet_url);

                let airdrop_request = client.request_airdrop(&wallet.pubkey(), LAMPORTS_PER_SOL).await;

                match airdrop_request {
                    Ok(sig) => {
                        let process_data = TaskProcessTxData {
                            tx_type: "Airdrop".to_string(),
                            signature: Some(sig),
                            signed_tx: None,
                            hash_time: None,
                        };

                        return Ok(process_data);
                    },
                    Err(e) => {
                        // error!("Failed to request airdrop. handle_event_request_airdrop");
                        // error!("Error: {}", e.to_string());
                        let process_data = TaskProcessTxData {
                            tx_type: "Airdrop".to_string(),
                            signature: None,
                            signed_tx: None,
                            hash_time: None,
                        };

                        return Err((
                            process_data,
                            e.to_string(),
                        ));
                    }
                }
            }));

            commands
                .entity(task_handler_entity)
                .insert(TaskProcessTx { task });
        } else {
            error!("Failed to get task_handler_entity. handle_event_claim_ore_rewards.");
        }
    }
}


#[derive(Event)]
pub struct EventCancelMining;

pub fn handle_event_cancel_mining(
    mut event_reader: EventReader<EventCancelMining>,
    mining_channels_res: Res<MiningDataChannelResource>,
) {
    if let Some(channel_rec) = mining_channels_res.sender.as_ref() {
        for _ev in event_reader.read() {
            let sender = channel_rec.clone();
            let _ = sender.try_send(MiningDataChannelMessage::Stop);
        }
    } else {
        for _ev in event_reader.read() {
            // read and do nothing
        }
    }
}
