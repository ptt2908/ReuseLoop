use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
    ckb_error::Error,
    ckb_hash::new_blake2b,
    ckb_types::{
        bytes::Bytes,
        core::{ScriptHashType, TransactionBuilder},
        packed::*,
        prelude::*,
    },
    context::{Context, random_type_id_script},
};

const AVAILABLE: u8 = 0;
const BORROWED: u8 = 1;
const RETURNED: u8 = 2;
const CAPACITY: u64 = 100_000_000_000;
const MAX_CYCLES: u64 = 10_000_000;

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

fn state_data(item_id: &[u8; 32], status: u8) -> Bytes {
    let mut data = Vec::with_capacity(34);
    data.push(0x01);
    data.extend_from_slice(item_id);
    data.push(status);
    Bytes::from(data)
}

fn registry_args(
    version: u8,
    registry_id: &[u8; 32],
    admin_lock_hash: &[u8; 32],
    state_code_hash: &[u8; 32],
    state_hash_type: u8,
) -> Bytes {
    let mut args = Vec::with_capacity(98);
    args.push(version);
    args.extend_from_slice(registry_id);
    args.extend_from_slice(admin_lock_hash);
    args.extend_from_slice(state_code_hash);
    args.push(state_hash_type);
    Bytes::from(args)
}

fn state_spec_args(
    version: u8,
    item_id: &[u8; 32],
    trusted_registry_type_hash: &[u8; 32],
    admin_lock_hash: &[u8; 32],
) -> Bytes {
    let mut args = Vec::with_capacity(97);
    args.push(version);
    args.extend_from_slice(item_id);
    args.extend_from_slice(trusted_registry_type_hash);
    args.extend_from_slice(admin_lock_hash);
    Bytes::from(args)
}

fn registry_data(schema_version: u8, registry_id: &[u8; 32], items: &[[u8; 32]]) -> Bytes {
    let mut data = Vec::with_capacity(37 + 32 * items.len());
    data.push(schema_version);
    data.extend_from_slice(registry_id);
    data.extend_from_slice(&(items.len() as u32).to_le_bytes());
    for item in items {
        data.extend_from_slice(item);
    }
    Bytes::from(data)
}

fn derive_registry_id(first_cell_input: &CellInput, global_output_index: u64) -> [u8; 32] {
    assert_eq!(first_cell_input.as_slice().len(), 44);
    let mut blake2b = new_blake2b();
    blake2b.update(first_cell_input.as_slice());
    blake2b.update(&global_output_index.to_le_bytes());
    let mut ret = [0u8; 32];
    blake2b.finalize(&mut ret);
    ret
}

fn byte32_to_array(val: &Byte32) -> [u8; 32] {
    let mut arr = [0u8; 32];
    arr.copy_from_slice(val.as_slice());
    arr
}

fn assert_script_error(result: Result<u64, Error>, expected_code: i8) {
    let err = result.expect_err("expected transaction verification to fail");
    let msg = err.to_string();
    let expected_pattern = format!("error code {expected_code} ");
    assert!(
        msg.contains(&expected_pattern),
        "expected script error code {expected_code}, but got: {msg}"
    );
}

struct RegistryTestFixture {
    context: Context,
    registry_out_point: OutPoint,
    admin_lock_script: Script,
    admin_lock_hash: [u8; 32],
    other_lock_script: Script,
    state_contract_out_point: OutPoint,
    state_code_hash: [u8; 32],
    state_hash_type: u8,
}

