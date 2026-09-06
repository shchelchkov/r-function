use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use r_producer::host::send_pipeline::SendValue;
use r_producer::kafka::producer::Producer;
use r_setting::functions::function_setting::FunctionSetting;
use r_setting::functions::functions::Function;
use r_setting::functions::functions_value::FunctionValue;
use r_setting::streams::stream::Stream;
use r_tree::value::polygon::Polygon;
use r_value::value::value::Values;

pub mod plugin_module;
pub mod plugin_repository;
pub mod plugin;
pub mod loader;
pub mod executor;


pub struct PluginContext {
    pub function: Function,
    pub function_value: FunctionValue,
    pub stream: Stream,
    pub values: Values,
    pub producer: Producer,
    pub polygon: Polygon,
    pub send_value: SendValue,
}