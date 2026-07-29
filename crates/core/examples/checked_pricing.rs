use radroots_core::pricing::QuantityPriceOps;
use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};

fn main() -> Result<(), radroots_core::Error> {
    let price = QuantityPrice::try_new(
        Money::try_new("6.00".parse::<Decimal>()?, Currency::USD)?,
        Quantity::try_new(Decimal::from(2_u32), Unit::MassKg)?,
    )?;
    let requested = Quantity::try_new(Decimal::from(3_u32), Unit::MassKg)?;
    let total = price.try_cost_for_rounded(&requested)?;

    assert_eq!(total.amount().to_string(), "9");
    Ok(())
}
