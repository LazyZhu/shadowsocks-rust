//! Server side

use std::io;
use std::sync::Arc;

use futures::stream::futures_unordered;
use futures::{Future, Stream};

use config::Config;
use plugin::{launch_plugin, PluginMode};
use relay::boxed_future;
use relay::tcprelay::server::run as run_tcp;
use relay::udprelay::server::run as run_udp;

/// Relay server running on server side.
///
/// ```no_run
/// use shadowsocks::config::{Config, ServerConfig};
/// use shadowsocks::crypto::CipherType;
/// use shadowsocks::relay::server::run;
///
/// let mut config = Config::new();
/// config.server = vec![ServerConfig::basic("127.0.0.1:8388".parse().unwrap(),
///                                          "server-password".to_string(),
///                                          CipherType::Aes256Cfb)];
/// run(config);
/// ```
///
pub fn run(mut config: Config) -> impl Future<Item = (), Error = io::Error> + Send {
    let mut vf = Vec::new();

    if config.enable_udp {
        // Clone config here, because the config for TCP relay will be modified
        // after plugins started
        let udp_config = Arc::new(config.clone());

        // Run UDP relay before starting plugins
        // Because plugins doesn't support UDP relay
        let udp_fut = run_udp(udp_config);
        vf.push(boxed_future(udp_fut));
    }

    // Hold it here, kill all plugins when `tokio::run` is finished
    let plugins = launch_plugin(&mut config, PluginMode::Server).expect("Failed to launch plugins");
    let mon = ::monitor::monitor_signal(plugins);

    // Recreate shared config here
    let config = Arc::new(config);

    let tcp_fut = run_tcp(config.clone());

    vf.push(boxed_future(mon));
    vf.push(boxed_future(tcp_fut));

    futures_unordered(vf).into_future().then(|res| -> io::Result<()> {
                                                 match res {
                                                     Ok(..) => Ok(()),
                                                     Err((err, ..)) => Err(err),
                                                 }
                                             })
}
