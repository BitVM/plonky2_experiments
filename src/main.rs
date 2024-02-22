use anyhow::Result;
use plonky2::field::extension::Extendable;
//use plonky2::field::types::Field;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::RichField;
//use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CommonCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, PoseidonGoldilocksConfig};
//use plonky2::recursion::cyclic_recursion::check_cyclic_proof_verifier_data;
//use plonky2::recursion::dummy_circuit::cyclic_base_proof;

fn common_data_for_recursion<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>() -> CommonCircuitData<F, D>
where
    C::Hasher: AlgebraicHasher<F>,
{
    let config = CircuitConfig::standard_recursion_config();
    let builder = CircuitBuilder::<F, D>::new(config);
    let data = builder.build::<C>();
    let config = CircuitConfig::standard_recursion_config();
    let mut builder = CircuitBuilder::<F, D>::new(config);
    let proof = builder.add_virtual_proof_with_pis(&data.common);
    let verifier_data = builder.add_virtual_verifier_data(data.common.config.fri_config.cap_height);
    builder.verify_proof::<C>(&proof, &verifier_data, &data.common);
    builder.verify_proof::<C>(&proof, &verifier_data, &data.common);
    let data = builder.build::<C>();

    let config = CircuitConfig::standard_recursion_config();
    let mut builder = CircuitBuilder::<F, D>::new(config);
    let proof = builder.add_virtual_proof_with_pis(&data.common);
    let verifier_data = builder.add_virtual_verifier_data(data.common.config.fri_config.cap_height);
    builder.verify_proof::<C>(&proof, &verifier_data, &data.common);
    builder.verify_proof::<C>(&proof, &verifier_data, &data.common);
    while builder.num_gates() < 1 << 12 {
        builder.add_gate(NoopGate, vec![]);
    }
    builder.build::<C>().common
}

// Example Sum for PCD/IVC (output = previous_output + other_proof_output),
// The previous proof and other proof are supposed to be the same IVC circuit.
// Layout:
// - initial value
// - output value
//
// Condition:
// - verify_proofs (boolean)
fn main() -> Result<()> {
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    let config = CircuitConfig::standard_recursion_config();
    let mut builder = CircuitBuilder::<F, D>::new(config);

    // Public inputs
    let initial_value = builder.add_virtual_public_input();
    let output_value = builder.add_virtual_public_input();

    // TODO: Is the return value supposed to be unused?
    let _verifier_data_target = builder.add_verifier_data_public_inputs();
    
    let mut common_data = common_data_for_recursion::<F, C, D>();
    common_data.num_public_inputs = builder.num_public_inputs();
    
    let verify_proofs = builder.add_virtual_bool_target_safe();

    // Unpack inner proof's public inputs.
    let inner_cyclic_proof_with_pis = builder.add_virtual_proof_with_pis(&common_data);
    let inner_cyclic_pis = &inner_cyclic_proof_with_pis.public_inputs;
    let inner_cyclic_initial_value = inner_cyclic_pis[0];
    let inner_cyclic_output_value = inner_cyclic_pis[1];

    // Unpack other proof's public inputs.
    let other_proof_with_pis = builder.add_virtual_proof_with_pis(&common_data);
    let other_pis = &other_proof_with_pis.public_inputs;
    let _other_initial_value = other_pis[0];
    let other_output_value = other_pis[1];
    
    // Connect our initial value to that of our inner proof.
    builder.connect(initial_value, inner_cyclic_initial_value);

    // The input value is the previous output value if we have an inner proof, or the initial
    // value if this is the base case.
    // Initial value could be constrained to be 0.
    let actual_value_in = builder.select(verify_proofs, inner_cyclic_output_value, initial_value);

    let new_output_value = builder.mul_add(verify_proofs.target, other_output_value, actual_value_in);
    builder.connect(output_value, new_output_value);
    
    // Verify inner proof
    builder.conditionally_verify_cyclic_proof_or_dummy::<C>(
        verify_proofs,
        &inner_cyclic_proof_with_pis,
        &common_data,
    )?;

    // Verify other proof
    builder.conditionally_verify_cyclic_proof_or_dummy::<C>(
        verify_proofs,
        &other_proof_with_pis,
        &common_data,
    )?;

    // END OF THE CIRCUIT

    // TODO: Generate dummy proof with cyclic_base_proof
    // https://docs.rs/plonky2/0.1.4/plonky2/recursion/dummy_circuit/fn.cyclic_base_proof.html

    let _cyclic_circuit_data = builder.build::<C>();
    Ok(())
}
