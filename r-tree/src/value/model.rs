use arc_swap::ArcSwap;
use geo::{BoundingRect, Contains, Intersects, Point, Polygon};
use rstar::{AABB, RTree, RTreeObject};
use sonic_rs::Value;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

// PolygonKey
//
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PolygonKey {
    pub setting_code: Arc<str>,
    pub key: Arc<str>,
}

impl PolygonKey {
    pub fn new(setting_code: impl Into<Arc<str>>, key: impl Into<Arc<str>>) -> Self {
        Self {
            setting_code: setting_code.into(),
            key: key.into(),
        }
    }
}

// PolygonObject
//
#[derive(Debug, Clone)]
pub struct PolygonObject {
    pub id: i64,
    pub polygon: Polygon<f64>,
    pub value: Arc<Value>,
}

// RTree
//
#[derive(Debug, Clone)]
pub struct IndexedPolygon {
    id: i64,
    polygon: Polygon<f64>,
    envelope: AABB<[f64; 2]>,
}

impl IndexedPolygon {
    fn new(object: &PolygonObject) -> Self {
        let rect = object
            .polygon
            .bounding_rect()
            .expect("polygon must have bounding rectangle");

        Self {
            id: object.id,
            polygon: object.polygon.clone(),
            envelope: AABB::from_corners(
                [rect.min().x, rect.min().y],
                [rect.max().x, rect.max().y],
            ),
        }
    }
}

impl RTreeObject for IndexedPolygon {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

// PolygonIndex
//
#[derive(Debug)]
pub struct PolygonIndex {
    tree: RTree<IndexedPolygon>,
}

impl PolygonIndex {
    fn empty() -> Self {
        Self { tree: RTree::new() }
    }

    fn build(objects: impl IntoIterator<Item = PolygonObject>) -> Self {
        let items = objects
            .into_iter()
            .map(|object| IndexedPolygon::new(&object))
            .collect();

        Self {
            tree: RTree::bulk_load(items),
        }
    }

    pub fn find_intersections_point(&self, point: &Point<f64>) -> Vec<i64> {
        let envelope = AABB::from_point([point.x(), point.y()]);

        self.tree
            .locate_in_envelope_intersecting(envelope)
            .filter(|candidate| candidate.polygon.contains(point))
            .map(|candidate| candidate.id)
            .collect()
    }

    pub fn find_intersections_polygon(&self, polygon: &Polygon<f64>) -> Vec<i64> {
        let Some(rect) = polygon.bounding_rect() else {
            return Vec::new();
        };

        let envelope =
            AABB::from_corners([rect.min().x, rect.min().y], [rect.max().x, rect.max().y]);

        self.tree
            .locate_in_envelope_intersecting(envelope)
            .filter(|candidate| candidate.polygon.intersects(polygon))
            .map(|candidate| candidate.id)
            .collect()
    }
}

// Bucket
//
#[derive(Debug)]
pub struct Bucket {
    pub objects: RwLock<HashMap<i64, PolygonObject>>,
    pub index: ArcSwap<PolygonIndex>,
}

impl Bucket {
    pub fn new() -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
            index: ArcSwap::from_pointee(PolygonIndex::empty()),
        }
    }

    pub fn rebuild(&self) {
        let objects = {
            let objects = self.objects.read().unwrap();

            objects.values().cloned().collect::<Vec<_>>()
        };

        let index = PolygonIndex::build(objects);

        self.index.store(Arc::new(index));
    }
}
