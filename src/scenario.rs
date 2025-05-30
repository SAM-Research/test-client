use std::{
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bon::builder;
use log::{error, info, warn};
use rand::{distributions::WeightedIndex, prelude::Distribution, thread_rng};
use sam_client::encryption::DecryptedEnvelope;
use sam_common::AccountId;
use tokio::{
    sync::{Mutex, broadcast::Receiver},
    task::LocalSet,
};

use crate::{
    data::{ClientReport, DispatchData, Friend, MessageLog, MessageType},
    test_client::TestClient,
    timer::Timer,
    utils::{denim_friends, get_friend, normal_friends, random_bytes, sample_prob, usernames},
};

type ArcClient = Arc<Mutex<TestClient>>;
type ArcLogs = Arc<Mutex<Vec<MessageLog>>>;
type ArcBool = Arc<Mutex<bool>>;
type ArcIncoming = Arc<Mutex<Vec<ReplyType>>>;

pub struct ScenarioRunner {
    data: DispatchData,
    client: ArcClient,
    local_set: LocalSet,
    start_time: u128,
    message_logs: ArcLogs,
    stop: ArcBool,
}

impl ScenarioRunner {
    pub fn new(data: DispatchData, client: TestClient) -> Self {
        Self {
            data,
            client: Arc::new(Mutex::new(client)),
            local_set: LocalSet::new(),
            start_time: 0,
            message_logs: ArcLogs::default(),
            stop: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn start(mut self) -> ClientReport {
        self.start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time cannot go backwards")
            .as_millis();
        self.event_loop().await;
        self.local_set.await;
        if let Err(e) = self.client.lock().await.disconnect().await {
            error!("Failed to disconnect: {e}");
        };
        ClientReport {
            start_time: self.start_time,
            messages: self.message_logs.lock().await.clone(),
        }
    }

    async fn event_loop(&self) {
        let tick_time = self.data.client.tick_millis;
        let end_tick = self.data.client.duration_ticks;
        let send_rate = self.data.client.send_rate;
        let reply_rate = self.data.client.reply_rate;
        let client = self.client.clone();
        let msg_log = self.message_logs.clone();
        let friends = &self.data.client.friends;

        let (normal_friends, denim_friends) = if client.lock().await.is_denim() {
            let normal_friends = Rc::new(normal_friends(friends));
            let denim_friends = Rc::new(denim_friends(friends));
            (normal_friends, denim_friends)
        } else {
            (Rc::new(friends.clone()), Rc::default())
        };

        let account_ids = Rc::new(self.data.start.friends.clone());
        let denim_prob = self.data.client.denim_probability;
        let reply_prob = self.data.client.reply_probability;
        let stale_reply = self.data.client.stale_reply;
        let sizes = self.data.client.message_size_range;
        let username = self.data.client.username.clone();

        let stop = self.stop.clone();

        let usernames = Rc::new(usernames(&account_ids));
        let friends = Rc::new(friends.clone());
        let incoming: ArcIncoming = Arc::default();
        let (reg_log, den_log) = {
            let guard = client.lock().await;
            if guard.is_denim() {
                let (reg, den) = (guard.regular_subscribe(), guard.deniable_subscribe());
                let reg_log = recv_logger()
                    .recv(reg)
                    .msg_log(msg_log.clone())
                    .username(username.clone())
                    .usernames(usernames.clone())
                    .msg_type(MessageType::Regular)
                    .start_time(self.start_time)
                    .tick_millis(tick_time)
                    .stop(stop.clone())
                    .incoming(incoming.clone())
                    .call();
                let den_log = recv_logger()
                    .recv(den)
                    .msg_log(msg_log.clone())
                    .username(username.clone())
                    .usernames(usernames.clone())
                    .msg_type(MessageType::Denim)
                    .start_time(self.start_time)
                    .tick_millis(tick_time)
                    .stop(stop.clone())
                    .incoming(incoming.clone())
                    .call();
                (reg_log, Some(den_log))
            } else {
                let reg = guard.regular_subscribe();
                let reg_log = recv_logger()
                    .recv(reg)
                    .msg_log(msg_log.clone())
                    .username(username.clone())
                    .usernames(usernames.clone())
                    .msg_type(MessageType::Regular)
                    .start_time(self.start_time)
                    .tick_millis(tick_time)
                    .stop(stop.clone())
                    .incoming(incoming.clone())
                    .call();
                (reg_log, None)
            }
        };

        self.local_set.spawn_local(reg_log);
        if let Some(logger) = den_log {
            self.local_set.spawn_local(logger);
        }

        self.local_set.spawn_local(async move {
            let mut timer = Timer::new(Duration::from_millis(tick_time.into()), end_tick);
            tokio::task::spawn_local(
                send_message()
                    .username(username.clone())
                    .client(client.clone())
                    .friends(normal_friends.clone())
                    .denim_friends(denim_friends.clone())
                    .account_ids(account_ids.clone())
                    .msg_log(msg_log.clone())
                    .denim_prob(denim_prob)
                    .message_sizes(sizes)
                    .current_tick(timer.current_tick())
                    .call(),
            );
            while timer.next().await {
                let process_client = client.clone();
                tokio::task::spawn_local(async move {
                    if let Err(e) = process_client.lock().await.process_messages().await {
                        error!("Error while processing Message: {e}");
                    }
                });

                if timer.do_action(reply_rate) {
                    tokio::task::spawn_local(
                        reply_message()
                            .username(username.clone())
                            .client(client.clone())
                            .friends(friends.clone())
                            .account_ids(account_ids.clone())
                            .msg_log(msg_log.clone())
                            .message_sizes(sizes)
                            .current_tick(timer.current_tick())
                            .reply_prob(reply_prob)
                            .incoming(incoming.clone())
                            .stale_ticks(stale_reply)
                            .call(),
                    );
                }

                if timer.do_action(send_rate) {
                    tokio::task::spawn_local(
                        send_message()
                            .username(username.clone())
                            .client(client.clone())
                            .friends(normal_friends.clone())
                            .denim_friends(denim_friends.clone())
                            .account_ids(account_ids.clone())
                            .msg_log(msg_log.clone())
                            .denim_prob(denim_prob)
                            .message_sizes(sizes)
                            .current_tick(timer.current_tick())
                            .call(),
                    );
                }
            }
            *stop.lock().await = true;
        });
    }
}

#[derive(Clone, PartialEq, Debug)]
struct IncomingMessage {
    tick: u32,
    from: String,
}

#[derive(Clone, PartialEq, Debug)]
enum ReplyType {
    Denim(IncomingMessage),
    Sam(IncomingMessage),
}

impl ReplyType {
    fn tick(&self) -> u32 {
        match self {
            ReplyType::Denim(incoming_message) => incoming_message,
            ReplyType::Sam(incoming_message) => incoming_message,
        }
        .tick
    }
    fn from(&self) -> &String {
        &match self {
            ReplyType::Denim(incoming_message) => incoming_message,
            ReplyType::Sam(incoming_message) => incoming_message,
        }
        .from
    }
}

#[builder]
async fn recv_logger(
    mut recv: Receiver<DecryptedEnvelope>,
    msg_log: ArcLogs,
    username: String,
    usernames: Rc<HashMap<AccountId, String>>,
    msg_type: MessageType,
    start_time: u128,
    tick_millis: u32,
    incoming: ArcIncoming,
    stop: ArcBool,
) {
    while !*stop.lock().await {
        let timeout_res = tokio::time::timeout(Duration::from_millis(500), recv.recv()).await;
        let recv_res = match timeout_res {
            Ok(res) => res,
            Err(_) => continue,
        };
        let env = match recv_res {
            Ok(env) => env,
            Err(e) => {
                error!("Failed to receive regular from processing: {e}");
                continue;
            }
        };

        let recv_tick = ((env.timestamp() - start_time) / tick_millis as u128) as u32;

        let msg_size = env.content_bytes().len();
        let source = env.source_account_id();
        let from_user = match usernames.get(&source) {
            Some(user) => user,
            None => {
                error!("User with account id '{source}' does not exist");
                continue;
            }
        };

        let msg = IncomingMessage {
            tick: recv_tick,
            from: from_user.clone(),
        };

        let reply = match msg_type {
            MessageType::Denim => ReplyType::Denim(msg),
            MessageType::Regular => ReplyType::Sam(msg),
            MessageType::Other => {
                error!("Received a message that was neither a denim or sam message!");
                continue;
            }
        };

        incoming.lock().await.push(reply);
        info!("Received message from '{from_user}'");
        msg_log.lock().await.push(MessageLog {
            r#type: msg_type.clone(),
            from: from_user.clone(),
            to: username.clone(),
            size: msg_size,
            tick: recv_tick,
        });
    }
}

#[builder]
async fn send_message(
    username: String,
    client: ArcClient,
    friends: Rc<HashMap<String, Friend>>,
    denim_friends: Rc<HashMap<String, Friend>>,
    account_ids: Rc<HashMap<String, AccountId>>,
    msg_log: ArcLogs,
    denim_prob: f32,
    message_sizes: (u32, u32),
    current_tick: u32,
) {
    let (min, max) = message_sizes;
    let mut rng = thread_rng();
    let mut guard = client.lock().await;
    let mut msg_log = msg_log.lock().await;

    let msg = random_bytes(min, max, &mut rng);
    let denim = sample_prob(denim_prob, &mut rng) && denim_friends.len() > 0;

    let friend = if denim && guard.is_denim() {
        get_friend(&denim_friends, &mut rng)
    } else {
        get_friend(&friends, &mut rng)
    };
    let friend_name = friend.as_ref().and_then(|f| Some(f.username.clone()));
    let account_id = friend.and_then(|f| account_ids.get(&f.username));

    let (account_id, friend_name) = match (account_id, friend_name) {
        (Some(f), Some(n)) => (f, n),
        _ => {
            error!("Send Message: Friend does not exist!");
            return;
        }
    };

    let msg_len = msg.len();
    let (res, msg_type) = if denim && guard.is_denim() {
        (
            guard.enqueue_message(*account_id, msg).await,
            MessageType::Denim,
        )
    } else {
        (
            guard.send_message(*account_id, msg).await,
            MessageType::Regular,
        )
    };

    if let Err(e) = res {
        error!("Send Message Client Error: {e}");
        return;
    }
    info!("Sent message to '{friend_name}'");
    msg_log.push(MessageLog {
        r#type: msg_type,
        from: username,
        to: friend_name,
        size: msg_len,
        tick: current_tick,
    });
}

#[builder]
async fn reply_message(
    username: String,
    client: ArcClient,
    friends: Rc<HashMap<String, Friend>>,
    account_ids: Rc<HashMap<String, AccountId>>,
    msg_log: ArcLogs,
    message_sizes: (u32, u32),
    stale_ticks: u32,
    current_tick: u32,
    reply_prob: f32,
    incoming: ArcIncoming,
) {
    let (min, max) = message_sizes;
    let mut rng = thread_rng();
    let mut guard = client.lock().await;
    let mut msg_log = msg_log.lock().await;

    let msg = random_bytes(min, max, &mut rng);
    let mut messages = incoming.lock().await;

    messages.retain(|x| current_tick - x.tick() > stale_ticks);
    if messages.len() == 0 {
        return;
    }
    let weights: Vec<f64> = messages
        .iter()
        .map(|x| {
            let username = x.from();
            friends
                .get(username)
                .and_then(|f| Some(f.frequency))
                .unwrap_or(0.0)
        })
        .collect();

    let index = WeightedIndex::new(&weights)
        .inspect_err(|e| error!("{e}"))
        .ok()
        .map(|dist| {
            let index = dist.sample(&mut rng);
            messages[index].clone()
        });

    let reply = match index {
        Some(reply) => reply,
        None => {
            warn!("Reply Message: Did not get a reply index!");
            return;
        }
    };

    if let Some(pos) = messages.iter().position(|x| x == &reply) {
        messages.remove(pos);
    } else {
        warn!("Reply Message: Could not remove reply");
    }

    if !sample_prob(reply_prob, &mut rng) {
        return;
    }

    let (account_id, friend_name, msg_type) = match reply {
        ReplyType::Denim(msg) => {
            let friend_name = msg.from;
            let account_id = account_ids.get(&friend_name);
            (account_id, friend_name, MessageType::Denim)
        }
        ReplyType::Sam(msg) => {
            let friend_name = msg.from;
            let account_id = account_ids.get(&friend_name);
            (account_id, friend_name, MessageType::Regular)
        }
    };

    let account_id = match account_id {
        Some(x) => x,
        None => {
            error!("Reply Message: Friend does not exist!");
            return;
        }
    };

    let msg_len = msg.len();
    let res = match msg_type {
        MessageType::Denim => guard.enqueue_message(*account_id, msg).await,
        MessageType::Regular => guard.send_message(*account_id, msg).await,
        MessageType::Other => {
            error!("Reply Message: Message reply was not a valid type!");
            return;
        }
    };

    if let Err(e) = res {
        error!("Reply Message Client Error: {e}");
        return;
    }
    info!("Sent reply to '{friend_name}'");
    msg_log.push(MessageLog {
        r#type: msg_type,
        from: username,
        to: friend_name,
        size: msg_len,
        tick: current_tick,
    });
}
