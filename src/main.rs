#[macro_use]
extern crate log;

use std::{env, thread, time::Duration};

use anyhow::{anyhow, Result};
use env_logger::Env;
use paho_mqtt as mqtt;

fn data_handler(msg: mqtt::Message) -> bool {
    info!("{}", msg);
    true
}

fn try_reconnect(cli: &mqtt::Client) -> bool {
    warn!("Connection lost. Waiting to retry connection");
    for _ in 0..12 {
        thread::sleep(Duration::from_millis(5000));
        if cli.reconnect().is_ok() {
            info!("Successfully reconnected");
            return true;
        }
    }
    error!("Unable to reconnect after several attempts.");
    false
}

// Create a set of properties with a single Subscription ID
fn sub_id(id: i32) -> mqtt::Properties {
    mqtt::properties![
        mqtt::PropertyCode::SubscriptionIdentifier => id
    ]
}

fn main() -> Result<()> {
    let env = Env::default()
        .filter_or("LOG_LEVEL", "info");

    env_logger::init_from_env(env);

    let hostname = hostname::get()?;

    let topic_prefix = env::args()
        .nth(1)
        .ok_or(anyhow!("Topic prefix required"))?;
    let mqtt_uri = env::args()
        .nth(2)
        .unwrap_or_else(|| "tcp://localhost:1883".to_string());
    let client_id_suffix = env::args()
        .nth(3)
        .unwrap_or_else(|| "".to_string());

    // Create the client. Use an ID for a persistent session.
    let create_opts = mqtt::CreateOptionsBuilder::new()
        .mqtt_version(mqtt::MQTT_VERSION_5)
        .server_uri(mqtt_uri)
        .client_id(format!("mqtt2file-{:?}{}", hostname, client_id_suffix))
        .finalize();

    let cli = mqtt::Client::new(create_opts)?;

    // Initialize the consumer before connecting
    let rx = cli.start_consuming();

    // Request a session that persists for 100 hours (360000sec) between connections
    let conn_opts = mqtt::ConnectOptionsBuilder::new()
        .clean_start(client_id_suffix == "")
        .properties(mqtt::properties![mqtt::PropertyCode::SessionExpiryInterval => 360000])
        .finalize();

    // A table of dispatch function for incoming messages by Subscription ID.
    // (actually sub_id-1 since we can't use zero for a subscription ID)
    let handler: Vec<fn(mqtt::Message) -> bool> = vec![data_handler];

    // Make the connection to the broker
    let rsp = cli.connect(conn_opts)?;

    // We're connecting with a persistent session. So we check if
    // the server already knows about us and remembers about our
    // subscription(s). If not, we subscribe for incoming requests.

    if let Some(conn_rsp) = rsp.connect_response() {
        info!(
            "Connected to: '{}' with MQTT version {}",
            conn_rsp.server_uri, conn_rsp.mqtt_version
        );

        if conn_rsp.session_present {
            info!("  w/ client session already present on broker.");
        } else {
            // Register subscriptions on the server, using Subscription ID's.
            info!("Subscribing to topics...");
            cli.subscribe_with_options(format!("{}/#", topic_prefix), 1, None, sub_id(1))?;
        }
    }

    // ^C handler will stop the consumer, breaking us out of the loop, below
    let ctrlc_cli = cli.clone();
    ctrlc::set_handler(move || {
        ctrlc_cli.stop_consuming();
    })
        .expect("Error setting Ctrl-C handler");

    // Just loop on incoming messages.
    // If we get a None message, check if we got disconnected,
    // and then try a reconnect.
    info!("Waiting for messages...");
    for msg in rx.iter() {
        if let Some(msg) = msg {
            // In a real app you'd want to do a lot more error checking and
            // recovery, but this should give an idea about the basics.

            let sub_id = msg
                .properties()
                .get_int(mqtt::PropertyCode::SubscriptionIdentifier)
                .expect("No Subscription ID") as usize;

            if !handler[sub_id - 1](msg) {
                break;
            }
        } else if cli.is_connected() || !try_reconnect(&cli) {
            break;
        }
    }

    // If we're still connected, then disconnect now,
    // otherwise we're already disconnected.
    if cli.is_connected() {
        info!("Disconnecting");
        cli.disconnect(None).unwrap();
    }
    info!("Exiting");

    Ok(())
}
