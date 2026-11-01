use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
    ckb_chain_spec::SystemCell,
    ckb_crypto::secp::Privkey,
    ckb_error::Error,
    ckb_hash::{blake2b_256, new_blake2b},
    ckb_types::{
        H256,
        bytes::Bytes,
        core::{DepType, ScriptHashType, TransactionBuilder, TransactionView},
        packed::{self, *},
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
            .build_script_with_hash_type(&self.state_contract_out_point, ScriptHashType::Type, args)
            .expect("build canonical state type script")
    }
}

struct StateTestFixture {
    context: Context,
    state_out_point: OutPoint,
    state_code_hash: [u8; 32],
    state_hash_type: u8,
    admin_lock_script: Script,
    admin_lock_hash: [u8; 32],
    other_lock_script: Script,
    registry_contract_out_point: OutPoint,
}

impl StateTestFixture {
    fn new() -> Self {
        let mut context = Context::default();
        let state_out_point = context.deploy_cell_by_name("state");
        let lock_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

        let admin_lock_script = context
            .build_script(&lock_out_point, Bytes::from(vec![0xAA; 20]))
            .expect("build admin lock script");
        let admin_lock_hash = byte32_to_array(&admin_lock_script.calc_script_hash());

        let other_lock_script = context
            .build_script(&lock_out_point, Bytes::from(vec![0xBB; 20]))
            .expect("build other lock script");

        let dummy_state_script = context
            .build_script(&state_out_point, Bytes::new())
            .expect("build dummy state script");
        let state_code_hash = byte32_to_array(&dummy_state_script.code_hash());
        let state_hash_type = dummy_state_script.hash_type().as_slice()[0];

        // Deploy a mock Registry code cell backed by ALWAYS_SUCCESS with a unique Type ID
        // so State script verification can be tested in isolation even when a Registry cell
        // is placed in Inputs or Outputs.
        let reg_type_id_script = random_type_id_script();
        let registry_contract_out_point = context.create_cell(
            CellOutput::new_builder()
                .capacity(1000 * 100_000_000u64)
                .lock(admin_lock_script.clone())
                .type_(Some(reg_type_id_script).pack())
                .build(),
            ALWAYS_SUCCESS.clone(),
        );

        Self {
            context,
            state_out_point,
            state_code_hash,
            state_hash_type,
            admin_lock_script,
            admin_lock_hash,
            other_lock_script,
            registry_contract_out_point,
        }
    }

    fn build_state_script(&mut self, args: Bytes) -> Script {
        self.context
            .build_script(&self.state_out_point, args)
            .expect("build state type script")
    }

    fn build_registry_script(&mut self, args: Bytes) -> Script {
        self.context
            .build_script_with_hash_type(
                &self.registry_contract_out_point,
                ScriptHashType::Type,
                args,
            )
            .expect("build registry type script")
    }

    fn create_registry_cell_dep(
        &mut self,
        lock_script: Script,
        reg_type_script: Option<Script>,
        data: Bytes,
    ) -> CellDep {
        let out_point = self.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(lock_script)
                .type_(reg_type_script.pack())
                .build(),
            data,
        );
        CellDep::new_builder()
            .out_point(out_point)
            .dep_type(DepType::Code)
            .build()
    }
}

fn run_transition(
    old_status: u8,
    new_status: u8,
    output_item_id: [u8; 32],
    output_data_override: Option<Bytes>,
) -> bool {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [7u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let type_script = f.build_state_script(s_args);

    let input_data = state_data(&item_id, old_status);
    let output_data =
        output_data_override.unwrap_or_else(|| state_data(&output_item_id, new_status));

    let input_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(type_script.clone()).pack())
            .build(),
        input_data,
    );

    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();

    let output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(input)
        .output(output)
        .output_data(output_data)
        .build();

    let tx = f.context.complete_tx(tx);
    f.context.verify_tx(&tx, MAX_CYCLES).is_ok()
}

fn run_with_duplicate_state_outputs() -> bool {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [7u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let type_script = f.build_state_script(s_args);

    let input_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
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
        .lock(f.admin_lock_script.clone())
        .type_(Some(type_script.clone()).pack())
        .build();

    let output_2 = CellOutput::new_builder()
        .capacity(output_capacity)
        .lock(f.admin_lock_script.clone())
        .type_(Some(type_script).pack())
        .build();

    let tx = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(input)
        .output(output_1)
        .output_data(state_data(&item_id, BORROWED))
        .output(output_2)
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    f.context.verify_tx(&tx, MAX_CYCLES).is_ok()
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
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MULTIPLE_CELLS);
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
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MULTIPLE_CELLS);
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

// ============================================================================
// Week 10 CP 10.3: State Script Hardening Tests (1 GroupInput -> 1 GroupOutput)
// ============================================================================

#[test]
fn state_lifecycle_allows_full_cycle_with_multi_item_registry_celldep() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_a = [0x10u8; 32];
    let item_b = [0x20u8; 32];
    let item_c = [0x30u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_a, item_b, item_c]),
    );

    let s_args = state_spec_args(0x01, &item_b, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    for (old_status, new_status) in [
        (AVAILABLE, BORROWED),
        (BORROWED, RETURNED),
        (RETURNED, AVAILABLE),
    ] {
        let in_out_point = f.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
            state_data(&item_b, old_status),
        );

        let tx = TransactionBuilder::default()
            .cell_dep(reg_cell_dep.clone())
            .input(
                CellInput::new_builder()
                    .previous_output(in_out_point)
                    .build(),
            )
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(state_type_script.clone()).pack())
                    .build(),
            )
            .output_data(state_data(&item_b, new_status))
            .build();

        let tx = f.context.complete_tx(tx);
        assert!(f.context.verify_tx(&tx, MAX_CYCLES).is_ok());
    }
}