impl RegistryTestFixture {
    fn new() -> Self {
        let mut context = Context::default();
        let registry_out_point = context.deploy_cell_by_name("registry");
        let lock_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

        let admin_lock_script = context
            .build_script(&lock_out_point, Bytes::from(vec![0xAA; 20]))
            .expect("build admin lock script");
        let admin_lock_hash = byte32_to_array(&admin_lock_script.calc_script_hash());

        let other_lock_script = context
            .build_script(&lock_out_point, Bytes::from(vec![0xBB; 20]))
            .expect("build other lock script");

        // Deploy a canonical State script cell backed by ALWAYS_SUCCESS with a unique Type ID
        // so that State outputs can be verified independently of the Week 9 State binary.
        let state_type_id_script = random_type_id_script();
        let state_code_hash = byte32_to_array(&state_type_id_script.calc_script_hash());
        let state_contract_out_point = context.create_cell(
            CellOutput::new_builder()
                .capacity(1000 * 100_000_000u64)
                .lock(admin_lock_script.clone())
                .type_(Some(state_type_id_script).pack())
                .build(),
            ALWAYS_SUCCESS.clone(),
        );

        Self {
            context,
            registry_out_point,
            admin_lock_script,
            admin_lock_hash,
            other_lock_script,
            state_contract_out_point,
            state_code_hash,
            state_hash_type: ScriptHashType::Type.into(),
        }
    }

    fn build_registry_script(&mut self, args: Bytes) -> Script {
        self.context
            .build_script(&self.registry_out_point, args)
            .expect("build registry type script")
    }

    fn build_canonical_state_script(&mut self, args: Bytes) -> Script {
        self.context
            .build_script_with_hash_type(
                &self.state_contract_out_point,
                ScriptHashType::Type,
                args,
            )
            .expect("build canonical state type script")
    }
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
    context.verify_tx(&tx, MAX_CYCLES).is_ok()
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
    context.verify_tx(&tx, MAX_CYCLES).is_ok()
}

// ============================================================================
// Week 9 State Script Baseline Tests (Preserved)
// ============================================================================

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

// ============================================================================
// Week 10 CP 10.2: Registry Genesis Tests (0 GroupInput -> 1 GroupOutput)
// ============================================================================

#[test]
fn registry_genesis_allows_valid_type_id() {
    let mut f = RegistryTestFixture::new();
    let seed_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let first_input = CellInput::new_builder()
        .previous_output(seed_out_point)
        .since(0u64)
        .build();

    let registry_id = derive_registry_id(&first_input, 0);
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);

    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(first_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[]))
        .build();

    let tx = f.context.complete_tx(tx);
    assert!(f.context.verify_tx(&tx, MAX_CYCLES).is_ok());
}

#[test]
fn registry_genesis_type_id_changes_when_since_changes() {
    let mut f = RegistryTestFixture::new();
    let seed_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input_since_zero = CellInput::new_builder()
        .previous_output(seed_out_point.clone())
        .since(0u64)
        .build();
    let input_since_nonzero = CellInput::new_builder()
        .previous_output(seed_out_point)
        .since(0x1234_5678_9abc_def0u64)
        .build();

    // Derive registry_id with since = 0, but spend input with non-zero since
    let registry_id_zero = derive_registry_id(&input_since_zero, 0);
    let registry_id_nonzero = derive_registry_id(&input_since_nonzero, 0);
    assert_ne!(registry_id_zero, registry_id_nonzero);

    let reg_args = registry_args(
        0x01,
        &registry_id_zero,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);

    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(input_since_nonzero)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id_zero, &[]))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_INVALID_ARGS);
}

#[test]
fn registry_genesis_type_id_changes_when_outpoint_changes() {
    let mut f = RegistryTestFixture::new();
    let seed_out_point_a = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::from(vec![1]),
    );
    let seed_out_point_b = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::from(vec![2]),
    );
    let input_a = CellInput::new_builder()
        .previous_output(seed_out_point_a)
        .build();
    let input_b = CellInput::new_builder()
        .previous_output(seed_out_point_b)
        .build();

    let registry_id = derive_registry_id(&input_a, 0);
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);

    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(input_b)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[]))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_INVALID_ARGS);
}

