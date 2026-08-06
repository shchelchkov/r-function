#[cfg(test)]
mod tests {
    use super::*;
    use geo::LineString;
    use geo::algorithm::contains::Contains;
    use r_tree::value::convert::{value_to_point, value_to_polygon, PolygonError};
    use sonic_rs::json;

    #[test]
    fn point_inside_rectangle() {
        let polygon_value = sonic_rs::json!({
        "id": 1,
        "type": "Polygon",
        "coordinates": [[
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0]
        ]]
    });

        let point_value = sonic_rs::json!({
        "id": 2,
        "type": "Point",
        "coordinates": [5.0, 5.0]
    });

        let (_, polygon) = value_to_polygon(&polygon_value).unwrap();
        let (_, point) = value_to_point(&point_value).unwrap();

        use geo::Contains;

        assert!(polygon.contains(&point));
    }


    #[test]
    fn point_outside_rectangle() {
        let value = json!({
            "id": 1,
            "type": "Polygon",
            "coordinates": [[
                [0.0, 0.0],
                [10.0, 0.0],
                [10.0, 10.0],
                [0.0, 10.0],
                [0.0, 0.0]
            ]]
        });

        let (_, polygon) = value_to_polygon(&value).unwrap();

        let point = geo::Point::new(15.0, 5.0);

        assert!(!polygon.contains(&point));
    }

    #[test]
    fn value_to_polygon_parses_polygon_with_hole() {
        let value = json!({
            "id": 42,
            "type": "Polygon",
            "coordinates": [
                [
                    [0.0, 0.0],
                    [10.0, 0.0],
                    [10.0, 10.0],
                    [0.0, 10.0],
                    [0.0, 0.0]
                ],
                [
                    [2.0, 2.0],
                    [8.0, 2.0],
                    [8.0, 8.0],
                    [2.0, 8.0],
                    [2.0, 2.0]
                ]
            ]
        });

        let (id, polygon) = value_to_polygon(&value).unwrap();

        assert_eq!(id, 42);

        assert_eq!(
            polygon.exterior(),
            &LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
        );

        assert_eq!(polygon.interiors().len(), 1);
        assert_eq!(
            polygon.interiors()[0],
            LineString::from(vec![
                (2.0, 2.0),
                (8.0, 2.0),
                (8.0, 8.0),
                (2.0, 8.0),
                (2.0, 2.0),
            ])
        );
    }

    #[test]
    fn value_to_polygon_parses_polygon_without_holes() {
        let value = json!({
            "id": 1,
            "type": "Polygon",
            "coordinates": [[
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.0, 0.0]
            ]]
        });

        let (id, polygon) = value_to_polygon(&value).unwrap();

        assert_eq!(id, 1);
        assert!(polygon.interiors().is_empty());
        assert_eq!(polygon.exterior().0.len(), 5);
    }

    #[test]
    fn value_to_polygon_returns_invalid_type_for_non_object() {
        let value = json!("polygon");

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::InvalidType)
        ));
    }

    #[test]
    fn value_to_polygon_returns_missing_id() {
        let value = json!({
            "type": "Polygon",
            "coordinates": []
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::MissingField("id"))
        ));
    }

    #[test]
    fn value_to_polygon_returns_missing_type() {
        let value = json!({
            "id": 1,
            "coordinates": []
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::MissingField("type"))
        ));
    }

    #[test]
    fn value_to_polygon_rejects_non_polygon_type() {
        let value = json!({
            "id": 1,
            "type": "Point",
            "coordinates": []
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::InvalidType)
        ));
    }

    #[test]
    fn value_to_polygon_returns_missing_coordinates() {
        let value = json!({
            "id": 1,
            "type": "Polygon"
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::MissingField("coordinates"))
        ));
    }

    #[test]
    fn value_to_polygon_rejects_empty_coordinates() {
        let value = json!({
            "id": 1,
            "type": "Polygon",
            "coordinates": []
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::InvalidCoordinates)
        ));
    }

    #[test]
    fn value_to_polygon_rejects_invalid_ring() {
        let value = json!({
            "id": 1,
            "type": "Polygon",
            "coordinates": [
                "invalid ring"
            ]
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::InvalidCoordinates)
        ));
    }

    #[test]
    fn value_to_polygon_rejects_position_with_less_than_two_values() {
        let value = json!({
            "id": 1,
            "type": "Polygon",
            "coordinates": [[
                [0.0],
                [1.0, 1.0]
            ]]
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::InvalidPosition)
        ));
    }

    #[test]
    fn value_to_polygon_rejects_non_numeric_position() {
        let value = json!({
            "id": 1,
            "type": "Polygon",
            "coordinates": [[
                ["invalid", 0.0],
                [1.0, 1.0]
            ]]
        });

        assert!(matches!(
            value_to_polygon(&value),
            Err(PolygonError::InvalidPosition)
        ));
    }

    #[test]
    fn value_to_polygon_accepts_position_with_more_than_two_values() {
        let value = json!({
            "id": 1,
            "type": "Polygon",
            "coordinates": [[
                [0.0, 0.0, 100.0],
                [1.0, 0.0, 100.0],
                [1.0, 1.0, 100.0],
                [0.0, 0.0, 100.0]
            ]]
        });

        let (_, polygon) = value_to_polygon(&value).unwrap();

        assert_eq!(polygon.exterior().0[0].x, 0.0);
        assert_eq!(polygon.exterior().0[0].y, 0.0);
        assert_eq!(polygon.exterior().0.len(), 4);
    }

    #[test]
    fn value_to_polygon_rejects_id_over_u64() {
        let value = json!({
            "id": 4294967295u64,
            "type": "Polygon",
            "coordinates": [[
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0]
            ]]
        });

        let result = value_to_polygon(&value);

        assert!(result.is_ok());
    }
}
