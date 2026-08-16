use sonic_rs::{JsonValueTrait, Object, Value};

pub fn data_trades<'a>(obj: &'a Object) -> Option<(&'a Value, &'a Value, &'a Value, &'a str)> {
    let symbol = obj.get(&"s")?.as_str()?;
    let price = obj.get(&"p")?;
    let size = obj.get(&"v")?;
    let side = obj.get(&"S")?;
    Some((price, size, side, symbol))
}
