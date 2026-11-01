#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!(16384, 1258306, 64);

use ckb_hash::new_blake2b;
use ckb_std::ckb_constants::Source;
use ckb_std::ckb_types::packed::Script;
use ckb_std::ckb_types::prelude::*;
use ckb_std::error::SysError;
use ckb_std::high_level::{
    QueryIter, load_cell_capacity, load_cell_data, load_cell_lock_hash,
    load_cell_occupied_capacity, load_cell_type, load_cell_type_hash, load_input, load_script,
    load_script_hash,
};

const REGISTRY_ARGS_LEN: usize = 98;
const STATE_ARGS_LEN: usize = 97;
const REGISTRY_HEADER_LEN: usize = 37;
const STATE_DATA_LEN: usize = 34;
const ITEM_ID_LEN: usize = 32;
const MAX_ITEMS: usize = 100;

const SCHEMA_VERSION: u8 = 0x01;

const AVAILABLE: u8 = 0;
const BORROWED: u8 = 1;
const RETURNED: u8 = 2;

const ERROR_INVALID_ARGS: i8 = 1;
const ERROR_INVALID_DATA: i8 = 2;
const ERROR_INVALID_TRANSITION: i8 = 3;
const ERROR_MISSING_CELL: i8 = 4;
const ERROR_MULTIPLE_CELLS: i8 = 5;
const ERROR_UNAUTHORIZED: i8 = 6;
const ERROR_CAPACITY: i8 = 7;
const ERROR_DUPLICATE_ITEM: i8 = 8;
const ERROR_CAPACITY_LIMIT: i8 = 9;
const ERROR_UNTRUSTED_REGISTRY: i8 = 10;