#[test]
fn registry_genesis_type_id_respects_global_output_index() {
    let mut f = RegistryTestFixture::new();
    let seed_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let first_input = CellInput::new_builder()
        .previous_output(seed_out_point)
        .build();

    let dummy_output = CellOutput::new_builder()
        .capacity(CAPACITY / 2)
        .lock(f.admin_lock_script.clone())
        .build();

    // Case 1: Registry cell at Output[1], but registry_id derived for index 0 -> ERROR_INVALID_ARGS
    let wrong_registry_id = derive_registry_id(&first_input, 0);
    let wrong_args = registry_args(
        0x01,
        &wrong_registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let wrong_reg_script = f.build_registry_script(wrong_args);
    let wrong_reg_output = CellOutput::new_builder()
        .capacity(CAPACITY / 2)
        .lock(f.admin_lock_script.clone())
        .type_(Some(wrong_reg_script).pack())
        .build();

    let bad_tx = TransactionBuilder::default()
        .input(first_input.clone())
        .output(dummy_output.clone())
        .output_data(Bytes::new())
        .output(wrong_reg_output)
        .output_data(registry_data(0x01, &wrong_registry_id, &[]))
        .build();
    let bad_tx = f.context.complete_tx(bad_tx);
    assert_script_error(f.context.verify_tx(&bad_tx, MAX_CYCLES), ERROR_INVALID_ARGS);

    // Case 2: Registry cell at Output[1], registry_id derived for index 1 -> Ok
    let right_registry_id = derive_registry_id(&first_input, 1);
    let right_args = registry_args(
        0x01,
        &right_registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let right_reg_script = f.build_registry_script(right_args);
    let right_reg_output = CellOutput::new_builder()
        .capacity(CAPACITY / 2)
        .lock(f.admin_lock_script.clone())
        .type_(Some(right_reg_script).pack())
        .build();

    let good_tx = TransactionBuilder::default()
        .input(first_input)
        .output(dummy_output)
        .output_data(Bytes::new())
        .output(right_reg_output)
        .output_data(registry_data(0x01, &right_registry_id, &[]))
        .build();
    let good_tx = f.context.complete_tx(good_tx);
    assert!(f.context.verify_tx(&good_tx, MAX_CYCLES).is_ok());
}

#[test]
fn registry_genesis_rejects_when_first_input_not_admin_locked() {
    let mut f = RegistryTestFixture::new();
    // Input[0] uses non-admin lock, Input[1] uses admin lock
    let non_admin_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.other_lock_script.clone())
            .build(),
        Bytes::from(vec![1]),
    );
    let admin_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::from(vec![2]),
    );

    let first_input = CellInput::new_builder()
        .previous_output(non_admin_out_point)
        .build();
    let second_input = CellInput::new_builder()
        .previous_output(admin_out_point)
        .build();

    let registry_id = derive_registry_id(&first_input, 0);
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);

    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(first_input)
        .input(second_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[]))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_UNAUTHORIZED);
}

#[test]
fn registry_genesis_rejects_invalid_initial_data() {
    let mut f = RegistryTestFixture::new();
    let seed_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let first_input = CellInput::new_builder()
        .previous_output(seed_out_point)
        .build();

    let registry_id = derive_registry_id(&first_input, 0);
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    // 1. Non-zero initial item count (1 item during genesis)
    let tx_non_zero_count = TransactionBuilder::default()
        .input(first_input.clone())
        .output(reg_output.clone())
        .output_data(registry_data(0x01, &registry_id, &[[1u8; 32]]))
        .build();
    let tx_non_zero_count = f.context.complete_tx(tx_non_zero_count);
    assert_script_error(
        f.context.verify_tx(&tx_non_zero_count, MAX_CYCLES),
        ERROR_INVALID_DATA,
    );

    // 2. Wrong schema version (0x02)
    let tx_bad_version = TransactionBuilder::default()
        .input(first_input.clone())
        .output(reg_output.clone())
        .output_data(registry_data(0x02, &registry_id, &[]))
        .build();
    let tx_bad_version = f.context.complete_tx(tx_bad_version);
    assert_script_error(
        f.context.verify_tx(&tx_bad_version, MAX_CYCLES),
        ERROR_INVALID_DATA,
    );

    // 3. Mismatched registry_id in cell data
    let tx_bad_id = TransactionBuilder::default()
        .input(first_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &[0xFF; 32], &[]))
        .build();
    let tx_bad_id = f.context.complete_tx(tx_bad_id);
    assert_script_error(
        f.context.verify_tx(&tx_bad_id, MAX_CYCLES),
        ERROR_INVALID_DATA,
    );
}