#[test]
fn state_rejects_missing_registry_celldep() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    // No Registry CellDep included in transaction
    let tx = TransactionBuilder::default()
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_rejects_wrong_registry_type_script_hash() {
    let mut f = StateTestFixture::new();
    let trusted_registry_id = [0x55u8; 32];
    let wrong_registry_id = [0x66u8; 32];
    let item_id = [0x11u8; 32];

    let trusted_reg_args = registry_args(
        0x01,
        &trusted_registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let trusted_reg_script = f.build_registry_script(trusted_reg_args);
    let trusted_reg_type_hash = byte32_to_array(&trusted_reg_script.calc_script_hash());

    let wrong_reg_args = registry_args(
        0x01,
        &wrong_registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let wrong_reg_script = f.build_registry_script(wrong_reg_args);
    let wrong_reg_type_hash = byte32_to_array(&wrong_reg_script.calc_script_hash());
    assert_ne!(trusted_reg_type_hash, wrong_reg_type_hash);

    // 1. CellDep has a different Registry Type Script hash
    let wrong_reg_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(wrong_reg_script),
        registry_data(0x01, &wrong_registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &trusted_reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let state_input = CellInput::new_builder()
        .previous_output(in_out_point)
        .build();
    let state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    let tx_wrong_hash = TransactionBuilder::default()
        .cell_dep(wrong_reg_dep)
        .input(state_input.clone())
        .output(state_output.clone())
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_wrong_hash = f.context.complete_tx(tx_wrong_hash);
    assert_script_error(
        f.context.verify_tx(&tx_wrong_hash, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );

    // 2. CellDep has valid Registry data bytes but no Type Script (type_ = None)
    let typeless_reg_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        None,
        registry_data(0x01, &trusted_registry_id, &[item_id]),
    );
    let tx_typeless = TransactionBuilder::default()
        .cell_dep(typeless_reg_dep)
        .input(state_input)
        .output(state_output)
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_typeless = f.context.complete_tx(tx_typeless);
    assert_script_error(
        f.context.verify_tx(&tx_typeless, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_rejects_duplicate_matching_registry_celldeps() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_dep_1 = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script.clone()),
        registry_data(0x01, &registry_id, &[item_id]),
    );
    let reg_dep_2 = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    let tx = TransactionBuilder::default()
        .cell_dep(reg_dep_1)
        .cell_dep(reg_dep_2)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_rejects_incorrect_registry_celldep_lock_hash() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    // Registry CellDep cell is locked by other_lock_script instead of admin_lock_script
    let bad_lock_reg_dep = f.create_registry_cell_dep(
        f.other_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    let tx = TransactionBuilder::default()
        .cell_dep(bad_lock_reg_dep)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_UNAUTHORIZED);
}

#[test]
fn state_rejects_incorrect_registry_args_length() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    for bad_reg_args in [
        Bytes::from(vec![0x01; 97]), // 97-byte short args
        Bytes::from(vec![0x01; 99]), // 99-byte long args
    ] {
        let bad_reg_script = f.build_registry_script(bad_reg_args);
        let bad_reg_type_hash = byte32_to_array(&bad_reg_script.calc_script_hash());
        let bad_reg_dep = f.create_registry_cell_dep(
            f.admin_lock_script.clone(),
            Some(bad_reg_script),
            registry_data(0x01, &registry_id, &[item_id]),
        );

        let s_args = state_spec_args(0x01, &item_id, &bad_reg_type_hash, &f.admin_lock_hash);
        let state_script = f.build_state_script(s_args);
        let in_out_point = f.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script.clone()).pack())
                .build(),
            state_data(&item_id, AVAILABLE),
        );
        let tx = TransactionBuilder::default()
            .cell_dep(bad_reg_dep)
            .input(
                CellInput::new_builder()
                    .previous_output(in_out_point)
                    .build(),
            )
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(state_script).pack())
                    .build(),
            )
            .output_data(state_data(&item_id, BORROWED))
            .build();
        let tx = f.context.complete_tx(tx);
        assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_INVALID_ARGS);
    }
}

#[test]
fn state_rejects_incorrect_registry_args_schema_version() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let bad_ver_reg_args = registry_args(
        0x02,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let bad_ver_reg_script = f.build_registry_script(bad_ver_reg_args);
    let bad_ver_reg_type_hash = byte32_to_array(&bad_ver_reg_script.calc_script_hash());
    let bad_ver_reg_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(bad_ver_reg_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &bad_ver_reg_type_hash, &f.admin_lock_hash);
    let state_script = f.build_state_script(s_args);
    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_bad_ver = TransactionBuilder::default()
        .cell_dep(bad_ver_reg_dep)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_bad_ver = f.context.complete_tx(tx_bad_ver);
    assert_script_error(
        f.context.verify_tx(&tx_bad_ver, MAX_CYCLES),
        ERROR_INVALID_ARGS,
    );
}

#[test]
fn state_rejects_incorrect_registry_args_admin_lock_hash() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];
    let wrong_admin_lock_hash = [0xEEu8; 32];

    // Registry Type Script args[33..65] has wrong_admin_lock_hash != State args admin_lock_hash
    let reg_args = registry_args(
        0x01,
        &registry_id,
        &wrong_admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    let tx = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_UNAUTHORIZED);
}

#[test]
fn state_rejects_incorrect_state_code_hash_or_hash_type_in_registry_args() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    // 1. Registry Type Script args[65..97] has wrong state_code_hash
    let bad_code_reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &[0x99u8; 32],
        f.state_hash_type,
    );
    let bad_code_reg_script = f.build_registry_script(bad_code_reg_args);
    let bad_code_reg_hash = byte32_to_array(&bad_code_reg_script.calc_script_hash());
    let bad_code_reg_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(bad_code_reg_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args_1 = state_spec_args(0x01, &item_id, &bad_code_reg_hash, &f.admin_lock_hash);
    let state_script_1 = f.build_state_script(s_args_1);
    let in_1 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_script_1.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_bad_code = TransactionBuilder::default()
        .cell_dep(bad_code_reg_dep)
        .input(CellInput::new_builder().previous_output(in_1).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_1).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_bad_code = f.context.complete_tx(tx_bad_code);
    assert_script_error(
        f.context.verify_tx(&tx_bad_code, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );

    // 2. Registry Type Script args[97] has wrong state_hash_type
    let wrong_hash_type = if f.state_hash_type == 0x00 {
        0x01
    } else {
        0x00
    };
    let bad_ht_reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        wrong_hash_type,
    );
    let bad_ht_reg_script = f.build_registry_script(bad_ht_reg_args);
    let bad_ht_reg_hash = byte32_to_array(&bad_ht_reg_script.calc_script_hash());
    let bad_ht_reg_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(bad_ht_reg_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args_2 = state_spec_args(0x01, &item_id, &bad_ht_reg_hash, &f.admin_lock_hash);
    let state_script_2 = f.build_state_script(s_args_2);
    let in_2 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_script_2.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_bad_ht = TransactionBuilder::default()
        .cell_dep(bad_ht_reg_dep)
        .input(CellInput::new_builder().previous_output(in_2).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_2).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_bad_ht = f.context.complete_tx(tx_bad_ht);
    assert_script_error(
        f.context.verify_tx(&tx_bad_ht, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_rejects_incorrect_registry_data_schema_version() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    let bad_schema_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x02, &registry_id, &[item_id]),
    );
    let tx_bad_schema = TransactionBuilder::default()
        .cell_dep(bad_schema_dep)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_bad_schema = f.context.complete_tx(tx_bad_schema);
    assert_script_error(
        f.context.verify_tx(&tx_bad_schema, MAX_CYCLES),
        ERROR_INVALID_DATA,
    );
}

#[test]
fn state_rejects_incorrect_registry_data_length() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let state_input = CellInput::new_builder()
        .previous_output(in_out_point)
        .build();
    let state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    let mut trailing_byte_data = registry_data(0x01, &registry_id, &[item_id]).to_vec();
    trailing_byte_data.push(0xAA);

    for bad_data in [
        Bytes::from(vec![0x01; 36]),     // < 37-byte header
        Bytes::from(trailing_byte_data), // 70 bytes instead of 69 bytes
    ] {
        let bad_len_dep = f.create_registry_cell_dep(
            f.admin_lock_script.clone(),
            Some(reg_type_script.clone()),
            bad_data,
        );
        let tx = TransactionBuilder::default()
            .cell_dep(bad_len_dep)
            .input(state_input.clone())
            .output(state_output.clone())
            .output_data(state_data(&item_id, BORROWED))
            .build();
        let tx = f.context.complete_tx(tx);
        assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_INVALID_DATA);
    }
}

#[test]
fn state_rejects_incorrect_registry_data_registry_id() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    // Registry Data has [0xFF; 32] as registry_id instead of registry_id ([0x55; 32])
    let bad_id_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &[0xFFu8; 32], &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    let tx = TransactionBuilder::default()
        .cell_dep(bad_id_dep)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_INVALID_DATA);
}

