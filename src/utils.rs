use std::collections::HashMap;

use log::error;
use rand::{Rng, distributions::WeightedIndex, prelude::Distribution};
use sam_common::AccountId;

use crate::data::Friend;

pub fn normal_friends(friends: &HashMap<String, Friend>) -> HashMap<String, Friend> {
    friends
        .iter()
        .filter(|(_, v)| !v.denim)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn denim_friends(friends: &HashMap<String, Friend>) -> HashMap<String, Friend> {
    friends
        .iter()
        .filter(|(_, v)| v.denim)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn usernames(account_ids: &HashMap<String, AccountId>) -> HashMap<AccountId, String> {
    account_ids
        .iter()
        .map(|(k, v)| (v.clone(), k.clone()))
        .collect()
}

pub fn get_friend<R: Rng>(friends: &HashMap<String, Friend>, rng: &mut R) -> Option<Friend> {
    let values: Vec<&Friend> = friends.values().collect();
    let weights: Vec<f64> = values.iter().map(|f| f.frequency).collect();

    WeightedIndex::new(&weights)
        .inspect_err(|e| error!("{e}"))
        .ok()
        .map(|dist| {
            let index = dist.sample(rng);
            values[index].clone()
        })
}

pub fn random_bytes<R: Rng>(min: u32, max: u32, rng: &mut R) -> Vec<u8> {
    let length = rng.gen_range(min..=max);
    (0..length).map(|_| rng.r#gen()).collect()
}

pub fn sample_prob<R: Rng>(prob: f32, rng: &mut R) -> bool {
    rng.r#gen::<f32>() < prob.clamp(0.0, 1.0)
}
