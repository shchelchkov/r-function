use geo::{LineString, Polygon, Point};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Value};

#[derive(Debug)]
pub enum PolygonError {
    MissingField(&'static str),
    InvalidType,
    InvalidCoordinates,
    InvalidPosition,
}

pub fn value_to_point(value: &Value) -> Result<(i64, Point<f64>), PolygonError> {
    let object = value.as_object().ok_or(PolygonError::InvalidType)?;

    let id = object
        .get(&"id")
        .and_then(|c| c.as_i64())
        .and_then(|id| i64::try_from(id).ok())
        .ok_or(PolygonError::MissingField("id"))?;

    let geometry_type = object
        .get(&"type")
        .and_then(|c| c.as_str())
        .ok_or(PolygonError::MissingField("type"))?;

    if geometry_type != "Point" {
        return Err(PolygonError::InvalidType);
    }

    let coordinates = object
        .get(&"coordinates")
        .ok_or(PolygonError::MissingField("coordinates"))?;

    let position = coordinates
        .as_array()
        .ok_or(PolygonError::InvalidPosition)?;

    if position.len() < 2 {
        return Err(PolygonError::InvalidPosition);
    }

    let x = position[0].as_f64().ok_or(PolygonError::InvalidPosition)?;

    let y = position[1].as_f64().ok_or(PolygonError::InvalidPosition)?;

    Ok((id, Point::new(x, y)))
}

pub fn value_to_polygon(value: &Value) -> Result<(i64, Polygon<f64>), PolygonError> {
    let object = value.as_object().ok_or(PolygonError::InvalidType)?;

    let id = object
        .get(&"id")
        .and_then(|c| c.as_i64())
        .and_then(|id| i64::try_from(id).ok())
        .ok_or(PolygonError::MissingField("id"))?;

    let geometry_type = object
        .get(&"type")
        .and_then(|c| c.as_str())
        .ok_or(PolygonError::MissingField("type"))?;

    if geometry_type != "Polygon" {
        return Err(PolygonError::InvalidType);
    }

    let coordinates = object
        .get(&"coordinates")
        .ok_or(PolygonError::MissingField("coordinates"))?;

    let rings = coordinates
        .as_array()
        .ok_or(PolygonError::InvalidCoordinates)?;

    if rings.is_empty() {
        return Err(PolygonError::InvalidCoordinates);
    }

    let exterior = parse_ring(&rings[0])?;

    let interiors = rings
        .iter()
        .skip(1)
        .map(parse_ring)
        .collect::<Result<Vec<_>, _>>()?;

    Ok((id, Polygon::new(exterior, interiors)))
}

fn parse_ring(value: &Value) -> Result<LineString<f64>, PolygonError> {
    let points = value.as_array().ok_or(PolygonError::InvalidCoordinates)?;

    let coords = points
        .iter()
        .map(parse_position)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LineString::from(coords))
}

fn parse_position(value: &Value) -> Result<(f64, f64), PolygonError> {
    let position = value.as_array().ok_or(PolygonError::InvalidPosition)?;

    if position.len() < 2 {
        return Err(PolygonError::InvalidPosition);
    }

    let x = position[0].as_f64().ok_or(PolygonError::InvalidPosition)?;

    let y = position[1].as_f64().ok_or(PolygonError::InvalidPosition)?;

    Ok((x, y))
}
