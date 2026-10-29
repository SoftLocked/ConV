use conv_protocol::sync::SyncConfig;
use conv_simulator::sync_network::{Faults, SyncNetwork};

fn main() {
    let mut net = SyncNetwork::new(2, 42, SyncConfig::default(), Faults::default());
    net.partition(0, 1);
    net.insert(0, None, "Hello ").unwrap();
    net.insert(1, None, "ConV!").unwrap();
    net.run_until(2_000).unwrap();
    println!(
        "Offline copies: {:?}, {:?}",
        net.peer(0).replica().document().text(),
        net.peer(1).replica().document().text()
    );
    net.heal();
    net.run_until(5_000).unwrap();
    assert!(net.converged());
    println!(
        "After automatic repair: {:?}",
        net.peer(0).replica().document().text()
    );
    println!(
        "Missing-range requests delivered: {}",
        net.requests_delivered
    );
}
