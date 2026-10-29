use conv_simulator::Network;

fn main() {
    let mut network = Network::new(2);
    network.partition(0, 1);
    network.peer_mut(0).insert(None, "Hello ").unwrap();
    network.peer_mut(1).insert(None, "ConV!").unwrap();
    println!(
        "Partitioned: {:?}, {:?}",
        network.peer(0).document().text(),
        network.peer(1).document().text()
    );
    network.heal();
    network.repair(0, 1, 10);
    network.repair(1, 0, 20);
    network.drain().unwrap();
    assert!(network.converged());
    println!(
        "Converged at virtual {} ms: {:?}",
        network.now(),
        network.peer(0).document().text()
    );
}
