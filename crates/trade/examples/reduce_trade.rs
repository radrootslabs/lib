use radroots_event::trade::TradeId;
use radroots_trade::{ReductionInput, reducer::reduce_trade_records};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trade_id = TradeId::parse("0123456789abcdef".repeat(2))?;
    let projection = reduce_trade_records(ReductionInput::new(trade_id));

    assert_eq!(projection.trade_id(), &trade_id);
    Ok(())
}
