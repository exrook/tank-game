fn main() {
    if std::env::args().skip(1).next().as_deref() == Some("s") {
        println!("Running server");
        tank_game::run_server();
    } else {
        println!("Running client");
        tank_game::run_client::<tank_game::PathfinderEventLoop>();
    }
    println!("Hello, world!");
}
