use magnus::{define_module, function, prelude::*, Error};

fn hello() -> String {
    "Hello from Expanse!".to_string()
}

#[magnus::init]
fn init() -> Result<(), Error> {
    let module = define_module("Expanse")?;
    module.define_singleton_method("hello", function!(hello, 0))?;
    Ok(())
}