#[test]
fn state_rejects_inconsistent_registry_item_count_and_data_length() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let state_input = CellInput::new_builder()
        .previous_output(in_out_point)
        .build();
    let state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    // 1. Header claims item_count = 2, but payload only has 1 item (69 bytes != 101 bytes)
    let mut count_too_high_data = registry_data(0x01, &registry_id, &[item_id]).to_vec();
    count_too_high_data[33..37].copy_from_slice(&2u32.to_le_bytes());

    // 2. Header claims item_count = 0, but payload has 1 item (69 bytes != 37 bytes)
    let mut count_too_low_data = registry_data(0x01, &registry_id, &[item_id]).to_vec();
    count_too_low_data[33..37].copy_from_slice(&0u32.to_le_bytes());

    for bad_data in [
        Bytes::from(count_too_high_data),
        Bytes::from(count_too_low_data),
    ] {
        let bad_dep = f.create_registry_cell_dep(
            f.admin_lock_script.clone(),
            Some(reg_type_script.clone()),
            bad_data,
        );
        let tx = TransactionBuilder::default()
            .cell_dep(bad_dep)
            .input(state_input.clone())
            .output(state_output.clone())
            .output_data(state_data(&item_id, BORROWED))
            .build();
        let tx = f.context.complete_tx(tx);
        assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_INVALID_DATA);
    }
}

#[test]
fn state_rejects_registry_item_count_exceeding_capacity_limit() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let state_input = CellInput::new_builder()
        .previous_output(in_out_point)
        .build();
    let state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    let mut items_101: Vec<[u8; 32]> = (0u8..100u8)
        .map(|i| {
            let mut id = [0u8; 32];
            id[0] = i + 1;
            id
        })
        .collect();
    items_101.push(item_id);
    let over_limit_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &items_101),
    );
    let tx_over_limit = TransactionBuilder::default()
        .cell_dep(over_limit_dep)
        .input(state_input)
        .output(state_output)
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_over_limit = f.context.complete_tx(tx_over_limit);
    assert_script_error(
        f.context.verify_tx(&tx_over_limit, MAX_CYCLES),
        ERROR_CAPACITY_LIMIT,
    );
}

#[test]
fn state_rejects_duplicate_item_ids_in_registry_data() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let dup_reg_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id, item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );

    let tx = TransactionBuilder::default()
        .cell_dep(dup_reg_dep)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_DUPLICATE_ITEM);
}

#[test]
fn state_rejects_unregistered_target_item_id() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let registered_1 = [0x01u8; 32];
    let registered_2 = [0x02u8; 32];
    let unregistered_target = [0x99u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[registered_1, registered_2]),
    );

    let s_args = state_spec_args(
        0x01,
        &unregistered_target,
        &reg_type_hash,
        &f.admin_lock_hash,
    );
    let state_type_script = f.build_state_script(s_args);

    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&unregistered_target, AVAILABLE),
    );

    let tx = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&unregistered_target, BORROWED))
        .build();

    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_rejects_registry_present_in_inputs_or_outputs() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script.clone()),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let state_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let state_input = CellInput::new_builder()
        .previous_output(state_in_out_point)
        .build();
    let state_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script).pack())
        .build();

    // 1. Registry cell present in transaction Inputs
    let reg_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_id]),
    );
    let tx_reg_in_input = TransactionBuilder::default()
        .cell_dep(reg_cell_dep.clone())
        .input(state_input.clone())
        .input(
            CellInput::new_builder()
                .previous_output(reg_in_out_point)
                .build(),
        )
        .output(state_output.clone())
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_reg_in_input = f.context.complete_tx(tx_reg_in_input);
    assert_script_error(
        f.context.verify_tx(&tx_reg_in_input, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );

    // 2. Registry cell present in transaction Outputs
    let reg_output = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();
    let tx_reg_in_output = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(state_input)
        .output(state_output)
        .output_data(state_data(&item_id, BORROWED))
        .output(reg_output)
        .output_data(registry_data(0x01, &registry_id, &[item_id]))
        .build();
    let tx_reg_in_output = f.context.complete_tx(tx_reg_in_output);
    assert_script_error(
        f.context.verify_tx(&tx_reg_in_output, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_rejects_invalid_state_args_length_or_version() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let invalid_args_cases = [
        Bytes::from(item_id.to_vec()), // 32B legacy args
        Bytes::from(vec![0x01; 96]),   // 96B short args
        Bytes::from(vec![0x01; 98]),   // 98B long args
        state_spec_args(0x02, &item_id, &reg_type_hash, &f.admin_lock_hash), // 97B wrong version 0x02
    ];

    for bad_args in invalid_args_cases {
        let bad_state_script = f.build_state_script(bad_args);
        let in_out_point = f.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(bad_state_script.clone()).pack())
                .build(),
            state_data(&item_id, AVAILABLE),
        );
        let tx = TransactionBuilder::default()
            .cell_dep(reg_cell_dep.clone())
            .input(
                CellInput::new_builder()
                    .previous_output(in_out_point)
                    .build(),
            )
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(bad_state_script).pack())
                    .build(),
            )
            .output_data(state_data(&item_id, BORROWED))
            .build();
        let tx = f.context.complete_tx(tx);
        assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_INVALID_ARGS);
    }
}

