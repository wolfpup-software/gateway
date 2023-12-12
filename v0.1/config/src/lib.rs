use std::collections;
use std::env;
use std::fmt;
use std::fs;
use std::path;

use serde_json;
use serde::{Serialize, Deserialize};

pub enum ConfigError<'a> {
	IoError(std::io::Error),
	JsonError(serde_json::Error),
	GenericError(&'a str),
}

impl fmt::Display for ConfigError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    	match self {
    		ConfigError::IoError(io_error) => write!(f, "{}", io_error),
    		ConfigError::JsonError(json_error) => write!(f, "{}", json_error),
    		ConfigError::GenericError(generic_error) => write!(f, "{}", generic_error),
    	}
    }
}

const CURR_DIR_NOT_FOUND: &str = "could not find working directory";
const CONFIG_NOT_FOUND_ERR: &str = "no config parameters were found at location";

const JSON_FILE_ERR: &str = "config json file failed to load";
const JSON_SERIALIZE_FAILED_ERR: &str = "config json serialization failed";
const JSON_DESERIALIZE_FAILED_ERR: &str = "config json deserialization failed";
const FILEPATH_KEY_ERR: &str = "config did not include an existing key file";
const FILEPATH_CERT_ERR: &str = "config did not include an existing cert flie";

const PARENT_NOT_FOUND_ERR: &str = "parent directory of config not found";


#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub key_filepath: path::PathBuf,
    pub cert_filepath: path::PathBuf,
    pub addresses: collections::HashMap<String, String>,
}
// convert this into a URI
impl Config {
    pub fn from_filepath(filepath: &path::PathBuf) -> Result<Config, ConfigError> {
        // get position relative to working directory
        let working_dir = match env::current_dir() {
            Ok(pb) => pb,
            Err(e) => return Err(ConfigError::IoError(e)),
        };
        
        let config_pathbuff = match combine_pathbuf(&working_dir.to_path_buf(), filepath) {
            Ok(pb) => pb,
            Err(e) => return Err(ConfigError::IoError(e)),
        };

        let parent_dir = match config_pathbuff.parent() {
            Some(p) => p.to_path_buf(),
            _ => return Err(ConfigError::GenericError(PARENT_NOT_FOUND_ERR)),
        };
    
        // build json conifg
        let json_as_str = match fs::read_to_string(&config_pathbuff) {
            Ok(r) => r,
            Err(e) => return Err(ConfigError::IoError(e)),
        };
    
        let config: Config = match serde_json::from_str(&json_as_str) {
            Ok(j) => j,
            Err(e) => return Err(ConfigError::JsonError(e)),
        };
        
        // create key and cert with absolute filepaths from client config
        let key = match combine_pathbuf(
            &parent_dir,
            &config.key_filepath,
        ) {
            Ok(j) => j,
            Err(e) => return Err(ConfigError::IoError(e)),
        };
        if key.is_dir() {
            return Err(ConfigError::GenericError(FILEPATH_KEY_ERR));
        }
        
        let cert = match combine_pathbuf(
            &parent_dir,
            &config.cert_filepath,
        ) {
            Ok(j) => j,
            Err(e) => return Err(ConfigError::IoError(e)),
        };
        if cert.is_dir() {
            return Err(ConfigError::GenericError(FILEPATH_CERT_ERR));
        }
        
        Ok(Config {
            host: config.host,
            port: config.port,
            key_filepath: key,
            cert_filepath: cert,
            addresses: config.addresses,
        })
    }
}

fn combine_pathbuf(
	base_dir: &path::PathBuf,
	filepath: &path::PathBuf,
) -> Result<path::PathBuf, std::io::Error> {
    let mut fp = path::PathBuf::from(&base_dir);
    fp.push(filepath);

    fp.canonicalize()
}

