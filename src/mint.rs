
use std::collections::HashMap;
use crate::*;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct NotifData{
    pub id: String,
    pub receiver_info: Option<serde_json::Value>,
    pub action_data: Option<serde_json::Value>,
    pub actioner_info: Option<serde_json::Value>,
    pub action_type: ActionType,
    pub fired_at: Option<i64>, 
    pub is_seen: bool,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum ActionType{ // all the action type that causes the notif to get fired
    #[default]
    Mint
    // probably other system notifs
    // ...
}

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct MintResult{
    pub nft_data: UpdateUserNftRequest,
    pub tx_hash: String,
    pub token_id: String,
    pub status: u8
}


#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct NftPortGetNftResponse{
    pub response: String,
    pub chain: String,
    pub contract_address: String,
    pub token_id: String,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct NftPortMintResponse{
    pub response: String,
    pub chain: String,
    pub contract_address: String,
    pub transaction_hash: String,
    pub transaction_external_url: String,
    pub metadata_uri: String,
    pub mint_to_address: String
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct UpdateUserNftRequest{
    pub caller_cid: String,
    pub col_id: i32,
    pub buyer_screen_cid: Option<String>,
    pub transfer_to_screen_cid: Option<String>,
    pub amount: i64, // amount of gas fee for this call
    pub nft_id: i32,
    pub event_type: String,
    pub contract_address: String,
    pub current_owner_screen_cid: String,
    pub metadata_uri: String,
    pub extra: Option<serde_json::Value>,
    pub attributes: Option<serde_json::Value>,
    pub onchain_id: Option<String>, 
    pub nft_name: String,
    pub is_minted: Option<bool>,
    pub nft_description: String,
    pub current_price: Option<i64>,
    pub is_listed: Option<bool>,
    pub freeze_metadata: Option<bool>,
    pub comments: Option<serde_json::Value>,
    pub likes: Option<serde_json::Value>,
    pub tx_hash: Option<String>,
    pub tx_signature: String,
    pub hash_data: String,
}

pub async fn mint_nft(
    asset_info: UpdateUserNftRequest
) -> (String, String, u8){
    
    /* upload card to ipfs */
    let nftport_token = std::env::var("NFTPORT_TOKEN").unwrap();

    if !asset_info.metadata_uri.is_empty(){

        // metadata_uri_ contains the ipfs and the image path on the server
        // so we have to split it by :: to extract the ipfs url to store 
        // that only onchain otherwise nftport will say:
        // `The amount of data to be saved on the blockchain is too large. 
        // Try making the request smaller by using shorter texts or setting 
        // a base_uri in the contract for long metadata URIs or by reducing 
        // the batch size for batch transactions.`
        let metadata_uri_ = asset_info.clone().metadata_uri;
        let mut splitted_metadata_uri_ = metadata_uri_.split("::");
        let ipfs_metadata_uri_ = splitted_metadata_uri_.next().unwrap();

        /* mint request */
        let mut mint_data = HashMap::new();
        mint_data.insert("chain", "polygon");
        mint_data.insert("contract_address", &asset_info.contract_address);
        mint_data.insert("metadata_uri", ipfs_metadata_uri_);
        let minter_screen_cid = wallexerr::misc::Wallet::generate_keccak256_from(asset_info.clone().caller_cid);
        mint_data.insert("mint_to_address", &minter_screen_cid);
        let nftport_mint_endpoint = format!("https://api.nftport.xyz/v0/mints/customizable");
        let res = reqwest::Client::new()
            .post(nftport_mint_endpoint.as_str())
            .header("Authorization", nftport_token.as_str())
            .json(&mint_data)
            .send()
            .await;


        /* ------------------- NFTPORT RESPONSE HANDLING PROCESS -------------------
            since text() and json() method take the ownership of the instance
            thus can't call text() method on ref_resp which is behind a shared ref 
            cause it'll be moved.
            
            let ref_resp = res.as_ref().unwrap();
            let text_resp = ref_resp.text().await.unwrap();

            to solve this issue first we get the stream of the response chunk
            then map it to the related struct, after that we can handle logging
            and redis caching process without losing ownership of things!
        */
        let get_mint_response = &mut res.unwrap();
        let get_mint_response_bytes = get_mint_response.chunk().await.unwrap();
        let err_resp_vec = get_mint_response_bytes.unwrap().to_vec();
        let get_mint_response_json = serde_json::from_slice::<NftPortMintResponse>(&err_resp_vec);
        /* 
            if we're here means that we couldn't map the bytes into the NftPortMintResponse 
            and perhaps we have errors in response from the nftport service
        */
        if get_mint_response_json.is_err(){

            return (String::from(""), String::from(""), 1);

        }

        let mint_response = get_mint_response_json.unwrap();
        if mint_response.response == String::from("OK"){

            let mint_tx_hash = mint_response.transaction_hash;

            /* sleep till the transaction gets confirmed on blockchain */
            tokio::time::sleep(tokio::time::Duration::from_secs(45)).await;

            let token_id_string = {
    
                /* get minted nft info */
                let nftport_get_nft_endpoint = format!("https://api.nftport.xyz/v0/mints/{}?chain=polygon", mint_tx_hash);
                let res = reqwest::Client::new()
                    .get(nftport_get_nft_endpoint.as_str())
                    .header("Authorization", nftport_token.as_str())
                    .send()
                    .await;


                /* ------------------- NFTPORT RESPONSE HANDLING PROCESS -------------------
                    since text() and json() method take the ownership of the instance
                    thus can't call text() method on ref_resp which is behind a shared ref 
                    cause it'll be moved.
                    
                    let ref_resp = res.as_ref().unwrap();
                    let text_resp = ref_resp.text().await.unwrap();

                    to solve this issue first we get the stream of the response chunk
                    then map it to the related struct, after that we can handle logging
                    and redis caching process without losing ownership of things!
                */
                let get_nft_response = &mut res.unwrap();
                let get_nft_response_bytes = get_nft_response.chunk().await.unwrap();
                let err_resp_vec = get_nft_response_bytes.unwrap().to_vec();
                let get_nft_response_json = serde_json::from_slice::<NftPortGetNftResponse>(&err_resp_vec);
                /* 
                    if we're here means that we couldn't map the bytes into the NftPortGetNftResponse 
                    and perhaps we have errors in response from the nftport service
                */
                if get_nft_response_json.is_err(){
                        
                    return (String::from(""), String::from(""), 1);

                }

                /* log caching using redis */
                let get_nft_response = get_nft_response_json.unwrap();
                log::info!("✅ NftPortGetNftResponse: {:#?}", get_nft_response.clone());

                if get_nft_response.response == String::from("OK"){

                    let token_id = get_nft_response.token_id;
                    log::info!("✅ Nft Minted With Id: {}", token_id.clone());
                    log::info!("✅ Nft Is Inside Contract: {}", asset_info.contract_address.clone());

                    token_id
                    

                } else{
                    String::from("")
                }
            
            };
            
            if mint_tx_hash.starts_with("0x"){

                

                return (mint_tx_hash, token_id_string, 0);
            } else{
                
                
                return (String::from(""), String::from(""), 1);
            }

        } else{

            /* mint wasn't ok */
            
            return (String::from(""), String::from(""), 1);
        }

    } else{

        /* upload in ipfs wasn't ok */
        
        return (String::from(""), String::from(""), 1);

    }

}