#[test]
fn state_rejects_invalid_state_input_or_output_lock_hash() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    // 1. State input lock != admin_lock_hash
    let bad_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.other_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_bad_in_lock = TransactionBuilder::default()
        .cell_dep(reg_cell_dep.clone())
        .input(
            CellInput::new_builder()
                .previous_output(bad_in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_bad_in_lock = f.context.complete_tx(tx_bad_in_lock);
    assert_script_error(
        f.context.verify_tx(&tx_bad_in_lock, MAX_CYCLES),
        ERROR_UNAUTHORIZED,
    );

    // 2. State output lock != admin_lock_hash
    let good_in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_bad_out_lock = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(
            CellInput::new_builder()
                .previous_output(good_in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.other_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_bad_out_lock = f.context.complete_tx(tx_bad_out_lock);
    assert_script_error(
        f.context.verify_tx(&tx_bad_out_lock, MAX_CYCLES),
        ERROR_UNAUTHORIZED,
    );
}

#[test]
fn state_rejects_remaining_invalid_lifecycle_transitions_and_cardinality() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    // 1. Remaining invalid transitions: BORROWED->AVAILABLE, BORROWED->BORROWED, RETURNED->BORROWED, RETURNED->RETURNED
    for (old_status, new_status) in [
        (BORROWED, AVAILABLE),
        (BORROWED, BORROWED),
        (RETURNED, BORROWED),
        (RETURNED, RETURNED),
    ] {
        let in_out_point = f.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
            state_data(&item_id, old_status),
        );
        let tx = TransactionBuilder::default()
            .cell_dep(reg_cell_dep.clone())
            .input(
                CellInput::new_builder()
                    .previous_output(in_out_point)
                    .build(),
            )
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(state_type_script.clone()).pack())
                    .build(),
            )
            .output_data(state_data(&item_id, new_status))
            .build();
        let tx = f.context.complete_tx(tx);
        assert_script_error(
            f.context.verify_tx(&tx, MAX_CYCLES),
            ERROR_INVALID_TRANSITION,
        );
    }

    // 2. Unlinked State creation (0 GroupInput -> 1 GroupOutput without Registry 1->1 update) -> ERROR_UNTRUSTED_REGISTRY (10)
    let plain_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let tx_missing_in = TransactionBuilder::default()
        .cell_dep(reg_cell_dep.clone())
        .input(CellInput::new_builder().previous_output(plain_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
        )
        .output_data(state_data(&item_id, AVAILABLE))
        .build();
    let tx_missing_in = f.context.complete_tx(tx_missing_in);
    assert_script_error(
        f.context.verify_tx(&tx_missing_in, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );

    // 3. State burn / missing State GroupOutput (1 GroupInput -> 0 GroupOutput) -> ERROR_MISSING_CELL (4)
    let burn_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_burn = TransactionBuilder::default()
        .cell_dep(reg_cell_dep.clone())
        .input(CellInput::new_builder().previous_output(burn_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .build(),
        )
        .output_data(Bytes::new())
        .build();
    let tx_burn = f.context.complete_tx(tx_burn);
    assert_script_error(
        f.context.verify_tx(&tx_burn, MAX_CYCLES),
        ERROR_MISSING_CELL,
    );

    // 4. Multiple State GroupInputs (2 GroupInput -> 1 GroupOutput) -> ERROR_MULTIPLE_CELLS (5)
    let in_1 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let in_2 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_multi_in = TransactionBuilder::default()
        .cell_dep(reg_cell_dep.clone())
        .input(CellInput::new_builder().previous_output(in_1).build())
        .input(CellInput::new_builder().previous_output(in_2).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_multi_in = f.context.complete_tx(tx_multi_in);
    assert_script_error(
        f.context.verify_tx(&tx_multi_in, MAX_CYCLES),
        ERROR_MULTIPLE_CELLS,
    );

    // 5. Multiple State GroupOutputs (1 GroupInput -> 2 GroupOutput) -> ERROR_MULTIPLE_CELLS (5)
    let split_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 2)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_multi_out = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(CellInput::new_builder().previous_output(split_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_multi_out = f.context.complete_tx(tx_multi_out);
    assert_script_error(
        f.context.verify_tx(&tx_multi_out, MAX_CYCLES),
        ERROR_MULTIPLE_CELLS,
    );
}

#[test]
fn state_rejects_capacity_regression_or_insufficient_occupied_capacity() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_id = [0x11u8; 32];

    let reg_args = registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    );
    let reg_type_script = f.build_registry_script(reg_args);
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());

    let reg_cell_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_id]),
    );

    let s_args = state_spec_args(0x01, &item_id, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    // 1. Output capacity < Input capacity (CAPACITY - 1 < CAPACITY) -> ERROR_CAPACITY (7)
    let in_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_reduced_cap = TransactionBuilder::default()
        .cell_dep(reg_cell_dep.clone())
        .input(
            CellInput::new_builder()
                .previous_output(in_out_point)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY - 1)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_reduced_cap = f.context.complete_tx(tx_reduced_cap);
    assert_script_error(
        f.context.verify_tx(&tx_reduced_cap, MAX_CYCLES),
        ERROR_CAPACITY,
    );

    // 2. Output capacity < Occupied capacity (100 CKB < 225 CKB) -> ERROR_CAPACITY (7)
    let underfunded_cap = 100 * 100_000_000u64;
    let underfunded_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(underfunded_cap)
            .lock(f.admin_lock_script.clone())
            .type_(Some(state_type_script.clone()).pack())
            .build(),
        state_data(&item_id, AVAILABLE),
    );
    let tx_underfunded = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(
            CellInput::new_builder()
                .previous_output(underfunded_in)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(underfunded_cap)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_id, BORROWED))
        .build();
    let tx_underfunded = f.context.complete_tx(tx_underfunded);
    assert_script_error(
        f.context.verify_tx(&tx_underfunded, MAX_CYCLES),
        ERROR_CAPACITY,
    );
}

// ============================================================================
// Week 10 CP 10.4: Atomic Registry + State Registration Infrastructure & Tests
// ============================================================================

fn blake160(data: &[u8]) -> [u8; 20] {
    let mut buf = [0u8; 20];
    let hash = blake2b_256(data);
    buf.copy_from_slice(&hash[..20]);
    buf
}

fn load_bundled_system_cell(bundled_path: &str) -> Bytes {
    let spec_json = format!(
        r#"{{"create_type_id":false,"capacity":null,"file":{{"bundled":"{bundled_path}"}}}}"#
    );
    let cell: SystemCell =
        serde_json::from_str(&spec_json).expect("deserialize bundled system cell");
    Bytes::from(
        cell.file
            .get()
            .expect("load bundled system cell bytes")
            .into_owned(),
    )
}

fn sign_tx(tx: TransactionView, key: &Privkey) -> TransactionView {
    const SIGNATURE_SIZE: usize = 65;

    let witnesses_len = tx.witnesses().len();
    let tx_hash = tx.hash();
    let mut signed_witnesses: Vec<packed::Bytes> = Vec::new();
    let mut blake2b = new_blake2b();
    let mut message = [0u8; 32];
    blake2b.update(&tx_hash.raw_data());

    let witness = WitnessArgs::default();
    let zero_lock: Bytes = Bytes::from(vec![0u8; SIGNATURE_SIZE]);
    let witness_for_digest = witness
        .clone()
        .as_builder()
        .lock(Some(zero_lock).pack())
        .build();
    let witness_len = witness_for_digest.as_bytes().len() as u64;
    blake2b.update(&witness_len.to_le_bytes());
    blake2b.update(&witness_for_digest.as_bytes());

    for n in 1..witnesses_len {
        let w = tx.witnesses().get(n).unwrap();
        let w_len = w.raw_data().len() as u64;
        blake2b.update(&w_len.to_le_bytes());
        blake2b.update(&w.raw_data());
    }
    blake2b.finalize(&mut message);
    let message = H256::from(message);
    let sig = key.sign_recoverable(&message).expect("sign recoverable");
    signed_witnesses.push(
        witness
            .as_builder()
            .lock(Some(Bytes::from(sig.serialize())).pack())
            .build()
            .as_bytes()
            .pack(),
    );
    for i in 1..witnesses_len {
        signed_witnesses.push(tx.witnesses().get(i).unwrap());
    }
    tx.as_advanced_builder()
        .set_witnesses(signed_witnesses)
        .build()
}

fn assert_lock_script_error(result: Result<u64, Error>) {
    let err = result.expect_err("expected lock script verification to fail");
    let msg = err.to_string();
    assert!(
        msg.contains("Inputs[0].Lock") && msg.contains("ValidationFailure"),
        "expected Lock Script ValidationFailure on Inputs[0].Lock, but got: {msg}"
    );
}

struct AtomicTestFixture {
    context: Context,
    registry_out_point: OutPoint,
    state_out_point: OutPoint,
    state_code_hash: [u8; 32],
    state_hash_type: u8,
    secp256k1_data_dep: CellDep,
    admin_privkey: Privkey,
    admin_lock_script: Script,
    admin_lock_hash: [u8; 32],
    other_privkey: Privkey,
    other_lock_script: Script,
}

impl AtomicTestFixture {
    fn new() -> Self {
        let mut context = Context::default();

        // Deploy both real RISC-V contract binaries
        let registry_out_point = context.deploy_cell_by_name("registry");
        let state_out_point = context.deploy_cell_by_name("state");

        // Extract canonical State Type Script identity
        let state_ref_script = context
            .build_script(&state_out_point, Bytes::new())
            .expect("build state reference script");
        let state_code_hash = byte32_to_array(&state_ref_script.code_hash());
        let state_hash_type = state_ref_script.hash_type().as_slice()[0];

        // Deploy real CKB secp256k1_blake160_sighash_all Lock Script and secp256k1_data cell
        let secp256k1_data_bin = load_bundled_system_cell("specs/cells/secp256k1_data");
        let secp256k1_sighash_bin =
            load_bundled_system_cell("specs/cells/secp256k1_blake160_sighash_all");
        let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin);
        let secp256k1_sighash_out_point = context.deploy_cell(secp256k1_sighash_bin);
        let secp256k1_data_dep = CellDep::new_builder()
            .out_point(secp256k1_data_out_point)
            .dep_type(DepType::Code)
            .build();

