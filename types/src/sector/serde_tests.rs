// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::{
    OnChainWindowPoStVerifyInfo, PoStProof, SealVerifyInfo, SealVerifyParams, WindowPoStVerifyInfo,
};
use cid::{multihash::Identity, Cid};
use encoding::{from_slice, to_vec};

fn empty_cid() -> Cid {
    Cid::new_from_cbor(&[], Identity)
}

#[test]
fn default_serializations() {
    let ocs = SealVerifyParams {
        sealed_cid: empty_cid(),
        ..Default::default()
    };
    let bz = to_vec(&ocs).unwrap();
    assert_eq!(from_slice::<SealVerifyParams>(&bz).unwrap(), ocs);

    let s = SealVerifyInfo {
        unsealed_cid: empty_cid(),
        sealed_cid: empty_cid(),
        ..Default::default()
    };
    let bz = to_vec(&s).unwrap();
    assert_eq!(from_slice::<SealVerifyInfo>(&bz).unwrap(), s);

    let s = WindowPoStVerifyInfo::default();
    let bz = to_vec(&s).unwrap();
    assert_eq!(from_slice::<WindowPoStVerifyInfo>(&bz).unwrap(), s);

    let s = PoStProof::default();
    let bz = to_vec(&s).unwrap();
    assert_eq!(from_slice::<PoStProof>(&bz).unwrap(), s);

    let s = PoStProof::default();
    let bz = to_vec(&s).unwrap();
    assert_eq!(from_slice::<PoStProof>(&bz).unwrap(), s);

    let s = OnChainWindowPoStVerifyInfo::default();
    let bz = to_vec(&s).unwrap();
    assert_eq!(from_slice::<OnChainWindowPoStVerifyInfo>(&bz).unwrap(), s);
}
