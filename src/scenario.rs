use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bon::builder;
use log::error;
use rand::thread_rng;
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
    utils::{denim_friends, get_friend, is_denim, normal_friends, random_bytes, usernames},
};

type ArcClient = Arc<Mutex<TestClient>>;
type RefLogs = Rc<RefCell<Vec<MessageLog>>>;
type RefBool = Rc<RefCell<bool>>;

pub struct ScenarioRunner {
    data: DispatchData,
    client: ArcClient,
    local_set: LocalSet,
    start_time: u64,
    message_logs: RefLogs,
    stop: RefBool,
}

impl ScenarioRunner {
    pub fn new(data: DispatchData, client: TestClient) -> Self {
        Self {
            data,
            client: Arc::new(Mutex::new(client)),
            local_set: LocalSet::new(),
            start_time: 0,
            message_logs: RefLogs::default(),
            stop: Rc::new(RefCell::new(false)),
        }
    }

    pub async fn start(mut self) -> ClientReport {
        self.start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time cannot go backwards")
            .as_secs();
        self.event_loop().await;
        self.local_set.await;

        ClientReport {
            start_time: self.start_time,
            messages: self.message_logs.borrow().clone(),
        }
    }

    async fn event_loop(&self) {
        let tick_time = self.data.client.tick_millis;
        let end_tick = self.data.client.duration_ticks;
        let action_rate = self.data.client.send_rate;
        let client = self.client.clone();
        let msg_log = self.message_logs.clone();
        let friends = &self.data.client.friends;

        let normal_friends = Rc::new(normal_friends(friends));
        let denim_friends = Rc::new(denim_friends(friends));
        let account_ids = Rc::new(self.data.start.friends.clone());
        let denim_prob = self.data.client.denim_probability;
        let sizes = self.data.client.message_size_range;
        let username = self.data.client.username.clone();

        let stop = self.stop.clone();

        let usernames = Rc::new(usernames(&account_ids));

        let (reg, den) = {
            let bclient = client.lock().await;
            (bclient.regular_subscribe(), bclient.deniable_subscribe())
        };

        let start_millis = self.start_time as u128 * 1000;

        let denim_logger = recv_logger()
            .recv(den)
            .msg_log(msg_log.clone())
            .username(username.clone())
            .usernames(usernames.clone())
            .msg_type(MessageType::Denim)
            .start_time(start_millis)
            .tick_millis(tick_time)
            .stop(stop.clone())
            .call();
        let regular_logger = recv_logger()
            .recv(reg)
            .msg_log(msg_log.clone())
            .username(username.clone())
            .usernames(usernames.clone())
            .msg_type(MessageType::Regular)
            .start_time(start_millis)
            .tick_millis(tick_time)
            .stop(stop.clone())
            .call();

        self.local_set.spawn_local(denim_logger);
        self.local_set.spawn_local(regular_logger);

        self.local_set.spawn_local(async move {
            let mut timer = Timer::new(
                Duration::from_millis(tick_time.into()),
                end_tick,
                action_rate,
            );

            while timer.next().await {
                let process_client = client.clone();
                tokio::task::spawn_local(async move {
                    if let Err(e) = process_client.lock().await.process_messages().await {
                        error!("Error while processing Message: {e}");
                    }
                });

                if !timer.do_action() {
                    continue;
                }
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
            *stop.borrow_mut() = true;
        });
    }
}

#[builder]
async fn recv_logger(
    mut recv: Receiver<DecryptedEnvelope>,
    msg_log: RefLogs,
    username: String,
    usernames: Rc<HashMap<AccountId, String>>,
    msg_type: MessageType,
    start_time: u128,
    tick_millis: u32,
    stop: RefBool,
) {
    while !*stop.borrow() {
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

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time cannot go backwards")
            .as_millis();
        // estimate tick as this runs outside the event loop
        let recv_tick = ((now - start_time) / tick_millis as u128) as u32;

        let msg_size = env.content_bytes().len();
        let source = env.source_account_id();
        let from_user = match usernames.get(&source) {
            Some(user) => user,
            None => {
                error!("User with account id '{source}' does not exist");
                continue;
            }
        };

        msg_log.borrow_mut().push(MessageLog {
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
    msg_log: RefLogs,
    denim_prob: f32,
    message_sizes: (u32, u32),
    current_tick: u32,
) {
    let (min, max) = message_sizes;
    let mut rng = thread_rng();
    let mut guard = client.lock().await;
    let mut msg_log = msg_log.borrow_mut();

    let msg = random_bytes(min, max, &mut rng);
    let denim = is_denim(denim_prob, &mut rng);

    let friend = if denim {
        get_friend(&denim_friends, &mut rng)
    } else {
        get_friend(&friends, &mut rng)
    };

    let friend_name = friend.as_ref().and_then(|f| Some(f.username.clone()));
    let account_id = friend.and_then(|f| account_ids.get(&f.username));

    let (account_id, friend_name) = match (account_id, friend_name) {
        (Some(f), Some(n)) => (f, n),
        _ => {
            error!("Friend does not exist!");
            return;
        }
    };

    let msg_len = msg.len();
    let (res, msg_type) = if denim {
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
        error!("{e}");
    }
    msg_log.push(MessageLog {
        r#type: msg_type,
        from: username,
        to: friend_name,
        size: msg_len,
        tick: current_tick,
    });
}