        // Generate deterministic secp256k1 keypairs for Admin and non-Admin
        let admin_privkey = Privkey::from_slice(&[0x11u8; 32]);
        let admin_pubkey = admin_privkey.pubkey().expect("admin pubkey");
        let admin_pubkey_hash = blake160(&admin_pubkey.serialize());
        let admin_lock_script = context
            .build_script(
                &secp256k1_sighash_out_point,
                Bytes::from(admin_pubkey_hash.to_vec()),
            )
            .expect("build secp256k1 admin lock script");
        let admin_lock_hash = byte32_to_array(&admin_lock_script.calc_script_hash());

        let other_privkey = Privkey::from_slice(&[0x22u8; 32]);
        let other_pubkey = other_privkey.pubkey().expect("other pubkey");
        let other_pubkey_hash = blake160(&other_pubkey.serialize());
        let other_lock_script = context
            .build_script(
                &secp256k1_sighash_out_point,
                Bytes::from(other_pubkey_hash.to_vec()),
            )
            .expect("build secp256k1 other lock script");

        Self {
            context,
            registry_out_point,
            state_out_point,
            state_code_hash,
            state_hash_type,
            secp256k1_data_dep,
            admin_privkey,
            admin_lock_script,
            admin_lock_hash,
            other_privkey,
            other_lock_script,
        }
    }

    fn build_registry_script(&mut self, args: Bytes) -> Script {
        self.context
            .build_script(&self.registry_out_point, args)
            .expect("build real registry type script")
    }

    fn build_canonical_registry_script(&mut self, registry_id: &[u8; 32]) -> (Script, [u8; 32]) {
        let reg_args = registry_args(
            0x01,
            registry_id,
            &self.admin_lock_hash,
            &self.state_code_hash,
            self.state_hash_type,
        );
        assert_eq!(reg_args.len(), 98);
        let reg_script = self.build_registry_script(reg_args);
        let reg_type_hash = byte32_to_array(&reg_script.calc_script_hash());
        (reg_script, reg_type_hash)
    }

    fn build_state_script(&mut self, args: Bytes) -> Script {
        self.context
            .build_script(&self.state_out_point, args)
            .expect("build real state type script")
    }

    fn build_canonical_state_script(
        &mut self,
        item_id: &[u8; 32],
        trusted_registry_type_hash: &[u8; 32],
    ) -> Script {
        let st_args = state_spec_args(
            0x01,
            item_id,
            trusted_registry_type_hash,
            &self.admin_lock_hash,
        );
        assert_eq!(st_args.len(), 97);
        self.build_state_script(st_args)
    }

    fn complete_without_signing(&mut self, tx: TransactionView) -> TransactionView {
        let tx_with_secp_dep = tx
            .as_advanced_builder()
            .cell_dep(self.secp256k1_data_dep.clone())
            .build();
        self.context.complete_tx(tx_with_secp_dep)
    }

    fn complete_and_sign(&mut self, tx: TransactionView, key: &Privkey) -> TransactionView {
        let completed = self.complete_without_signing(tx);
        sign_tx(completed, key)
    }

    fn build_valid_atomic_registration_tx(
        &mut self,
        registry_id: &[u8; 32],
        existing_items: &[[u8; 32]],
        new_item_id: &[u8; 32],
    ) -> (TransactionView, OutPoint) {
        let (reg_type_script, reg_type_hash) = self.build_canonical_registry_script(registry_id);
        let state_type_script = self.build_canonical_state_script(new_item_id, &reg_type_hash);

        let reg_in_out_point = self.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(self.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
            registry_data(0x01, registry_id, existing_items),
        );

        let mut updated_items = existing_items.to_vec();
        updated_items.push(*new_item_id);

        let reg_input = CellInput::new_builder()
            .previous_output(reg_in_out_point.clone())
            .build();
        let reg_output = CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(self.admin_lock_script.clone())
            .type_(Some(reg_type_script).pack())
            .build();
        let state_output = CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(self.admin_lock_script.clone())
            .type_(Some(state_type_script).pack())
            .build();

        let tx = TransactionBuilder::default()
            .input(reg_input)
            .output(reg_output)
            .output_data(registry_data(0x01, registry_id, &updated_items))
            .output(state_output)
            .output_data(state_data(new_item_id, AVAILABLE))
            .build();

        (tx, reg_in_out_point)
    }
}

// ----------------------------------------------------------------------------
// 1. Positive Atomic Registration Tests (Both Real Binaries + Real Admin Sig)
// ----------------------------------------------------------------------------

#[test]
fn atomic_registration_allows_first_item_from_empty_registry() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];

    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);
    let expected_state_script = f.build_canonical_state_script(&item_1, &reg_type_hash);

    let (tx, reg_in_out_point) = f.build_valid_atomic_registration_tx(&registry_id, &[], &item_1);
    let admin_privkey = f.admin_privkey.clone();
    let signed_tx = f.complete_and_sign(tx, &admin_privkey);

    // Ensure the Registry Cell is consumed as an Input and NOT treated as a CellDep
    for dep in signed_tx.cell_deps() {
        assert_ne!(
            dep.out_point(),
            reg_in_out_point,
            "Registry Cell must not be a CellDep during atomic registration"
        );
    }

    // Verify Registry output data (1 item appended)
    let reg_out_data: Bytes = signed_tx.outputs_data().get(0).unwrap().unpack();
    assert_eq!(reg_out_data.len(), 37 + 32);
    assert_eq!(reg_out_data[0], 0x01);
    assert_eq!(&reg_out_data[1..33], &registry_id);
    assert_eq!(
        u32::from_le_bytes(reg_out_data[33..37].try_into().unwrap()),
        1
    );
    assert_eq!(&reg_out_data[37..69], &item_1);

    // Verify State output Args, Data, AVAILABLE status, Lock, and Type Script
    let state_out = signed_tx.outputs().get(1).unwrap();
    assert_eq!(state_out.lock().as_slice(), f.admin_lock_script.as_slice());
    let actual_state_script = state_out.type_().to_opt().expect("state type script");
    assert_eq!(
        actual_state_script.as_slice(),
        expected_state_script.as_slice()
    );
    let actual_state_args: Bytes = actual_state_script.args().unpack();
    assert_eq!(actual_state_args.len(), 97);
    assert_eq!(actual_state_args[0], 0x01);
    assert_eq!(&actual_state_args[1..33], &item_1);
    assert_eq!(&actual_state_args[33..65], &reg_type_hash);
    assert_eq!(&actual_state_args[65..97], &f.admin_lock_hash);

    let state_out_data: Bytes = signed_tx.outputs_data().get(1).unwrap().unpack();
    assert_eq!(state_out_data.len(), 34);
    assert_eq!(state_out_data[0], 0x01);
    assert_eq!(&state_out_data[1..33], &item_1);
    assert_eq!(state_out_data[33], AVAILABLE);

    let reg_out = signed_tx.outputs().get(0).unwrap();
    assert_eq!(
        reg_out.type_().to_opt().unwrap().as_slice(),
        reg_type_script.as_slice()
    );

    // Execute real secp256k1_blake160_sighash_all Lock Script + real Registry + real State scripts
    assert!(f.context.verify_tx(&signed_tx, MAX_CYCLES).is_ok());
}

