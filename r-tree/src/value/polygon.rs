use std::sync::Arc;

use dashmap::DashMap;
use sonic_rs::Value;
use crate::value::convert::{value_to_polygon, PolygonError, value_to_point};
use crate::value::model::{Bucket, PolygonKey, PolygonObject};

#[derive(Debug, Clone, Default)]
pub struct Values {
    shared: Arc<Shared>,
}

#[derive(Debug, Default)]
struct Shared {
    buckets: DashMap<PolygonKey, Arc<Bucket>>,
    values: DashMap<i64, Arc<Value>>,
}

impl Values {
    pub fn new() -> Self {
        Self::default()
    }

    fn bucket(&self, key: &PolygonKey) -> Arc<Bucket> {
        self.shared
            .buckets
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Bucket::new()))
            .clone()
    }

    pub fn put_polygon(
        &self,
        setting_code: &str,
        key: Arc<str>,
        value: Value,
    ) -> bool {
        let key = PolygonKey::new(Arc::<str>::from(setting_code), key);
        let Ok((id, polygon)) = value_to_polygon(&value) else {
            return false;
        };

        let bucket = self.bucket(&key);

        let object = PolygonObject {
            id,
            polygon,
            value: Arc::new(value),
        };

        {
            let mut objects = bucket.objects.write().unwrap();

            objects.insert(id, object.clone());
        }

        self.shared.values.insert(id, Arc::clone(&object.value));

        bucket.rebuild();

        true
    }

    pub fn remove_polygon(&self, setting_code: &str, key: Arc<str>, value: Value) -> bool {
        let polygon_key = PolygonKey::new(Arc::<str>::from(setting_code), key);

        let Ok((id, polygon)) = value_to_polygon(&value) else {
            return false;
        };

        let Some(bucket) = self.shared.buckets.get(&polygon_key) else {
            return false;
        };

        let removed = {
            let mut objects = bucket.objects.write().unwrap();

            objects.remove(&id).is_some()
        };

        if !removed {
            return false;
        }

        self.shared.values.remove(&id);

        bucket.rebuild();

        true
    }

    pub fn contains_polygon(
        &self,
        setting_code: &str,
        key: &str,
        value: &Value,
    ) -> Option<Vec<Arc<Value>>> {
        let polygon_key =
            PolygonKey::new(Arc::<str>::from(setting_code), Arc::<str>::from(key));

        let Ok((id, point)) = value_to_polygon(value) else {
            return None;
        };

        let bucket = self.shared.buckets.get(&polygon_key)?;

        let index = bucket.index.load();
        let ids = index.find_intersections_polygon(&point);

        let result = ids
            .into_iter()
            .filter(|other_id| *other_id != id)
            .filter_map(|id| {
                self.shared
                    .values
                    .get(&id)
                    .map(|value| Arc::clone(value.value()))
            })
            .collect();

        Some(result)
    }

    pub fn contains_point(
        &self,
        setting_code: &str,
        key: &str,
        value: &Value,
    ) -> Option<Vec<Arc<Value>>> {
        let polygon_key =
            PolygonKey::new(Arc::<str>::from(setting_code), Arc::<str>::from(key));

        let Ok((id, point)) = value_to_point(value) else {
            return None;
        };

        let bucket = self.shared.buckets.get(&polygon_key)?;

        let index = bucket.index.load();
        let ids = index.find_intersections_point(&point);

        let result = ids
            .into_iter()
            .filter(|other_id| *other_id != id)
            .filter_map(|id| {
                self.shared
                    .values
                    .get(&id)
                    .map(|value| Arc::clone(value.value()))
            })
            .collect();

        Some(result)
    }

    pub fn entries(&self) -> Vec<(PolygonKey, Vec<Arc<Value>>)> {
        self.shared
            .buckets
            .iter()
            .map(|entry| {
                let key = entry.key().clone();
                let bucket = Arc::clone(entry.value());

                let values = bucket
                    .objects
                    .read()
                    .unwrap()
                    .values()
                    .map(|object| Arc::clone(&object.value))
                    .collect();

                (key, values)
            })
            .collect()
    }
}
