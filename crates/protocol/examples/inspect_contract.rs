use radroots_protocol::{capability, event, runtime, schema};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    capability::v1::validate_catalog(capability::v1::CATALOG)?;
    event::v1::validate_catalog(event::v1::CATALOG)?;
    event::v1::validate_trade_state_vocabulary(event::v1::TRADE_STATE_VOCABULARY)?;
    runtime::v1::validate_catalog(runtime::v1::CATALOG)?;

    let registry = schema::protocol_v1_registry()?;
    println!("validated {} protocol schemas", registry.len());
    Ok(())
}
