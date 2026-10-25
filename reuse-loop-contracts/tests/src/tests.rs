use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
    ckb_types::{bytes::Bytes, core::TransactionBuilder, packed::*, prelude::*},
    context::Context,
};

const AVAILABLE: u8 = 0;
const BORROWED: u8 = 1;
const RETURNED: u8 = 2;
const CAPACITY: u64 = 100_000_000_000;

fn state_data(item_id: &[u8; 32], status: u8) -> Bytes {
    let mut data = Vec::with_capacity(34);
    data.push(0x01);
    data.extend_from_slice(item_id);
    data.push(status);
    Bytes::from(data)
}

fn run_transition(
    old_status: u8,
    new_status: u8,
    output_item_id: [u8; 32],
    output_data_override: Option<Bytes>,
) -> bool {
    let mut context = Context::default();

    let state_out_point = context.deploy_cell_by_name("state");
    let item_id = [7u8; 32];

    let type_script = context
        .build_script(&state_out_point, Bytes::from(item_id.to_vec()))
        .expect("build state type script");

    let lock_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let lock_script = context
        .build_script(&lock_out_point, Bytes::new())
        .expect("build always-success lock script");

    let input_data = state_data(&item_id, old_status);
    let output_data = output_data_override
        .unwrap_or_else(|| state_data(&output_item_id, new_status));

    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(lock_script.clone())
            .type_(Some(type_script.clone()).pack())
            .build(),
        input_data,
    );

    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();

    let output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(lock_script)
        .type_(Some(type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(input)
        .output(output)
        .output_data(output_data)
        .build();

    let tx = context.complete_tx(tx);
    context.verify_tx(&tx, 10_000_000).is_ok()
}

fn run_with_duplicate_state_outputs() -> bool {
    let mut context = Context::default();
    let state_out_point = context.deploy_cell_by_name("state");
    let item_id = [7u8; 32];

    let type_script = context
        .build_script(&state_out_point, Bytes::from(item_id.to_vec()))
        .expect("build state type script");

    let lock_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let lock_script = context
        .build_script(&lock_out_point, Bytes::new())
        .expect("build always-success lock script");

    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(lock_script.clone())
            .type_(Some(type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();

    let output_capacity = CAPACITY / 2;

    let output_1 = CellOutput::new_builder()
        .capacity(output_capacity)
        .lock(lock_script.clone())
        .type_(Some(type_script.clone()).pack())
        .build();

    let output_2 = CellOutput::new_builder()
        .capacity(output_capacity)
        .lock(lock_script)
        .type_(Some(type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(input)
        .output(output_1)
        .output_data(state_data(&item_id, BORROWED))
        .output(output_2)
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = context.complete_tx(tx);
    context.verify_tx(&tx, 10_000_000).is_ok()
}

#[test]
fn allows_available_to_borrowed() {
    assert!(run_transition(AVAILABLE, BORROWED, [7; 32], None));
}

#[test]
fn allows_borrowed_to_returned() {
    assert!(run_transition(BORROWED, RETURNED, [7; 32], None));
}

#[test]
fn allows_returned_to_available() {
    assert!(run_transition(RETURNED, AVAILABLE, [7; 32], None));
}

#[test]
fn rejects_available_to_returned() {
    assert!(!run_transition(AVAILABLE, RETURNED, [7; 32], None));
}

#[test]
fn rejects_available_to_available() {
    assert!(!run_transition(AVAILABLE, AVAILABLE, [7; 32], None));
}

#[test]
fn rejects_changed_item_id() {
    assert!(!run_transition(AVAILABLE, BORROWED, [8; 32], None));
}

#[test]
fn rejects_invalid_schema_version() {
    let mut data = state_data(&[7; 32], BORROWED).to_vec();
    data[0] = 0x02;

    assert!(!run_transition(
        AVAILABLE,
        BORROWED,
        [7; 32],
        Some(Bytes::from(data)),
    ));
}

#[test]
fn rejects_invalid_data_length() {
    let mut data = state_data(&[7; 32], BORROWED).to_vec();
    data.pop();

    assert!(!run_transition(
        AVAILABLE,
        BORROWED,
        [7; 32],
        Some(Bytes::from(data)),
    ));
}

#[test]
fn rejects_invalid_status() {
    let mut data = state_data(&[7; 32], BORROWED).to_vec();
    data[33] = 9;

    assert!(!run_transition(
        AVAILABLE,
        BORROWED,
        [7; 32],
        Some(Bytes::from(data)),
    ));
}

#[test]
fn rejects_duplicate_state_outputs() {
    assert!(!run_with_duplicate_state_outputs());
}
