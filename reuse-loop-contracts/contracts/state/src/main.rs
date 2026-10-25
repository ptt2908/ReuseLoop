#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!(16384, 1258306, 64);

use ckb_std::ckb_constants::Source;
use ckb_std::error::SysError;
use ckb_std::high_level::{load_cell_data, load_script};

const DATA_LEN: usize = 34;
const SCHEMA_VERSION: u8 = 0x01;

const AVAILABLE: u8 = 0;
const BORROWED: u8 = 1;
const RETURNED: u8 = 2;

const ERROR_INVALID_ARGS: i8 = 1;
const ERROR_INVALID_DATA: i8 = 2;
const ERROR_INVALID_TRANSITION: i8 = 3;
const ERROR_MISSING_CELL: i8 = 4;
const ERROR_MULTIPLE_CELLS: i8 = 5;

pub fn program_entry() -> i8 {
    let script = match load_script() {
        Ok(script) => script,
        Err(_) => return ERROR_INVALID_ARGS,
    };

    let args = script.args().raw_data();

    if args.len() != 32 {
        return ERROR_INVALID_ARGS;
    }

    let input_data = match load_cell_data(0, Source::GroupInput) {
        Ok(data) => data,
        Err(_) => return ERROR_MISSING_CELL,
    };

    let output_data = match load_cell_data(0, Source::GroupOutput) {
        Ok(data) => data,
        Err(_) => return ERROR_MISSING_CELL,
    };

    match has_second_cell(Source::GroupInput) {
        Ok(true) => return ERROR_MULTIPLE_CELLS,
        Ok(false) => {}
        Err(_) => return ERROR_MISSING_CELL,
    }

    match has_second_cell(Source::GroupOutput) {
        Ok(true) => return ERROR_MULTIPLE_CELLS,
        Ok(false) => {}
        Err(_) => return ERROR_MISSING_CELL,
    }

    if !valid_data(&input_data, args.as_ref())
        || !valid_data(&output_data, args.as_ref())
    {
        return ERROR_INVALID_DATA;
    }

    let old_status = input_data[33];
    let new_status = output_data[33];

    let valid_transition = matches!(
        (old_status, new_status),
        (AVAILABLE, BORROWED)
            | (BORROWED, RETURNED)
            | (RETURNED, AVAILABLE)
    );

    if !valid_transition {
        return ERROR_INVALID_TRANSITION;
    }

    0
}

fn has_second_cell(source: Source) -> Result<bool, SysError> {
    match load_cell_data(1, source) {
        Ok(_) => Ok(true),
        Err(SysError::IndexOutOfBound) => Ok(false),
        Err(error) => Err(error),
    }
}

fn valid_data(data: &[u8], item_id: &[u8]) -> bool {
    data.len() == DATA_LEN
        && data[0] == SCHEMA_VERSION
        && &data[1..33] == item_id
        && matches!(data[33], AVAILABLE | BORROWED | RETURNED)
}