#[test]
fn registry_genesis_rejects_duplicate_registry_outputs() {
    let mut f = RegistryTestFixture::new();
    let seed_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let first_input = CellInput::new_builder()
        .previous_output(seed_out_point)
        .build();

    let registry_id = derive_registry_id(&first_input, 0);
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);

    let reg_output_1 = CellOutput::new_builder()
        .capacity(CAPACITY / 2)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script.clone()).pack())
        .build();
    let reg_output_2 = CellOutput::new_builder()
        .capacity(CAPACITY / 2)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(first_input)
        .output(reg_output_1)
        .output_data(registry_data(0x01, &registry_id, &[]))
        .output(reg_output_2)
        .output_data(registry_data(0x01, &registry_id, &[]))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_MULTIPLE_CELLS,
    );
}

#[test]
fn registry_rejects_burn_attempt() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x42u8; 32];
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();

    // Consume Registry Cell without producing a Registry output (1 -> 0 burn)
    let plain_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .build();

    let tx = TransactionBuilder::default()
        .input(reg_input)
        .output(plain_output)
        .output_data(Bytes::new())
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MISSING_CELL);
}

#[test]
fn registry_rejects_invalid_args_length_version_or_hash_type() {
    let mut f = RegistryTestFixture::new();
    let seed_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let first_input = CellInput::new_builder()
        .previous_output(seed_out_point)
        .build();
    let registry_id = derive_registry_id(&first_input, 0);

    // 1. Short args (97 bytes instead of 98 bytes)
    let short_args = Bytes::from(vec![0x01; 97]);
    let short_script = f.build_registry_script(short_args);
    let tx_short = TransactionBuilder::default()
        .input(first_input.clone())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(short_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[]))
        .build();
    let tx_short = f.context.complete_tx(tx_short);
    assert_script_error(
        f.context.verify_tx(&tx_short, MAX_CYCLES),
        ERROR_INVALID_ARGS,
    );

    // 2. Unsupported args version (0x02)
    let bad_ver_args = registry_args(
        0x02,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let bad_ver_script = f.build_registry_script(bad_ver_args);
    let tx_bad_ver = TransactionBuilder::default()
        .input(first_input.clone())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(bad_ver_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[]))
        .build();
    let tx_bad_ver = f.context.complete_tx(tx_bad_ver);
    assert_script_error(
        f.context.verify_tx(&tx_bad_ver, MAX_CYCLES),
        ERROR_INVALID_ARGS,
    );

    // 3. Invalid state_hash_type (0x09)
    let bad_ht_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        0x09,
    );
    let bad_ht_script = f.build_registry_script(bad_ht_args);
    let tx_bad_ht = TransactionBuilder::default()
        .input(first_input)
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(bad_ht_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[]))
        .build();
    let tx_bad_ht = f.context.complete_tx(tx_bad_ht);
    assert_script_error(
        f.context.verify_tx(&tx_bad_ht, MAX_CYCLES),
        ERROR_INVALID_ARGS,
    );
}

// ============================================================================
// Week 10 CP 10.2: Registry Update / Item Registration Tests (1 -> 1)
// ============================================================================

#[test]
fn registry_registration_allows_first_and_subsequent_items() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    // 1. First registration (0 -> 1 item)
    let reg_in_out_point_1 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_input_1 = CellInput::new_builder()
        .previous_output(reg_in_out_point_1)
        .build();
    let reg_output_1 = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script.clone()).pack())
        .build();

    let state_args_1 = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script_1 = f.build_canonical_state_script(state_args_1);
    let state_output_1 = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script_1).pack())
        .build();

    let tx_1 = TransactionBuilder::default()
        .input(reg_input_1)
        .output(reg_output_1)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(state_output_1)
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx_1 = f.context.complete_tx(tx_1);
    assert!(f.context.verify_tx(&tx_1, MAX_CYCLES).is_ok());

    // 2. Second registration (1 -> 2 items, preserving item_1 at index 0)
    let reg_in_out_point_2 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_1]),
    );
    let reg_input_2 = CellInput::new_builder()
        .previous_output(reg_in_out_point_2)
        .build();
    let reg_output_2 = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let state_args_2 = state_spec_args(0x01, &item_2, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script_2 = f.build_canonical_state_script(state_args_2);
    let state_output_2 = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script_2).pack())
        .build();

    let tx_2 = TransactionBuilder::default()
        .input(reg_input_2)
        .output(reg_output_2)
        .output_data(registry_data(0x01, &registry_id, &[item_1, item_2]))
        .output(state_output_2)
        .output_data(state_data(&item_2, AVAILABLE))
        .build();
    let tx_2 = f.context.complete_tx(tx_2);
    assert!(f.context.verify_tx(&tx_2, MAX_CYCLES).is_ok());
}