#[test]
fn atomic_registration_allows_subsequent_item_with_existing_items() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];
    let item_3 = [0x03u8; 32];

    let (_, reg_type_hash) = f.build_canonical_registry_script(&registry_id);
    let expected_state_script = f.build_canonical_state_script(&item_3, &reg_type_hash);

    let (tx, reg_in_out_point) =
        f.build_valid_atomic_registration_tx(&registry_id, &[item_1, item_2], &item_3);
    let admin_privkey = f.admin_privkey.clone();
    let signed_tx = f.complete_and_sign(tx, &admin_privkey);

    for dep in signed_tx.cell_deps() {
        assert_ne!(dep.out_point(), reg_in_out_point);
    }

    // Verify Registry output preserves [item_1, item_2] and appends item_3
    let reg_out_data: Bytes = signed_tx.outputs_data().get(0).unwrap().unpack();
    assert_eq!(reg_out_data.len(), 37 + 32 * 3);
    assert_eq!(
        u32::from_le_bytes(reg_out_data[33..37].try_into().unwrap()),
        3
    );
    assert_eq!(&reg_out_data[37..69], &item_1);
    assert_eq!(&reg_out_data[69..101], &item_2);
    assert_eq!(&reg_out_data[101..133], &item_3);

    // Verify State output for item_3
    let state_out = signed_tx.outputs().get(1).unwrap();
    assert_eq!(
        state_out.type_().to_opt().unwrap().as_slice(),
        expected_state_script.as_slice()
    );
    let state_out_data: Bytes = signed_tx.outputs_data().get(1).unwrap().unpack();
    assert_eq!(state_out_data, state_data(&item_3, AVAILABLE));

    assert!(f.context.verify_tx(&signed_tx, MAX_CYCLES).is_ok());
}

#[test]
fn atomic_e2e_genesis_registration_and_lifecycle_with_real_scripts_and_admin_sig() {
    let mut f = AtomicTestFixture::new();
    let admin_privkey = f.admin_privkey.clone();
    let item_1 = [0xA1u8; 32];

    // Step 1: Registry Genesis (0 -> 1) with CKB Type ID and real Admin signature
    let seed_out_point = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let genesis_input = CellInput::new_builder()
        .previous_output(seed_out_point)
        .since(0u64)
        .build();
    let registry_id = derive_registry_id(&genesis_input, 0);
    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);

    let genesis_reg_output = CellOutput::new_builder()
        .capacity(CAPACITY * 10)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script.clone()).pack())
        .build();
    let genesis_reg_data = registry_data(0x01, &registry_id, &[]);

    let genesis_tx = TransactionBuilder::default()
        .input(genesis_input)
        .output(genesis_reg_output.clone())
        .output_data(genesis_reg_data.clone())
        .build();
    let signed_genesis_tx = f.complete_and_sign(genesis_tx, &admin_privkey);
    assert!(f.context.verify_tx(&signed_genesis_tx, MAX_CYCLES).is_ok());

    // Step 2: Atomic Registration (Registry 1 -> 1, State 0 -> 1)
    let reg_cell_0 = f.context.create_cell(genesis_reg_output, genesis_reg_data);
    let state_type_script = f.build_canonical_state_script(&item_1, &reg_type_hash);

    let reg_output_1 = CellOutput::new_builder()
        .capacity(CAPACITY * 10)
        .lock(f.admin_lock_script.clone())
        .type_(Some(reg_type_script).pack())
        .build();
    let reg_data_1 = registry_data(0x01, &registry_id, &[item_1]);

    let state_output_0 = CellOutput::new_builder()
        .capacity(CAPACITY)
        .lock(f.admin_lock_script.clone())
        .type_(Some(state_type_script.clone()).pack())
        .build();
    let state_data_0 = state_data(&item_1, AVAILABLE);

    let reg_tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_cell_0).build())
        .output(reg_output_1.clone())
        .output_data(reg_data_1.clone())
        .output(state_output_0.clone())
        .output_data(state_data_0.clone())
        .build();
    let signed_reg_tx = f.complete_and_sign(reg_tx, &admin_privkey);
    assert!(f.context.verify_tx(&signed_reg_tx, MAX_CYCLES).is_ok());

    // Step 3: Lifecycle Transition (AVAILABLE -> BORROWED) with Registry CellDep
    let reg_cell_1 = f.context.create_cell(reg_output_1, reg_data_1);
    let reg_cell_dep = CellDep::new_builder()
        .out_point(reg_cell_1)
        .dep_type(DepType::Code)
        .build();
    let state_cell_0 = f.context.create_cell(state_output_0, state_data_0);

    let borrow_tx = TransactionBuilder::default()
        .cell_dep(reg_cell_dep)
        .input(
            CellInput::new_builder()
                .previous_output(state_cell_0)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, BORROWED))
        .build();
    let signed_borrow_tx = f.complete_and_sign(borrow_tx, &admin_privkey);
    assert!(f.context.verify_tx(&signed_borrow_tx, MAX_CYCLES).is_ok());
}

// ----------------------------------------------------------------------------
// 2. Admin Signature Verification Tests (Real secp256k1_blake160_sighash_all)
// ----------------------------------------------------------------------------

#[test]
fn atomic_registration_admin_signature_valid_succeeds() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];

    let (tx, _) = f.build_valid_atomic_registration_tx(&registry_id, &[], &item_1);
    let admin_privkey = f.admin_privkey.clone();
    let signed_tx = f.complete_and_sign(tx, &admin_privkey);

    assert!(f.context.verify_tx(&signed_tx, MAX_CYCLES).is_ok());
}

#[test]
fn atomic_registration_admin_signature_invalid_fails() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];

    let (tx, _) = f.build_valid_atomic_registration_tx(&registry_id, &[], &item_1);

    // 1. Signed with non-admin private key (other_privkey) -> Lock Script rejects
    let other_privkey = f.other_privkey.clone();
    let wrong_key_tx = f.complete_and_sign(tx.clone(), &other_privkey);
    assert_lock_script_error(f.context.verify_tx(&wrong_key_tx, MAX_CYCLES));

    // 2. Corrupted 65-byte signature in WitnessArgs.lock -> Lock Script rejects
    let corrupted_witness = WitnessArgs::default()
        .as_builder()
        .lock(Some(Bytes::from(vec![0xFFu8; 65])).pack())
        .build();
    let corrupted_tx = f
        .complete_without_signing(tx)
        .as_advanced_builder()
        .set_witnesses(vec![corrupted_witness.as_bytes().pack()])
        .build();
    assert_lock_script_error(f.context.verify_tx(&corrupted_tx, MAX_CYCLES));
}

#[test]
fn atomic_registration_admin_signature_missing_fails() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];

    let (tx, _) = f.build_valid_atomic_registration_tx(&registry_id, &[], &item_1);

    // 1. No witnesses present at all -> Lock Script rejects
    let unsigned_tx = f.complete_without_signing(tx.clone());
    assert_eq!(unsigned_tx.witnesses().len(), 0);
    assert_lock_script_error(f.context.verify_tx(&unsigned_tx, MAX_CYCLES));

    // 2. Empty WitnessArgs with lock = None -> Lock Script rejects
    let empty_witness = WitnessArgs::default();
    let empty_witness_tx = f
        .complete_without_signing(tx)
        .as_advanced_builder()
        .set_witnesses(vec![empty_witness.as_bytes().pack()])
        .build();
    assert_lock_script_error(f.context.verify_tx(&empty_witness_tx, MAX_CYCLES));
}

// ----------------------------------------------------------------------------
// 3. Negative State Registration Tests (0 GroupInput -> 1 GroupOutput)
//    Uses StateTestFixture (real State binary, mock Registry script) so that
//    State Registration Mode validation is tested directly without pre-emption.
// ----------------------------------------------------------------------------

