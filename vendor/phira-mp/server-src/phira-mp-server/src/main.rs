use std::fs::File;

mod l10n;
pub use l10n::*;

mod room;
pub use room::*;

mod server;
pub use server::*;

mod session;
pub use session::*;

mod ban;
pub use ban::*;

mod web;
use web::start_web_server;
pub use web::MaintenanceConfig;

mod replay;
pub use replay::*;

use anyhow::Result;
use clap::Parser;
use std::{
    collections::{
        HashMap,
        hash_map::{Entry, VacantEntry},
    },
    net::{Ipv4Addr, SocketAddr},
    path::Path,
    sync::Arc,
};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use uuid::Uuid;

pub type SafeMap<K, V> = RwLock<HashMap<K, V>>;
pub type IdMap<V> = SafeMap<Uuid, V>;

fn vacant_entry<V>(map: &mut HashMap<Uuid, V>) -> VacantEntry<'_, Uuid, V> {
    let mut id = Uuid::new_v4();
    while map.contains_key(&id) {
        // 修正此处的语法错误
        id = Uuid::new_v4();
    }
    match map.entry(id) {
        Entry::Vacant(entry) => entry,
        _ => unreachable!(),
    }
}

pub fn init_log(file: &str, log_level: &str) -> Result<WorkerGuard> {
    use tracing::{metadata::LevelFilter, Level};
    use tracing_log::LogTracer;
    use tracing_subscriber::{EnvFilter, filter, fmt, prelude::*};

    let log_dir = Path::new("log");
    if log_dir.exists() {
        if !log_dir.is_dir() {
            panic!("log exists and is not a folder");
        }
    } else {
        std::fs::create_dir(log_dir).expect("failed to create log folder");
    }

    LogTracer::init()?;

    let level = match log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::TRACE,
        "debug" => LevelFilter::DEBUG,
        "warn" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        _ => LevelFilter::INFO,
    };

    // 为文件和控制台输出创建单独的过滤器
    let file_filter = EnvFilter::from_default_env()
        .add_directive(level.into());
    let console_filter = EnvFilter::from_default_env()
        .add_directive(level.into());

    let (non_blocking, guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::hourly(log_dir, file));

    let subscriber = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_filter(file_filter)
        )
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_filter(console_filter)
        )
        .with(
            filter::Targets::new()
                .with_target("hyper", Level::INFO)
                .with_target("rustls", Level::INFO)
                .with_target("isahc", Level::INFO)
                .with_target("tower", Level::INFO)
                .with_default(Level::TRACE)
        );

    tracing::subscriber::set_global_default(subscriber).expect("unable to set global subscriber");
    Ok(guard)
}

/// Command line arguments
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(
        short,
        long,
        default_value_t = 12346,
        help = "Specify the port number to use for the server"
    )]
    port: u16,

    #[clap(
        short,
        long,
        default_value = "info",
        help = "Set the log level (trace, debug, info, warn, error)"
    )]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let game_port = args.port;
    let web_port = game_port + 1; // 恢复为游戏端口+1
    let admin_web_port = 31207; // 管理界面使用固定端口31207
    let _guard = init_log("phira-mp", &args.log_level)?;

    let game_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), game_port);

    // 创建游戏服务器
    let game_listener = TcpListener::bind(&game_addr).await?;
    
    // 创建Server时传入web_port
    let mut config: ServerConfig = File::open("server_config.yml")
        .ok()
        .and_then(|f| serde_yaml::from_reader(f).ok())
        .unwrap_or_default();
    config.web_port = Some(web_port);
    
    let server = Arc::new(Server::new(game_listener, config));

    // 打印服务器地址
    info!("游戏服务器地址: {}", game_addr);
    info!("Web API服务器地址: http://0.0.0.0:{}", web_port);
    info!("Web管理服务器地址: http://0.0.0.0:{}", admin_web_port);

    // 同时运行游戏服务器、API服务器和管理服务器
    tokio::select! {
        game_result = async {
            loop {
                if let Err(err) = server.accept().await {
                    warn!("游戏服务器接受连接失败: {err:?}");
                }
            }
        } => game_result,

        _ = start_web_server(Arc::clone(&server), web_port, admin_web_port) => {},
    };

    Ok(())
}
