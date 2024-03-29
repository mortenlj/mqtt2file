#[macro_use]
extern crate log;

use std::thread;
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::ops::Add;
use std::path::Path;

use anyhow::{anyhow, Result};
use env_logger::Env;
use paho_mqtt as mqtt;
use paho_mqtt::{Client, ConnectOptions};
use clap::{Parser, ArgAction};
use crossbeam_channel::RecvTimeoutError;
use chrono::{Local, Duration};

/// Collect messages from mqtt and write them to file
#[derive(Parser, Debug)]
#[clap(author = "Morten Lied Johansen", version, about, long_about = None)]
struct Args {
    /// Prefix of topics to subscribe to
    topic_prefix: String,

    /// Directory to save files into
    directory: String,

    /// MQTT server URI
    #[clap(short = 'u', long = "uri", default_value = "tcp://localhost:1883")]
    mqtt_uri: String,

    /// Client ID suffix. When given, create a persistent session with the client id mqtt2file-<hostname>-<suffix>
    #[clap(short, long, default_value = "")]
    client_id_suffix: String,

    /// Control verbosity of logs. Can be repeated
    #[clap(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Set timeout value in minutes
    #[clap(short, long, default_value = "5")]
    timeout: i64,
}


fn data_handler(msg: mqtt::Message, directory: &String) -> Result<()> {
    let filename = msg.properties()
        .find_user_property("filename")
        .ok_or(anyhow!("No filename in user-property"))?;
    let filepath = Path::new(directory).join(filename);
    info!("Saving message to {:?}", filepath);
    let mut file = File::create(filepath).expect("Create failed!");
    file.write_all(msg.payload()).expect("Failed to write payload!");
    Ok(())
}

fn try_reconnect(cli: &mqtt::Client) -> bool {
    warn!("Connection lost. Waiting to retry connection");
    for _ in 0..12 {
        thread::sleep(Duration::seconds(5).to_std().unwrap());
        if cli.reconnect().is_ok() {
            info!("Successfully reconnected");
            return true;
        }
    }
    error!("Unable to reconnect after several attempts.");
    false
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    init_logging(&args);

    let client_id = create_client_id(&args)?;
    let cli = create_mqtt_client(args.mqtt_uri, client_id)?;

    // Initialize the consumer before connecting
    let rx = cli.start_consuming();
    setup_ctrlc_handler(cli.clone());

    // Make the connection to the broker
    let conn_opts = create_conn_opts(args.client_id_suffix == "");
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
            info!("Subscribing to topics {}/#...", args.topic_prefix);
            cli.subscribe_with_options(format!("{}/#", args.topic_prefix), mqtt::QOS_1, None, None)?;
        }
    }

    // Just loop on incoming messages.
    // If we get a None message, check if we got disconnected,
    // and then try a reconnect.
    let mut consuming = true;
    loop {
        let delay = Duration::minutes(args.timeout);
        let deadline = Local::now().add(delay);
        info!("Waiting for messages until {}...", deadline);
        debug!("   that means {}", delay);
        match rx.recv_timeout(delay.to_std()?) {
            Ok(msg) => {
                // Handle message
                if let Some(msg) = msg {
                    let result = data_handler(msg, &args.directory);
                    if result.is_err() {
                        error!("Error handling message: {}", result.err().unwrap())
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                debug!("Disconnected, trying reconnect");
                if cli.is_connected() || !try_reconnect(&cli) {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if consuming {
                    debug!("Timed out; Stop consumer thread and do a second poll for more messages");
                    cli.stop_consuming();
                    consuming = false;
                } else {
                    debug!("Recevied second timeout, breaking out");
                    break;
                }
            }
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

fn create_conn_opts(use_persistent_session: bool) -> ConnectOptions {
    let conn_opts: ConnectOptions;
    if use_persistent_session {
        conn_opts = mqtt::ConnectOptionsBuilder::new()
            .clean_start(true)
            .finalize();
    } else {
        // Request a session that persists for 100 hours (360000sec) between connections
        conn_opts = mqtt::ConnectOptionsBuilder::new()
            .clean_session(false) // Needs to set this first, to clear the v3 version of the property
            .clean_start(false)
            .properties(mqtt::properties![mqtt::PropertyCode::SessionExpiryInterval => 360000])
            .finalize()
    }
    conn_opts
}


/// Create the client. Use an ID for a persistent session.
fn create_mqtt_client(mqtt_uri: String, client_id: String) -> Result<Client> {
    let create_opts = mqtt::CreateOptionsBuilder::new()
        .mqtt_version(mqtt::MQTT_VERSION_5)
        .server_uri(mqtt_uri)
        .client_id(client_id)
        .finalize();

    let cli = mqtt::Client::new(create_opts)?;
    Ok(cli)
}

fn create_client_id(args: &Args) -> Result<String> {
    let hostname = hostname::get().map_err(|_| anyhow!("Unable to get hostname"))?
        .into_string().map_err(|_| anyhow!("Unable convert hostname to regular string"))?;

    let client_id_prefix = format!("mqtt2file-{}", hostname);
    if args.client_id_suffix != "" {
        return Ok(format!("{}-{}", client_id_prefix, args.client_id_suffix));
    }
    return Ok(client_id_prefix);
}

/// ^C handler will stop the consumer, breaking us out of the loop
fn setup_ctrlc_handler(ctrlc_cli: Client) {
    ctrlc::set_handler(move || {
        ctrlc_cli.stop_consuming();
    })
        .expect("Error setting Ctrl-C handler");
}

/// Configure logging taking verbosity into account
fn init_logging(args: &Args) {
    let log_levels = vec!["error", "warning", "info", "debug"];
    let default_level = 1;
    let actual_level = min(default_level + args.verbose as usize, log_levels.len() - 1);
    let env = Env::default()
        .filter_or("LOG_LEVEL", log_levels[actual_level]);
    env_logger::init_from_env(env);
}
