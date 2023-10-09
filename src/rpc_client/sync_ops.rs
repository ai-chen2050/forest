// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::rpc_api::sync_api::*;
use jsonrpc_v2::Error as JsonRpcError;

use crate::rpc_client::call;

pub async fn sync_check_bad(
    params: SyncCheckBadParams,
    auth_token: &Option<String>,
) -> Result<SyncCheckBadResult, JsonRpcError> {
    call(SYNC_CHECK_BAD, params, auth_token).await
}

pub async fn sync_mark_bad(
    params: SyncMarkBadParams,
    auth_token: &Option<String>,
) -> Result<SyncMarkBadResult, JsonRpcError> {
    call(SYNC_MARK_BAD, params, auth_token).await
}

pub async fn sync_status(
    (): SyncStateParams,
    auth_token: &Option<String>,
) -> Result<SyncStateResult, JsonRpcError> {
    call(SYNC_STATE, (), auth_token).await
}