#[test]
fn state_registration_rejects_without_registry_update() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
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

    let s_args = state_spec_args(0x01, &item_1, &reg_type_hash, &f.admin_lock_hash);
    let state_type_script = f.build_state_script(s_args);

    let plain_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );

    // Even if a valid Registry CellDep is attached, Registration Mode requires Registry 1->1 in Input/Output
    let reg_dep = f.create_registry_cell_dep(
        f.admin_lock_script.clone(),
        Some(reg_type_script),
        registry_data(0x01, &registry_id, &[item_1]),
    );

    let tx = TransactionBuilder::default()
        .cell_dep(reg_dep)
        .input(CellInput::new_builder().previous_output(plain_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_registration_rejects_untrusted_registry_type_hash() {
    let mut f = StateTestFixture::new();
    let trusted_registry_id = [0x55u8; 32];
    let untrusted_registry_id = [0x66u8; 32];
    let item_1 = [0x01u8; 32];

    let trusted_reg_script = f.build_registry_script(registry_args(
        0x01,
        &trusted_registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let trusted_reg_hash = byte32_to_array(&trusted_reg_script.calc_script_hash());

    let untrusted_reg_script = f.build_registry_script(registry_args(
        0x01,
        &untrusted_registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(untrusted_reg_script.clone()).pack())
            .build(),
        registry_data(0x01, &untrusted_registry_id, &[]),
    );

    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &trusted_reg_hash,
        &f.admin_lock_hash,
    ));

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(untrusted_reg_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &untrusted_registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_registration_rejects_missing_registry_input() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let plain_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .build(),
        Bytes::new(),
    );

    // Registry output is present, but Registry input is missing
    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(plain_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_registration_rejects_missing_registry_output() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    // Registry input is present, but Registry output is missing
    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_registration_rejects_multiple_registry_inputs() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let reg_in_1 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let reg_in_2 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_1).build())
        .input(CellInput::new_builder().previous_output(reg_in_2).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MULTIPLE_CELLS);
}

#[test]
fn state_registration_rejects_multiple_registry_outputs() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_MULTIPLE_CELLS);
}

#[test]
fn state_registration_rejects_mismatched_appended_item_id() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let state_item_id = [0x01u8; 32];
    let registry_appended_id = [0x02u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &state_item_id,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    // Registry appends registry_appended_id (0x02), while State output is for state_item_id (0x01)
    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[registry_appended_id]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&state_item_id, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );
}

#[test]
fn state_registration_rejects_non_available_initial_status() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    for bad_status in [BORROWED, RETURNED] {
        let reg_in = f.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
            registry_data(0x01, &registry_id, &[]),
        );

        let tx = TransactionBuilder::default()
            .input(CellInput::new_builder().previous_output(reg_in).build())
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(reg_type_script.clone()).pack())
                    .build(),
            )
            .output_data(registry_data(0x01, &registry_id, &[item_1]))
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(state_type_script.clone()).pack())
                    .build(),
            )
            .output_data(state_data(&item_1, bad_status))
            .build();
        let tx = f.context.complete_tx(tx);
        assert_script_error(
            f.context.verify_tx(&tx, MAX_CYCLES),
            ERROR_INVALID_TRANSITION,
        );
    }
}

#[test]
fn state_registration_rejects_wrong_state_output_lock() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.other_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_UNAUTHORIZED);
}

#[test]
fn state_registration_rejects_missing_admin_locked_input() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    // Only input in transaction is locked by other_lock_script (no admin-locked input)
    let non_admin_reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.other_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let tx = TransactionBuilder::default()
        .input(
            CellInput::new_builder()
                .previous_output(non_admin_reg_in)
                .build(),
        )
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_UNAUTHORIZED);
}

#[test]
fn state_registration_rejects_invalid_registry_input_or_output_data() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_2,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let invalid_cases: Vec<(Bytes, Bytes, i8)> = vec![
        // 1. Invalid Registry input schema version (0x02) -> ERROR_INVALID_DATA (2)
        (
            registry_data(0x02, &registry_id, &[item_1]),
            registry_data(0x01, &registry_id, &[item_1, item_2]),
            ERROR_INVALID_DATA,
        ),
        // 2. Inconsistent Registry output registry_id -> ERROR_INVALID_DATA (2)
        (
            registry_data(0x01, &registry_id, &[item_1]),
            registry_data(0x01, &[0xFFu8; 32], &[item_1, item_2]),
            ERROR_INVALID_DATA,
        ),
        // 3. Corrupted duplicate items in Registry input -> ERROR_INVALID_DATA (2)
        (
            registry_data(0x01, &registry_id, &[item_1, item_1]),
            registry_data(0x01, &registry_id, &[item_1, item_1, item_2]),
            ERROR_INVALID_DATA,
        ),
        // 4. Malformed Registry output data length (truncated) -> ERROR_INVALID_DATA (2)
        (
            registry_data(0x01, &registry_id, &[item_1]),
            Bytes::from(vec![0x01; 36]),
            ERROR_INVALID_DATA,
        ),
        // 5. Registry output count != input count + 1 (0 -> 2 items) -> ERROR_INVALID_TRANSITION (3)
        (
            registry_data(0x01, &registry_id, &[]),
            registry_data(0x01, &registry_id, &[item_1, item_2]),
            ERROR_INVALID_TRANSITION,
        ),
    ];

    for (in_data, out_data, expected_err) in invalid_cases {
        let reg_in = f.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
            in_data,
        );

        let tx = TransactionBuilder::default()
            .input(CellInput::new_builder().previous_output(reg_in).build())
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(reg_type_script.clone()).pack())
                    .build(),
            )
            .output_data(out_data)
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(state_type_script.clone()).pack())
                    .build(),
            )
            .output_data(state_data(&item_2, AVAILABLE))
            .build();
        let tx = f.context.complete_tx(tx);
        assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), expected_err);
    }
}

#[test]
fn state_registration_rejects_altered_registry_prefix() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];
    let item_3 = [0x03u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_3,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_1, item_2]),
    );

    // Reorder existing prefix [item_2, item_1] when registering item_3
    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_2, item_1, item_3]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_3, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(
        f.context.verify_tx(&tx, MAX_CYCLES),
        ERROR_INVALID_TRANSITION,
    );
}

#[test]
fn state_registration_rejects_duplicate_item_id() {
    let mut f = StateTestFixture::new();
    let registry_id = [0x55u8; 32];
    let item_1 = [0x01u8; 32];

    let reg_type_script = f.build_registry_script(registry_args(
        0x01,
        &registry_id,
        &f.admin_lock_hash,
        &f.state_code_hash,
        f.state_hash_type,
    ));
    let reg_type_hash = byte32_to_array(&reg_type_script.calc_script_hash());
    let state_type_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_1]),
    );

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1, item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let tx = f.context.complete_tx(tx);
    assert_script_error(f.context.verify_tx(&tx, MAX_CYCLES), ERROR_DUPLICATE_ITEM);
}

// ----------------------------------------------------------------------------
// 4. Negative Registry-State Integration Tests (AtomicTestFixture:
//    Both Real Binaries + Real secp256k1_blake160_sighash_all Admin Signature)
// ----------------------------------------------------------------------------

#[test]
fn atomic_registration_rejects_missing_canonical_state_output() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];

    let (reg_type_script, _) = f.build_canonical_registry_script(&registry_id);
    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .build();
    let admin_privkey = f.admin_privkey.clone();
    let signed_tx = f.complete_and_sign(tx, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_tx, MAX_CYCLES),
        ERROR_MISSING_CELL,
    );
}

#[test]
fn atomic_registration_rejects_multiple_canonical_state_outputs() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];

    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);
    let state_script_1 = f.build_canonical_state_script(&item_1, &reg_type_hash);
    let state_script_2 = f.build_canonical_state_script(&item_2, &reg_type_hash);

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_1).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_2).pack())
                .build(),
        )
        .output_data(state_data(&item_2, AVAILABLE))
        .build();
    let admin_privkey = f.admin_privkey.clone();
    let signed_tx = f.complete_and_sign(tx, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_tx, MAX_CYCLES),
        ERROR_MULTIPLE_CELLS,
    );
}

