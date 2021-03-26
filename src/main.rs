fn main() {
    println!("AAAA");
    if (cfg!(feature = "server") && std::env::args().skip(1).next().as_deref() == Some("s"))
        || !cfg!(feature = "client")
    {
        #[cfg(feature = "server")]
        {
            println!("Running server");
            tank_game::run_server();
        }
    } else {
        #[cfg(feature = "client")]
        {
            println!("Running client");
            tank_game::run_client::<tank_game::PixelsEventLoop>();
        }
    }
    println!("Hello, world!");
}
