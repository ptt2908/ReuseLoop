#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!(16384, 1258306, 64);

use ckb_std::ckb_constants::Source;
use ckb_std::ckb_types::packed::Script;
use ckb_std::ckb_types::prelude::*;
use ckb_std::error::SysError;
use ckb_std::high_level::{
    load_cell_capacity, load_cell_data, load_cell_lock_hash, load_cell_occupied_capacity,
    load_cell_type, load_cell_type_hash, load_script,
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
    let args = script.args().raw_data();

    if args.len() != STATE_ARGS_LEN || args[0] != SCHEMA_VERSION {
        return Err(ERROR_INVALID_ARGS);
    }

    let item_id = &args[1..33];
    let trusted_registry_type_hash = &args[33..65];
    let admin_lock_hash = &args[65..97];

    // Check group cardinality before loading group cell data:
    // Exactly 1 State GroupInput -> 1 State GroupOutput is required.
    let group_in_count = count_group_cells(Source::GroupInput)?;
    let group_out_count = count_group_cells(Source::GroupOutput)?;

    if group_in_count == 0 || group_out_count == 0 {
        return Err(ERROR_MISSING_CELL);
    }
    if group_in_count > 1 || group_out_count > 1 {
        return Err(ERROR_MULTIPLE_CELLS);
    }

    // Verify State Input and Output Lock Script hashes equal admin_lock_hash
    let in_lock_hash =
        load_cell_lock_hash(0, Source::GroupInput).map_err(|_| ERROR_MISSING_CELL)?;
    let out_lock_hash =
        load_cell_lock_hash(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;
    if &in_lock_hash[..] != admin_lock_hash || &out_lock_hash[..] != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    // Verify capacity preservation and occupied capacity
    let in_cap = load_cell_capacity(0, Source::GroupInput).map_err(|_| ERROR_MISSING_CELL)?;
    let out_cap = load_cell_capacity(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;
    let out_occ =
        load_cell_occupied_capacity(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;
    if out_cap < out_occ || out_cap < in_cap {
        return Err(ERROR_CAPACITY);
    }

    // Validate State Input and Output 34-byte data and status transition
    let input_data = load_cell_data(0, Source::GroupInput).map_err(|_| ERROR_MISSING_CELL)?;
    let output_data = load_cell_data(0, Source::GroupOutput).map_err(|_| ERROR_MISSING_CELL)?;

    if !valid_state_data(&input_data, item_id) || !valid_state_data(&output_data, item_id) {
        return Err(ERROR_INVALID_DATA);
    }

    let old_status = input_data[33];
    let new_status = output_data[33];

    let valid_transition = matches!(
        (old_status, new_status),
        (AVAILABLE, BORROWED) | (BORROWED, RETURNED) | (RETURNED, AVAILABLE)
    );
    if !valid_transition {
        return Err(ERROR_INVALID_TRANSITION);
    }

    // Registry must not appear in transaction Inputs or Outputs during lifecycle transitions
    if has_registry_cell(Source::Input, trusted_registry_type_hash)?
        || has_registry_cell(Source::Output, trusted_registry_type_hash)?
    {
        return Err(ERROR_UNTRUSTED_REGISTRY);
    }

    // Locate and validate the trusted Registry CellDep and verify item_id membership
    validate_registry_celldep(
        &script,
        item_id,
        trusted_registry_type_hash,
        admin_lock_hash,
    )
}

fn validate_registry_celldep(
    state_script: &Script,
    item_id: &[u8],
    trusted_registry_type_hash: &[u8],
    admin_lock_hash: &[u8],
) -> Result<(), i8> {
    let reg_dep_idx = find_unique_registry_celldep(trusted_registry_type_hash)?
        .ok_or(ERROR_UNTRUSTED_REGISTRY)?;

    // Verify Registry CellDep Lock Script hash equals admin_lock_hash
    let reg_lock_hash =
        load_cell_lock_hash(reg_dep_idx, Source::CellDep).map_err(|_| ERROR_MISSING_CELL)?;
    if &reg_lock_hash[..] != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    // Load Registry Type Script and validate its 98-byte args
    let reg_type_script = load_cell_type(reg_dep_idx, Source::CellDep)
        .map_err(|_| ERROR_MISSING_CELL)?
        .ok_or(ERROR_UNTRUSTED_REGISTRY)?;
    let reg_args = reg_type_script.args().raw_data();

    if reg_args.len() != REGISTRY_ARGS_LEN || reg_args[0] != SCHEMA_VERSION {
        return Err(ERROR_INVALID_ARGS);
    }

    let expected_registry_id = &reg_args[1..33];
    let reg_admin_lock_hash = &reg_args[33..65];
    let reg_state_code_hash = &reg_args[65..97];
    let reg_state_hash_type = reg_args[97];

    if reg_admin_lock_hash != admin_lock_hash {
        return Err(ERROR_UNAUTHORIZED);
    }

    if state_script.code_hash().as_slice() != reg_state_code_hash
        || state_script.hash_type().as_slice()[0] != reg_state_hash_type
    {
        return Err(ERROR_UNTRUSTED_REGISTRY);
    }

    // Load and validate Registry CellDep data
    let reg_data = load_cell_data(reg_dep_idx, Source::CellDep).map_err(|_| ERROR_MISSING_CELL)?;
    let item_count = parse_and_validate_registry_data(&reg_data, expected_registry_id)?;

    if !is_item_registered(&reg_data, item_count, item_id) {
        return Err(ERROR_UNTRUSTED_REGISTRY);
    }

    Ok(())
}

fn find_unique_registry_celldep(trusted_registry_type_hash: &[u8]) -> Result<Option<usize>, i8> {
    let mut matched_index: Option<usize> = None;
    let mut index = 0;

    loop {
        match load_cell_type_hash(index, Source::CellDep) {
            Ok(Some(type_hash)) => {
                if &type_hash[..] == trusted_registry_type_hash {
                    if matched_index.is_some() {
                        return Err(ERROR_UNTRUSTED_REGISTRY);
                    }
                    matched_index = Some(index);
                }
                index += 1;
            }
            Ok(None) => {
                index += 1;
            }
            Err(SysError::IndexOutOfBound) => break,
            Err(_) => return Err(ERROR_MISSING_CELL),
        }
    }

    Ok(matched_index)
}

fn has_registry_cell(source: Source, trusted_registry_type_hash: &[u8]) -> Result<bool, i8> {
    let mut index = 0;

    loop {
        match load_cell_type_hash(index, source) {
            Ok(Some(type_hash)) => {
                if &type_hash[..] == trusted_registry_type_hash {
                    return Ok(true);
                }
                index += 1;
            }
            Ok(None) => {
                index += 1;
            }
            Err(SysError::IndexOutOfBound) => return Ok(false),
            Err(_) => return Err(ERROR_MISSING_CELL),
        }
    }
}

fn count_group_cells(source: Source) -> Result<usize, i8> {
    match load_cell_capacity(0, source) {
        Ok(_) => {}
        Err(SysError::IndexOutOfBound) => return Ok(0),
        Err(_) => return Err(ERROR_MISSING_CELL),
    }

    match load_cell_capacity(1, source) {
        Ok(_) => Ok(2),
        Err(SysError::IndexOutOfBound) => Ok(1),
        Err(_) => Err(ERROR_MISSING_CELL),
    }
}

fn valid_state_data(data: &[u8], item_id: &[u8]) -> bool {
    data.len() == STATE_DATA_LEN
        && data[0] == SCHEMA_VERSION
        && &data[1..33] == item_id
        && matches!(data[33], AVAILABLE | BORROWED | RETURNED)
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

fn parse_and_validate_registry_data(data: &[u8], expected_registry_id: &[u8]) -> Result<usize, i8> {
    let count = parse_registry_header(data, expected_registry_id)?;
    if has_duplicate_items(data, count) {
        return Err(ERROR_DUPLICATE_ITEM);
    }
    Ok(count)
}

fn is_item_registered(data: &[u8], item_count: usize, target_item_id: &[u8]) -> bool {
    for i in 0..item_count {
        let start = REGISTRY_HEADER_LEN + i * ITEM_ID_LEN;
        if &data[start..start + ITEM_ID_LEN] == target_item_id {
            return true;
        }
    }
    false
}
