use std::sync::Arc;

use r_setting::streams::stream_setting::StreamSetting;

pub(crate) enum RouteSource {
    Streams(Arc<Vec<StreamSetting>>),
    Static,
}

impl RouteSource {
        pub(crate) fn routes<'a>(
        &'a self,
        fallback: &'a [String],
    ) -> Box<dyn Iterator<Item = (&'a str, Option<&'a str>)> + 'a> {
        match self {
            RouteSource::Streams(s) => Box::new(
                s.iter()
                    .filter(|s| s.is_active())
                    .filter_map(|s| s.channel().map(|c| (c, s.setting_code_stream()))),
            ),
            RouteSource::Static => Box::new(fallback.iter().map(|t| (t.as_str(), None))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(json: &str) -> StreamSetting {
        sonic_rs::from_str(json).expect("valid StreamSetting json")
    }

    #[test]
    fn streams_all_inactive_yields_nothing() {
        let settings = Arc::new(vec![
            setting(r#"{"isActive": false, "channel": "a"}"#),
            setting(r#"{"channel": "b"}"#), 
        ]);
        let route = RouteSource::Streams(settings);
        let fallback = vec!["topic".to_string()];

        let routes: Vec<(&str, Option<&str>)> = route.routes(&fallback).collect();
        assert!(routes.is_empty());
    }

    #[test]
    fn routes_pair_active_channels_with_setting_code_stream() {
        let settings = Arc::new(vec![
            setting(r#"{"isActive": true, "channel": "a", "settingCodeStream": "sc-a"}"#),
            setting(r#"{"isActive": false, "channel": "b", "settingCodeStream": "sc-b"}"#), 
            setting(r#"{"isActive": true, "channel": "c"}"#), 
        ]);
        let route = RouteSource::Streams(settings);
        let fallback = vec!["topic".to_string()];

        let routes: Vec<(&str, Option<&str>)> = route.routes(&fallback).collect();
        assert_eq!(routes, [("a", Some("sc-a")), ("c", None)]);
    }

    #[test]
    fn static_routes_carry_no_setting_code_stream() {
        let route = RouteSource::Static;
        let fallback = vec!["t1".to_string(), "t2".to_string()];

        let routes: Vec<(&str, Option<&str>)> = route.routes(&fallback).collect();
        assert_eq!(routes, [("t1", None), ("t2", None)]);
    }
}
