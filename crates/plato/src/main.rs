mod app;

use plato_core::anyhow::Error;
use crate::app::run;

#[cfg(feature = "devel")]
use env_logger;


fn main() -> Result<(), Error> {
    #[cfg(feature = "devel")]
    env_logger::init();

    run()?;
    Ok(())
}