#[test]
fn atomic_registration_rejects_mismatched_state_item_id() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];
    let admin_privkey = f.admin_privkey.clone();

    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);
    let state_script_item_1 = f.build_canonical_state_script(&item_1, &reg_type_hash);
    let state_script_item_2 = f.build_canonical_state_script(&item_2, &reg_type_hash);

    // 1. State Args has item_2 while Registry appends item_1:
    //    - Inputs[0].Type (Registry group) rejects with ERROR_INVALID_ARGS (1)
    //    - Outputs[1].Type (State group) rejects with ERROR_UNTRUSTED_REGISTRY (10)
    let reg_in_1 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let tx_bad_args_item = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_1).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_item_2).pack())
                .build(),
        )
        .output_data(state_data(&item_2, AVAILABLE))
        .build();
    let signed_1 = f.complete_and_sign(tx_bad_args_item, &admin_privkey);
    let res_1 = f.context.verify_tx(&signed_1, MAX_CYCLES);
    let err_msg_1 = res_1
        .as_ref()
        .expect_err("expected atomic registration to fail")
        .to_string();
    if err_msg_1.contains("Inputs[0].Type") {
        assert_script_error(res_1, ERROR_INVALID_ARGS);
    } else {
        assert!(err_msg_1.contains("Outputs[1].Type"));
        assert_script_error(res_1, ERROR_UNTRUSTED_REGISTRY);
    }

    // 2. State Args has item_1, but State Data has item_2 -> ERROR_INVALID_DATA (2)
    let reg_in_2 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let tx_bad_data_item = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_2).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_script_item_1).pack())
                .build(),
        )
        .output_data(state_data(&item_2, AVAILABLE))
        .build();
    let signed_2 = f.complete_and_sign(tx_bad_data_item, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_2, MAX_CYCLES),
        ERROR_INVALID_DATA,
    );
}

#[test]
fn atomic_registration_rejects_wrong_state_args() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let admin_privkey = f.admin_privkey.clone();

    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);

    // 1. Wrong trusted_registry_type_hash in State Args -> ERROR_UNTRUSTED_REGISTRY (10)
    let wrong_reg_hash_script = f.build_state_script(state_spec_args(
        0x01,
        &item_1,
        &[0xFFu8; 32],
        &f.admin_lock_hash,
    ));
    let reg_in_1 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let tx_wrong_reg_hash = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_1).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(wrong_reg_hash_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let signed_1 = f.complete_and_sign(tx_wrong_reg_hash, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_1, MAX_CYCLES),
        ERROR_UNTRUSTED_REGISTRY,
    );

    // 2. Wrong version in State Args (0x02) -> ERROR_INVALID_ARGS (1)
    let wrong_ver_script = f.build_state_script(state_spec_args(
        0x02,
        &item_1,
        &reg_type_hash,
        &f.admin_lock_hash,
    ));
    let reg_in_2 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );
    let tx_wrong_ver = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_2).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(wrong_ver_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let signed_2 = f.complete_and_sign(tx_wrong_ver, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_2, MAX_CYCLES),
        ERROR_INVALID_ARGS,
    );
}

#[test]
fn atomic_registration_rejects_wrong_state_lock() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let admin_privkey = f.admin_privkey.clone();

    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);
    let state_type_script = f.build_canonical_state_script(&item_1, &reg_type_hash);

    let reg_in = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[]),
    );

    let tx = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.other_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_1, AVAILABLE))
        .build();
    let signed_tx = f.complete_and_sign(tx, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_tx, MAX_CYCLES),
        ERROR_UNAUTHORIZED,
    );
}

#[test]
fn atomic_registration_rejects_non_available_state_output() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let admin_privkey = f.admin_privkey.clone();

    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);
    let state_type_script = f.build_canonical_state_script(&item_1, &reg_type_hash);

    for bad_status in [BORROWED, RETURNED] {
        let reg_in = f.context.create_cell(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
            registry_data(0x01, &registry_id, &[]),
        );

        let tx = TransactionBuilder::default()
            .input(CellInput::new_builder().previous_output(reg_in).build())
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY * 10)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(reg_type_script.clone()).pack())
                    .build(),
            )
            .output_data(registry_data(0x01, &registry_id, &[item_1]))
            .output(
                CellOutput::new_builder()
                    .capacity(CAPACITY)
                    .lock(f.admin_lock_script.clone())
                    .type_(Some(state_type_script.clone()).pack())
                    .build(),
            )
            .output_data(state_data(&item_1, bad_status))
            .build();
        let signed_tx = f.complete_and_sign(tx, &admin_privkey);
        assert_script_error(
            f.context.verify_tx(&signed_tx, MAX_CYCLES),
            ERROR_INVALID_TRANSITION,
        );
    }
}

#[test]
fn atomic_registration_rejects_duplicate_registration() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let admin_privkey = f.admin_privkey.clone();

    let (tx, _) = f.build_valid_atomic_registration_tx(&registry_id, &[item_1], &item_1);
    let signed_tx = f.complete_and_sign(tx, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_tx, MAX_CYCLES),
        ERROR_DUPLICATE_ITEM,
    );
}

#[test]
fn atomic_registration_rejects_invalid_append_only_update() {
    let mut f = AtomicTestFixture::new();
    let registry_id = [0x44u8; 32];
    let item_1 = [0x01u8; 32];
    let item_2 = [0x02u8; 32];
    let item_3 = [0x03u8; 32];
    let admin_privkey = f.admin_privkey.clone();

    let (reg_type_script, reg_type_hash) = f.build_canonical_registry_script(&registry_id);
    let state_type_script = f.build_canonical_state_script(&item_3, &reg_type_hash);

    // 1. Reorder existing prefix [item_2, item_1, item_3] -> ERROR_INVALID_TRANSITION (3)
    let reg_in_1 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_1, item_2]),
    );
    let tx_reordered = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_1).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script.clone()).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_2, item_1, item_3]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script.clone()).pack())
                .build(),
        )
        .output_data(state_data(&item_3, AVAILABLE))
        .build();
    let signed_reordered = f.complete_and_sign(tx_reordered, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_reordered, MAX_CYCLES),
        ERROR_INVALID_TRANSITION,
    );

    // 2. Overwrite existing item without incrementing count ([item_1, item_2] -> [item_1, item_3]) -> ERROR_INVALID_TRANSITION (3)
    let reg_in_2 = f.context.create_cell(
        CellOutput::new_builder()
            .capacity(CAPACITY * 10)
            .lock(f.admin_lock_script.clone())
            .type_(Some(reg_type_script.clone()).pack())
            .build(),
        registry_data(0x01, &registry_id, &[item_1, item_2]),
    );
    let tx_overwritten = TransactionBuilder::default()
        .input(CellInput::new_builder().previous_output(reg_in_2).build())
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY * 10)
                .lock(f.admin_lock_script.clone())
                .type_(Some(reg_type_script).pack())
                .build(),
        )
        .output_data(registry_data(0x01, &registry_id, &[item_1, item_3]))
        .output(
            CellOutput::new_builder()
                .capacity(CAPACITY)
                .lock(f.admin_lock_script.clone())
                .type_(Some(state_type_script).pack())
                .build(),
        )
        .output_data(state_data(&item_3, AVAILABLE))
        .build();
    let signed_overwritten = f.complete_and_sign(tx_overwritten, &admin_privkey);
    assert_script_error(
        f.context.verify_tx(&signed_overwritten, MAX_CYCLES),
        ERROR_INVALID_TRANSITION,
    );
}
