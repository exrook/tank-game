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
            let backend_count = 0;
            #[cfg(feature = "druid_backend")]
            let backend_count = backend_count + 1;
            #[cfg(feature = "pixels_backend")]
            let backend_count = backend_count + 1;
            #[cfg(feature = "minifb_backend")]
            let backend_count = backend_count + 1;

            let mut args = std::env::args().skip(1);
            let arg = args.next();
            let second_arg = args.next();
            let host = if backend_count > 1 {
                second_arg.as_deref()
            } else {
                arg.as_deref()
            };
            println!("IM A {:?}", arg);
            let arg = arg.clone().unwrap_or_default();
            println!("IM A {:?}", arg);

            #[cfg(feature = "druid_backend")]
            if backend_count == 1 || arg == "druid" {
                tank_game::run_client::<tank_game::DruidEventLoop>(host);
            }
            #[cfg(feature = "minifb_backend")]
            if backend_count == 1 || arg == "minifb" {
                tank_game::run_client::<tank_game::MinifbEventLoop>(host);
            }
            #[cfg(feature = "pixels_backend")]
            if backend_count == 1 || arg == "pixels" {
                tank_game::run_client::<tank_game::PixelsEventLoop>(host);
            }
        }
    }
    println!("Hello, world!");
}
