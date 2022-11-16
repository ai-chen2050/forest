// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use backoff::{future::retry, ExponentialBackoff};
use blake2b_simd::{Hash, State as Blake2b};
use fvm_shared::sector::SectorSize;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File as SyncFile;
use std::io::{self, copy as sync_copy, BufReader as SyncBufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::BufWriter;
use tokio_util::compat::FuturesAsyncReadCompatExt;

const GATEWAY: &str = "https://proofs.filecoin.io/ipfs/";
const PARAM_DIR: &str = "filecoin-proof-parameters";
const DIR_ENV: &str = "FIL_PROOFS_PARAMETER_CACHE";
const GATEWAY_ENV: &str = "IPFS_GATEWAY";
const TRUST_PARAMS_ENV: &str = "TRUST_PARAMS";
const DEFAULT_PARAMETERS: &str = include_str!("parameters.json");

/// Sector size options for fetching.
pub enum SectorSizeOpt {
    /// All keys and proofs gen parameters
    All,
    /// Only verification parameters
    Keys,
    /// All keys and proofs gen parameters for a given size
    Size(SectorSize),
}

type ParameterMap = HashMap<String, ParameterData>;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ParameterData {
    cid: String,
    digest: String,
    sector_size: u64,
}

// Proof parameter file directory. Defaults to %DATA_DIR/filecoin-proof-parameters unless
// the FIL_PROOFS_PARAMETER_CACHE environment variable is set.
fn param_dir(data_dir: &Path) -> PathBuf {
    std::env::var(PathBuf::from(DIR_ENV))
        .map(PathBuf::from)
        .unwrap_or_else(|_| data_dir.join(PARAM_DIR))
}

/// Forest uses a set of external crates for verifying the proofs generated by the miners.
/// These external crates require a specific set of parameter files to be located at
/// in a specific folder. By default, it is `/var/tmp/filecoin-proof-parameters` but
/// it can be overridden by the `FIL_PROOFS_PARAMETER_CACHE` environment variable.
/// Forest will automatically download the parameter files from IPFS and verify their
/// validity. For consistency, Forest will prefer to download the files it's local data
/// directory. To this end, the `FIL_PROOFS_PARAMETER_CACHE` environment variable is
/// updated before the parameters are downloaded.
///
/// More information available here: <https://github.com/filecoin-project/rust-fil-proofs#parameter-file-location>
pub fn set_proofs_parameter_cache_dir_env(data_dir: &Path) {
    std::env::set_var(DIR_ENV, param_dir(data_dir));
}

/// Get proofs parameters and all verification keys for a given sector size given
/// a parameter JSON manifest.
pub async fn get_params(
    data_dir: &Path,
    param_json: &str,
    storage_size: SectorSizeOpt,
) -> Result<(), anyhow::Error> {
    fs::create_dir_all(param_dir(data_dir)).await?;

    let params: ParameterMap = serde_json::from_str(param_json)?;
    let mut tasks = Vec::with_capacity(params.len());

    params
        .into_iter()
        .filter(|(name, info)| match storage_size {
            SectorSizeOpt::Keys => !name.ends_with("params"),
            SectorSizeOpt::Size(size) => {
                size as u64 == info.sector_size || !name.ends_with(".params")
            }
            SectorSizeOpt::All => true,
        })
        .for_each(|(name, info)| {
            let data_dir_clone = data_dir.to_owned();
            tasks.push(tokio::task::spawn(async move {
                fetch_verify_params(&data_dir_clone, &name, Arc::new(info)).await
            }))
        });

    let mut errors = vec![];

    for t in tasks {
        if let Err(err) = t.await {
            errors.push(err);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        let error_messages: Vec<_> = errors.iter().map(|e| format!("{e}")).collect();
        anyhow::bail!(anyhow::Error::msg(format!(
            "Aggregated errors:\n{}",
            error_messages.join("\n\n")
        )))
    }
}

/// Get proofs parameters and all verification keys for a given sector size using default manifest.
#[inline]
pub async fn get_params_default(
    data_dir: &Path,
    storage_size: SectorSizeOpt,
) -> Result<(), anyhow::Error> {
    get_params(data_dir, DEFAULT_PARAMETERS, storage_size).await
}

async fn fetch_verify_params(
    data_dir: &Path,
    name: &str,
    info: Arc<ParameterData>,
) -> Result<(), anyhow::Error> {
    let path: PathBuf = param_dir(data_dir).join(name);
    let path: Arc<Path> = Arc::from(path.as_path());

    match check_file(path.clone(), info.clone()).await {
        Ok(()) => return Ok(()),
        Err(e) => {
            if e.kind() != ErrorKind::NotFound {
                warn!("Error checking file: {}", e);
            }
        }
    }

    fetch_params(&path, &info).await?;

    check_file(path, info).await.map_err(|e| {
        // TODO remove invalid file
        e.into()
    })
}

async fn fetch_params(path: &Path, info: &ParameterData) -> Result<(), anyhow::Error> {
    let gw = std::env::var(GATEWAY_ENV).unwrap_or_else(|_| GATEWAY.to_owned());
    debug!("Fetching {:?} from {}", path, gw);
    let url = format!("{}{}", gw, info.cid);

    retry(ExponentialBackoff::default(), || async {
        Ok(fetch_params_inner(&url, path).await?)
    })
    .await
}

async fn fetch_params_inner(url: impl AsRef<str>, path: &Path) -> Result<(), anyhow::Error> {
    let client: surf::Client = surf::Config::default().set_timeout(None).try_into()?;
    let req = client.get(url);
    let response = req.await.map_err(|e| anyhow::anyhow!(e))?;
    anyhow::ensure!(response.status().is_success());
    let content_len = response.len();
    let mut source = response.compat();
    let file = File::create(path).await?;
    let mut writer = BufWriter::new(file);
    tokio::io::copy(&mut source, &mut writer).await?;
    let file_metadata = std::fs::metadata(path)?;
    anyhow::ensure!(Some(file_metadata.len() as usize) == content_len);
    Ok(())
}

async fn check_file(path: Arc<Path>, info: Arc<ParameterData>) -> Result<(), io::Error> {
    if std::env::var(TRUST_PARAMS_ENV) == Ok("1".to_owned()) {
        warn!("Assuming parameter files are okay. Do not use in production!");
        return Ok(());
    }

    let cloned_path = path.clone();
    let hash = tokio::task::spawn_blocking(move || -> Result<Hash, io::Error> {
        let file = SyncFile::open(cloned_path.as_ref())?;
        let mut reader = SyncBufReader::new(file);
        let mut hasher = Blake2b::new();
        sync_copy(&mut reader, &mut hasher)?;
        Ok(hasher.finalize())
    })
    .await??;

    let str_sum = hash.to_hex();
    let str_sum = &str_sum[..32];
    if str_sum == info.digest {
        debug!("Parameter file {:?} is ok", path);
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::Other,
            format!(
                "Checksum mismatch in param file {:?}. ({} != {})",
                path, str_sum, info.digest
            ),
        ))
    }
}
