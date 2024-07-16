use http::Uri;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fmt;
use std::path;
use std::path::PathBuf;
use tokio::fs;

pub enum TargetAddress {
    Safe,
    Dangerous,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    pub host_and_port: String,
    pub key_filepath: PathBuf,
    pub cert_filepath: PathBuf,
    pub addresses: Vec<(String, String)>,
    pub dangerous_unsigned_addresses: Vec<(String, String)>,
}

pub enum ConfigError<'a> {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    UriError(<http::Uri as TryFrom<String>>::Error),
    Error(&'a str),
}

impl fmt::Display for ConfigError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::IoError(io_error) => write!(f, "{}", io_error),
            ConfigError::UriError(json_error) => write!(f, "{}", json_error),
            ConfigError::JsonError(json_error) => write!(f, "{}", json_error),
            ConfigError::Error(error) => write!(f, "{}", error),
        }
    }
}

const HTTP: &str = "http";

pub async fn from_filepath(filepath: &PathBuf) -> Result<Config, ConfigError> {
    // get position relative to working directory
    let config_path = match path::absolute(filepath) {
        Ok(pb) => pb,
        Err(e) => return Err(ConfigError::IoError(e)),
    };

    let parent_dir = match config_path.parent() {
        Some(p) => p.to_path_buf(),
        _ => return Err(ConfigError::Error("parent directory of config not found")),
    };

    // build json conifg
    let json_as_str = match fs::read_to_string(&config_path).await {
        Ok(r) => r,
        Err(e) => return Err(ConfigError::IoError(e)),
    };
    let config: Config = match serde_json::from_str(&json_as_str) {
        Ok(j) => j,
        Err(e) => return Err(ConfigError::JsonError(e)),
    };

    // create absolute filepaths for key and cert
    let key = match path::absolute(parent_dir.join(&config.key_filepath)) {
        Ok(j) => j,
        Err(e) => return Err(ConfigError::IoError(e)),
    };
    if key.is_dir() {
        return Err(ConfigError::Error(
            "failed to create absolute path from relative path for key_filepath",
        ));
    }

    let cert = match path::absolute(parent_dir.join(&config.cert_filepath)) {
        Ok(j) => j,
        Err(e) => return Err(ConfigError::IoError(e)),
    };
    if cert.is_dir() {
        return Err(ConfigError::Error(
            "failed to create absolute path from relative path for cert_filepath",
        ));
    }

    Ok(Config {
        host_and_port: config.host_and_port,
        key_filepath: key,
        cert_filepath: cert,
        addresses: config.addresses,
        dangerous_unsigned_addresses: config.dangerous_unsigned_addresses,
    })
}

// Now we need some qualities
// host => URI, safe, dangerous
// host => uri
// Map<URI host, destination URI>.
// ie: Map<example.com, http://some_address:6789>
pub fn create_address_map(config: &Config) -> Result<HashMap<String, (Uri, bool)>, ConfigError> {
    let mut hashmap = HashMap::<String, (Uri, bool)>::new();
    if let Err(e) = create_address_map_bit(&mut hashmap, &config.addresses, false) {
        return Err(e);
    };
    if let Err(e) = create_address_map_bit(&mut hashmap, &config.dangerous_unsigned_addresses, true)
    {
        return Err(e);
    };

    Ok(hashmap)
}

pub fn create_address_map_bit<'a>(
    url_map: &mut HashMap<String, (Uri, bool)>,
    addresses: &Vec<(String, String)>,
    is_dangerous: bool,
) -> Result<(), ConfigError<'a>> {
    for (arrival_str, dest_str) in addresses {
        let arrival_uri = match Uri::try_from(arrival_str) {
            Ok(uri) => uri,
            Err(e) => return Err(ConfigError::UriError(e)),
        };

        // get port if available
        let mut host = match get_host_and_port(&arrival_uri) {
            Ok(h) => h.to_string(),
            Err(e) => return Err(e),
        };

        // no need to remove path and query, it is replaced later
        let dest_uri = match Uri::try_from(dest_str) {
            Ok(uri) => uri,
            Err(e) => return Err(ConfigError::UriError(e)),
        };

        url_map.insert(host, (dest_uri, is_dangerous));
    }
    Ok(())
}

// use same function to pull host and stuff as retreival
// get host and port
pub fn get_host_and_port<'a>(uri: &Uri) -> Result<String, ConfigError<'a>> {
    // get port if available
    let host = match uri.host() {
        Some(h) => h,
        _ => return Err(ConfigError::Error("could not parse hosts from addresses")),
    };

    let scheme = match uri.scheme() {
        Some(h) => h.as_str(),
        _ => return Err(ConfigError::Error("could not parse hosts from addresses")),
    };

    let port = match uri.port() {
        Some(p) => p.to_string(),
        _ => match scheme {
            "https" => "443".to_string(),
            _ => "80".to_string(),
        },
    };

    Ok(host.to_string() + ":" + &port)
}
