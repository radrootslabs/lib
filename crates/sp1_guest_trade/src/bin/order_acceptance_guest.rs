#![cfg_attr(target_os = "zkvm", no_main)]

use radroots_sp1_guest_trade::{
    RadrootsSp1TradeOrderAcceptanceWitness, reduce_order_acceptance_canonical_public_values,
};

sp1_zkvm::entrypoint!(main);

fn main() {
    let witness = sp1_zkvm::io::read::<RadrootsSp1TradeOrderAcceptanceWitness>();
    let public_values = reduce_order_acceptance_canonical_public_values(&witness)
        .expect("valid radroots order acceptance witness");
    sp1_zkvm::io::commit_slice(&public_values);
}