#[test]
fn registry_registration_rejects_duplicate_item_id() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_1]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let state_args = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_canonical_state_script(state_args);
    let state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(reg_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1, item_1]))
        .output(state_output)
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_DUPLICATE_ITEM);
}

#[test]
fn registry_registration_rejects_modified_existing_prefix() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];
    let item_3 = [0x03u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_1, item_2]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let state_args = state_spec_args(0x01, &item_3, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_canonical_state_script(state_args);
    let state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    // Reorder existing prefix [item_2, item_1] instead of [item_1, item_2]
    let tx = TransactionBuilder::default()
        .input(reg_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_2, item_1, item_3]))
        .output(state_output)
        .output_data(state_data(&item_3, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_INVALID_TRANSITION,
    );
}

#[test]
fn registry_registration_rejects_missing_state_output() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(reg_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MISSING_CELL);
}

#[test]
fn registry_registration_rejects_non_canonical_state_code_hash() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let state_args = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);

    // Deploy a second script with a different Type ID (different code_hash, same hash_type = Type)
    let rogue_type_id_script = random_type_id_script();
    let rogue_contract_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(1000 * 100_000_000u64)
            .lock(f.admin_lock_script.clone())
            .type_(Some(rogue_type_id_script).pack())
            .build(),
        ALWAYS_SUCCESS.clone(),
    );
    let wrong_code_hash_script = f
        .context
        .build_script_with_hash_type(&rogue_contract_out_point, ScriptHashType::Type, state_args)
        .expect("build script with non-canonical code_hash");

    let bad_state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(wrong_code_hash_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(reg_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(bad_state_output)
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MISSING_CELL);
}

#[test]
fn registry_registration_rejects_non_canonical_state_hash_type() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];

    // Configure Registry to expect canonical State script by Data1 (0x02) with data_hash(ALWAYS_SUCCESS)
    let state_data_hash = byte32_to_array(&CellOutput::calc_data_hash(&ALWAYS_SUCCESS));
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &state_data_hash,
        ScriptHashType::Data1.into(),
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let state_args = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);

    // 1. Construct the expected canonical State Script using ScriptHashType::Data1
    let canonical_data1_state_script = f
        .context
        .build_script_with_hash_type(
            &f.state_contract_out_point,
            ScriptHashType::Data1,
            state_args.clone(),
        )
        .expect("build canonical State script with Data1");

    // 2. Construct the invalid State Script using ScriptHashType::Data2
    let invalid_data2_state_script = f
        .context
        .build_script_with_hash_type(
            &f.state_contract_out_point,
            ScriptHashType::Data2,
            state_args,
        )
        .expect("build invalid State script with Data2");

    // 3. Explicitly assert that both scripts have the exact same code_hash (and args)
    assert_eq!(
        canonical_data1_state_script.code_hash(),
        invalid_data2_state_script.code_hash()
    );
    assert_eq!(
        canonical_data1_state_script.code_hash().as_slice(),
        &state_data_hash[..]
    );
    assert_eq!(
        canonical_data1_state_script.args(),
        invalid_data2_state_script.args()
    );

    // 4. Explicitly assert that their hash_type values are different (Data1 = 0x02 vs Data2 = 0x04)
    assert_ne!(
        canonical_data1_state_script.hash_type().as_slice(),
        invalid_data2_state_script.hash_type().as_slice()
    );
    assert_eq!(
        canonical_data1_state_script.hash_type(),
        ScriptHashType::Data1.into()
    );
    assert_eq!(
        invalid_data2_state_script.hash_type(),
        ScriptHashType::Data2.into()
    );

    // Control check: the identical transaction succeeds when using canonical_data1_state_script
    let valid_state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(canonical_data1_state_script).pack())
        .build();
    let valid_tx = TransactionBuilder::default()
        .input(reg_input.clone())
        .output(reg_output.clone())
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(valid_state_output)
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let valid_tx = f.context.complete_tx(valid_tx);
    assert!(f.context.verify_tx(&valid_tx, MAX_CYCLES).is_ok());

    // 5. Verify that Registry rejects the transaction when only State hash_type differs (Data2 vs Data1)
    let bad_state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(invalid_data2_state_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(reg_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(bad_state_output)
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MISSING_CELL);
}

