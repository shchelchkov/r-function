use axum::extract::FromRef;
use r_feed::FeedHub;
use r_setting::catalogs::catalog::Catalog;
use r_setting::consumers::consumer::Consumer;
use r_setting::functions::functions::Function;
use r_setting::functions::functions_value::FunctionValue;
use r_setting::git::GitHandle;
use r_setting::streams::stream::Stream;
use r_value::value::value::Values;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Clone)]
pub struct AppState {
    pub values: Values,
    pub git: Arc<GitHandle>,
    pub function: Function,
    pub function_value: FunctionValue,
    pub catalog: Catalog,
    pub consumer: Consumer,
    pub stream: Stream,
    pub feed: FeedHub,
    pub shutdown: watch::Receiver<bool>,
}

impl FromRef<AppState> for Values {
    fn from_ref(state: &AppState) -> Self {
        state.values.clone()
    }
}

impl FromRef<AppState> for Arc<GitHandle> {
    fn from_ref(state: &AppState) -> Self {
        state.git.clone()
    }
}

impl FromRef<AppState> for Function {
    fn from_ref(state: &AppState) -> Self {
        state.function.clone()
    }
}

impl FromRef<AppState> for FunctionValue {
    fn from_ref(state: &AppState) -> Self {
        state.function_value.clone()
    }
}

impl FromRef<AppState> for Catalog {
    fn from_ref(state: &AppState) -> Self {
        state.catalog.clone()
    }
}

impl FromRef<AppState> for Consumer {
    fn from_ref(state: &AppState) -> Self {
        state.consumer.clone()
    }
}

impl FromRef<AppState> for Stream {
    fn from_ref(state: &AppState) -> Self {
        state.stream.clone()
    }
}

impl FromRef<AppState> for FeedHub {
    fn from_ref(state: &AppState) -> Self {
        state.feed.clone()
    }
}

#[derive(Clone)]
pub struct WsState {
    pub feed: FeedHub,
    pub shutdown: watch::Receiver<bool>,
}

impl FromRef<AppState> for WsState {
    fn from_ref(state: &AppState) -> Self {
        WsState {
            feed: state.feed.clone(),
            shutdown: state.shutdown.clone(),
        }
    }
}