pub fn program_entry() -> i8 {
    match run() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run() -> Result<(), i8> {
    let script = load_script().map_err(|_| ERROR_INVALID_ARGS)?;
    let self_type_hash = load_script_hash().map_err(|_| ERROR_INVALID_ARGS)?;
    let args = script.args().raw_data();

    if args.len() != REGISTRY_ARGS_LEN || args[0] != SCHEMA_VERSION {
        return Err(ERROR_INVALID_ARGS);
    }

    let registry_id = &args[1..33];
    let admin_lock_hash = &args[33..65];
    let state_code_hash = &args[65..97];
    let state_hash_type = args[97];

    if !matches!(state_hash_type, 0x00 | 0x01 | 0x02 | 0x04) {
        return Err(ERROR_INVALID_ARGS);
    }

    // Check group cardinality before loading group cell data
    let group_in_count = count_group_cells(Source::GroupInput)?;
    let group_out_count = count_group_cells(Source::GroupOutput)?;

    if group_out_count != 1 {
        return Err(ERROR_MISSING_CELL);
    }

    // Verify GroupOutput[0] lock and occupied capacity
    let out_lock_hash =
        load_cell_lock_hash(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;
    if &out_lock_hash[..] != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    let out_cap = load_cell_capacity(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;
    let out_occ =
        load_cell_occupied_capacity(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;
    if out_cap < out_occ {
        return Err(ERROR_CAPACITY);
    }

    // Ensure no canonical State cells are consumed in Source::Input during Registry operations
    if has_canonical_state_cell(Source::Input, state_code_hash, state_hash_type)? {
        return Err(ERROR_INVALID_TRANSITION);
    }

    if group_in_count == 0 {
        validate_genesis(
            registry_id,
            admin_lock_hash,
            state_code_hash,
            state_hash_type,
            &self_type_hash,
        )
    } else {
        validate_registration(
            registry_id,
            admin_lock_hash,
            state_code_hash,
            state_hash_type,
            &self_type_hash,
            out_cap,
        )
    }
}

fn validate_genesis(
    registry_id: &[u8],
    admin_lock_hash: &[u8],
    state_code_hash: &[u8],
    state_hash_type: u8,
    self_type_hash: &[u8; 32],
) -> Result<(), i8> {
    // Input[0] itself must use admin_lock_hash
    let first_input_lock =
        load_cell_lock_hash(0, Source::Input).map_err(|_| ERROR_MISSING_CELL)?;
    if &first_input_lock[..] != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    // Verify CKB Type ID (52-byte preimage: 44-byte Input[0] + 8-byte global output index)
    verify_registry_type_id(registry_id, self_type_hash)?;

    // Validate initial Registry cell data (must have 0 items)
    let out_data = load_cell_data(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;
    let out_count = parse_and_validate_registry_data(&out_data, registry_id)?;
    if out_count != 0 {
        return Err(ERROR_INVALID_DATA);
    }

    // Ensure no canonical State cells are created during Registry initialization
    if has_canonical_state_cell(Source::Output, state_code_hash, state_hash_type)? {
        return Err(ERROR_INVALID_TRANSITION);
    }

    Ok(())
}

fn validate_registration(
    registry_id: &[u8],
    admin_lock_hash: &[u8],
    state_code_hash: &[u8],
    state_hash_type: u8,
    self_type_hash: &[u8; 32],
    out_cap: u64,
) -> Result<(), i8> {
    // Verify GroupInput[0] lock and capacity preservation
    let in_lock_hash =
        load_cell_lock_hash(0, Source::GroupInput).map_err(|_| ERROR_MISSING_CELL)?;
    if &in_lock_hash[..] != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    let in_cap = load_cell_capacity(0, Source::GroupInput).map_err(|_| ERROR_MISSING_CELL)?;
    if out_cap < in_cap {
        return Err(ERROR_CAPACITY);
    }

    let in_data = load_cell_data(0, Source::GroupInput).map_err(|_| ERROR_MISSING_CELL)?;
    let out_data = load_cell_data(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;

    let in_count = parse_registry_header(&in_data, registry_id)?;
    if has_duplicate_items(&in_data, in_count) {
        return Err(ERROR_INVALID_DATA);
    }

    let out_count = parse_registry_header(&out_data, registry_id)?;
    if out_count != in_count + 1 {
        return Err(ERROR_INVALID_TRANSITION);
    }

    let prefix_end = REGISTRY_HEADER_LEN + ITEM_ID_LEN * in_count;
    if out_data[REGISTRY_HEADER_LEN..prefix_end] != in_data[REGISTRY_HEADER_LEN..prefix_end] {
        return Err(ERROR_INVALID_TRANSITION);
    }

    let new_item_id = &out_data[prefix_end..prefix_end + ITEM_ID_LEN];
    for idx in 0..in_count {
        let start = REGISTRY_HEADER_LEN + idx * ITEM_ID_LEN;
        if &in_data[start..start + ITEM_ID_LEN] == new_item_id {
            return Err(ERROR_DUPLICATE_ITEM);
        }
    }

    // Verify canonical State output cell
    verify_canonical_state_output(
        new_item_id,
        self_type_hash,
        admin_lock_hash,
        state_code_hash,
        state_hash_type,
    )
}

fn verify_registry_type_id(
    expected_registry_id: &[u8],
    self_type_hash: &[u8; 32],
) -> Result<(), i8> {
    let first_input = load_input(0, Source::Input).map_err(|_| ERROR_MISSING_CELL)?;

    let mut matched_output_index: Option<u64> = None;
    for (idx, type_hash_opt) in QueryIter::new(load_cell_type_hash, Source::Output).enumerate() {
        if type_hash_opt.as_ref() == Some(self_type_hash) {
            if matched_output_index.is_some() {
                return Err(ERROR_MULTIPLE_CELLS);
            }
            matched_output_index = Some(idx as u64);
        }
    }
    let global_output_index = matched_output_index.ok_or(ERROR_MISSING_CELL)?;

    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&global_output_index.to_le_bytes());
    let mut derived_registry_id = [0u8; 32];
    blake2b.finalize(&mut derived_registry_id);

    if &derived_registry_id[..] != expected_registry_id {
        return Err(ERROR_INVALID_ARGS);
    }

    Ok(())
}

fn verify_canonical_state_output(
    new_item_id: &[u8],
    self_type_hash: &[u8; 32],
    admin_lock_hash: &[u8],
    state_code_hash: &[u8],
    state_hash_type: u8,
) -> Result<(), i8> {
    let mut matched_state_output: Option<(usize, Script)> = None;

    for (idx, type_opt) in QueryIter::new(load_cell_type, Source::Output).enumerate() {
        if let Some(type_script) = type_opt
            && is_canonical_state_script(&type_script, state_code_hash, state_hash_type)
        {
            if matched_state_output.is_some() {
                return Err(ERROR_MULTIPLE_CELLS);
            }
            matched_state_output = Some((idx, type_script));
        }
    }

    let (state_idx, state_type_script) = matched_state_output.ok_or(ERROR_MISSING_CELL)?;

    // Validate 97-byte State Type Script args:
    // version (1B) || item_id (32B) || trusted_registry_type_hash (32B) || admin_lock_hash (32B)
    let state_args = state_type_script.args().raw_data();
    if state_args.len() != STATE_ARGS_LEN || state_args[0] != SCHEMA_VERSION {
        return Err(ERROR_INVALID_ARGS);
    }
    if &state_args[1..33] != new_item_id {
        return Err(ERROR_INVALID_ARGS);
    }
    if state_args[33..65] != self_type_hash[..] {
        return Err(ERROR_UNTRUSTED_REGISTRY);
    }
    if &state_args[65..97] != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    // Validate State output Lock Script
    let state_lock_hash =
        load_cell_lock_hash(state_idx, Source::Output).map_err(|_| ERROR_MISSING_CELL)?;
    if &state_lock_hash[..] != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    // Validate State output occupied capacity
    let state_cap =
        load_cell_capacity(state_idx, Source::Output).map_err(|_| ERROR_MISSING_CELL)?;
    let state_occ =
        load_cell_occupied_capacity(state_idx, Source::Output).map_err(|_| ERROR_MISSING_CELL)?;
    if state_cap < state_occ {
        return Err(ERROR_CAPACITY);
    }

    // Validate 34-byte State Cell data and AVAILABLE (0x00) initial status
    let state_data = load_cell_data(state_idx, Source::Output).map_err(|_| ERROR_MISSING_CELL)?;
    if state_data.len() != STATE_DATA_LEN
        || state_data[0] != SCHEMA_VERSION
        || &state_data[1..33] != new_item_id
        || !matches!(state_data[33], AVAILABLE | BORROWED | RETURNED)
    {
        return Err(ERROR_INVALID_DATA);
    }

    if state_data[33] != AVAILABLE {
        return Err(ERROR_INVALID_TRANSITION);
    }

    Ok(())
}

fn count_group_cells(source: Source) -> Result<usize, i8> {
    match load_cell_capacity(0, source) {
        Ok(_) => {}
        Err(SysError::IndexOutOfBound) => return Ok(0),
        Err(_) => return Err(ERROR_MISSING_CELL),
    }

    match load_cell_capacity(1, source) {
        Ok(_) => Err(ERROR_MULTIPLE_CELLS),
        Err(SysError::IndexOutOfBound) => Ok(1),
        Err(_) => Err(ERROR_MISSING_CELL),
    }
}

fn is_canonical_state_script(
    script: &Script,
    state_code_hash: &[u8],
    state_hash_type: u8,
) -> bool {
    script.code_hash().as_slice() == state_code_hash
        && script.hash_type().as_slice()[0] == state_hash_type
}

fn has_canonical_state_cell(
    source: Source,
    state_code_hash: &[u8],
    state_hash_type: u8,
) -> Result<bool, i8> {
    for type_script in QueryIter::new(load_cell_type, source).flatten() {
        if is_canonical_state_script(&type_script, state_code_hash, state_hash_type) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_registry_header(data: &[u8], expected_registry_id: &[u8]) -> Result<usize, i8> {
    if data.len() < REGISTRY_HEADER_LEN {
        return Err(ERROR_INVALID_DATA);
    }
    if data[0] != SCHEMA_VERSION || &data[1..33] != expected_registry_id {
        return Err(ERROR_INVALID_DATA);
    }

    let item_count = u32::from_le_bytes([data[33], data[34], data[35], data[36]]) as usize;
    if item_count > MAX_ITEMS {
        return Err(ERROR_CAPACITY_LIMIT);
    }

    let expected_len = REGISTRY_HEADER_LEN + ITEM_ID_LEN * item_count;
    if data.len() != expected_len {
        return Err(ERROR_INVALID_DATA);
    }

    Ok(item_count)
}

fn has_duplicate_items(data: &[u8], item_count: usize) -> bool {
    for i in 0..item_count {
        let start_i = REGISTRY_HEADER_LEN + i * ITEM_ID_LEN;
        let id_i = &data[start_i..start_i + ITEM_ID_LEN];
        for j in (i + 1)..item_count {
            let start_j = REGISTRY_HEADER_LEN + j * ITEM_ID_LEN;
            let id_j = &data[start_j..start_j + ITEM_ID_LEN];
            if id_i == id_j {
                return true;
            }
        }
    }
    false
}

fn parse_and_validate_registry_data(
    data: &[u8],
    expected_registry_id: &[u8],
) -> Result<usize, i8> {
    let count = parse_registry_header(data, expected_registry_id)?;
    if has_duplicate_items(data, count) {
        return Err(ERROR_DUPLICATE_ITEM);
    }
    Ok(count)
}