#[test]
fn registry_registration_rejects_multiple_canonical_state_outputs() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    let state_args_1 = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);
    let state_args_2 = state_spec_args(0x01, &item_2, &reg_type_hash, &f.admin_lock_hash);
    let state_script_1 = f.build_canonical_state_script(state_args_1);
    let state_script_2 = f.build_canonical_state_script(state_args_2);

    let state_output_1 = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_script_1).pack())
        .build();
    let state_output_2 = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_script_2).pack())
        .build();

    let tx = TransactionBuilder::default()
        .input(reg_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(state_output_1)
        .output_data(state_data(&item_1, AVAILABLE))
        .output(state_output_2)
        .output_data(state_data(&item_2, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_MULTIPLE_CELLS,
    );
}

#[test]
fn registry_registration_rejects_invalid_state_args_data_or_status() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();

    // 1. State args item_id mismatch -> ERROR_INVALID_ARGS
    let bad_item_args = state_spec_args(0x01, &[0x99; 32], &reg_type_hash, &f.admin_lock_hash);
    let bad_item_script = f.build_canonical_state_script(bad_item_args);
    let tx_bad_item_args = TransactionBuilder::default()
        .input(reg_input.clone())
        .output(reg_output.clone())
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(bad_item_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx_bad_item_args = f.context.complete_tx(tx_bad_item_args);
    assert_script_error(
        f.context.verify_tx(&tx_bad_item_args, MAX_CYCLES),
        ERROR_INVALID_ARGS,
    );

    // 2. State args trusted_registry_type_hash mismatch -> ERROR_UNTRUSTED_REGISTRY
    let bad_reg_hash_args = state_spec_args(0x01, &item_1, &[0xFF; 32], &f.admin_lock_hash);
    let bad_reg_hash_script = f.build_canonical_state_script(bad_reg_hash_args);
    let tx_bad_reg_hash = TransactionBuilder::default()
        .input(reg_input.clone())
        .output(reg_output.clone())
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(bad_reg_hash_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx_bad_reg_hash = f.context.complete_tx(tx_bad_reg_hash);
    assert_script_error(
        f.context.verify_tx(&tx_bad_reg_hash, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );

    // 3. State args admin_lock_hash mismatch -> ERROR_UNAUTHORIZED
    let bad_admin_args = state_spec_args(0x01, &item_1, &reg_type_hash, &[0xEE; 32]);
    let bad_admin_script = f.build_canonical_state_script(bad_admin_args);
    let tx_bad_admin = TransactionBuilder::default()
        .input(reg_input.clone())
        .output(reg_output.clone())
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(bad_admin_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx_bad_admin = f.context.complete_tx(tx_bad_admin);
    assert_script_error(
        f.context.verify_tx(&tx_bad_admin, MAX_CYCLES),
        ERROR_UNAUTHORIZED,
    );

    // 4. Valid State args, but State data has wrong item_id -> ERROR_INVALID_DATA
    let good_state_args = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);
    let good_state_script = f.build_canonical_state_script(good_state_args);
    let good_state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(good_state_script).pack())
        .build();

    let tx_bad_state_data = TransactionBuilder::default()
        .input(reg_input.clone())
        .output(reg_output.clone())
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(good_state_output.clone())
        .output_data(state_data(&[0x77; 32], AVAILABLE))
        .build();
    let tx_bad_state_data = f.context.complete_tx(tx_bad_state_data);
    assert_script_error(
        f.context.verify_tx(&tx_bad_state_data, MAX_CYCLES),
        ERROR_INVALID_DATA,
    );

    // 5. Initial State status is BORROWED instead of AVAILABLE -> ERROR_INVALID_TRANSITION
    let tx_bad_initial_status = TransactionBuilder::default()
        .input(reg_input)
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(good_state_output)
        .output_data(state_data(&item_1, BORROWED))
        .build();
    let tx_bad_initial_status = f.context.complete_tx(tx_bad_initial_status);
    assert_script_error(
        f.context.verify_tx(&tx_bad_initial_status, MAX_CYCLES),
        ERROR_INVALID_TRANSITION,
    );
}

#[test]
fn registry_registration_rejects_capacity_violations() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_input = CellInput::new_builder()
        .previous_output(reg_in_out_point)
        .build();

    let state_args = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_canonical_state_script(state_args);

    // 1. Registry output capacity < Registry input capacity -> ERROR_CAPACITY
    let reduced_reg_output = CellOutput::new_builder()
        .capacity(CAPACITY - 1)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script.clone()).pack())
        .build();
    let good_state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script.clone()).pack())
        .build();

    let tx_reduced_reg_cap = TransactionBuilder::default()
        .input(reg_input.clone())
        .output(reduced_reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(good_state_output)
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx_reduced_reg_cap = f.context.complete_tx(tx_reduced_reg_cap);
    assert_script_error(
        f.context.verify_tx(&tx_reduced_reg_cap, MAX_CYCLES),
        ERROR_CAPACITY,
    );

    // 2. State output capacity < State occupied capacity (225 CKB = 22_500_000_000 shannons) -> ERROR_CAPACITY
    let good_reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();
    let underfunded_state_output = CellOutput::new_builder()
        .capacity(100 * 100_000_000u64) // 100 CKB < 225 CKB occupied capacity
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    let tx_underfunded_state = TransactionBuilder::default()
        .input(reg_input)
        .output(good_reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(underfunded_state_output)
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx_underfunded_state = f.context.complete_tx(tx_underfunded_state);
    assert_script_error(
        f.context.verify_tx(&tx_underfunded_state, MAX_CYCLES),
        ERROR_CAPACITY,
    );
}

#[test]
fn registry_registration_enforces_max_100_items_limit() {
    let mut f = RegistryTestFixture::new();
    let registry_id = [0x33u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let mut items_99: Vec<[u8; 32]> = (0u8..99u8)
        .map(|i| {
            let mut id = [0u8; 32];
            id[0] = i + 1;
            id
        })
        .collect();

    let item_100 = [100u8; 32];
    let item_101 = [101u8; 32];

    // 1. Registering the 100th item (99 -> 100) succeeds
    let reg_in_99 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &items_99),
    );
    items_99.push(item_100);
    let items_100 = items_99;

    let state_args_100 = state_spec_args(0x01, &item_100, &reg_type_hash, &f.admin_lock_hash);
    let state_script_100 = f.build_canonical_state_script(state_args_100);

    let tx_100 = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_99).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &items_100))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_100).pack())
                .build(),
        )
        .output_data(state_data(&item_100, AVAILABLE))
        .build();
    let tx_100 = f.context.complete_tx(tx_100);
    assert!(f.context.verify_tx(&tx_100, MAX_CYCLES).is_ok());

    // 2. Registering the 101st item (100 -> 101) fails with ERROR_CAPACITY_LIMIT (9)
    let reg_in_100 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &items_100),
    );
    let mut items_101 = items_100;
    items_101.push(item_101);

    let state_args_101 = state_spec_args(0x01, &item_101, &reg_type_hash, &f.admin_lock_hash);
    let state_script_101 = f.build_canonical_state_script(state_args_101);

    let tx_101 = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_100).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &items_101))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_101).pack())
                .build(),
        )
        .output_data(state_data(&item_101, AVAILABLE))
        .build();
    let tx_101 = f.context.complete_tx(tx_101);
    assert_script_error(
        f.context.verify_tx(&tx_101, MAX_CYCLES),
        ERROR_CAPACITY_LIMIT,
    );
}
