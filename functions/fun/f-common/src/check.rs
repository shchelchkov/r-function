use sonic_rs::{Object, Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Ok,
    Warn,
    Error,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Ok => "ok",
            State::Warn => "warn",
            State::Error => "error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Step {
    pub name: &'static str,
    pub state: State,
    pub sent: Option<u64>,
    pub errors: Vec<&'static str>,
}

impl Step {
    pub fn ok(name: &'static str) -> Self {
        Self {
            name,
            state: State::Ok,
            sent: None,
            errors: Vec::new(),
        }
    }

    pub fn sent(mut self, n: u64) -> Self {
        self.sent = Some(n);
        self
    }

    pub fn error(mut self, code: &'static str) -> Self {
        self.errors.push(code);
        self.state = State::Error;
        self
    }

    pub fn warn(mut self, code: &'static str) -> Self {
        self.errors.push(code);
        if self.state < State::Warn {
            self.state = State::Warn;
        }
        self
    }

    pub fn inc_sent(mut self) -> Self {
        self.sent = Some(self.sent.unwrap_or(0) + 1);
        self
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.sent = match (self.sent, other.sent) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        self.errors.extend(other.errors);
        if other.state > self.state {
            self.state = other.state;
        }
        self
    }

    fn to_value(&self) -> Value {
        let mut obj = Object::new();
        obj.insert("fn", self.name);
        obj.insert("state", self.state.as_str());
        if let Some(sent) = self.sent {
            obj.insert("sent", sent);
        }
        if !self.errors.is_empty() {
            obj.insert("errors", json!(self.errors));
        }
        obj.into_value()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CheckFunction {
    pub steps: Vec<Step>,
}

impl CheckFunction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn state(&self) -> State {
        self.steps
            .iter()
            .map(|s| s.state)
            .max()
            .unwrap_or(State::Ok)
    }

    pub fn into_value(self) -> Value {
        let state = self.state();
        let steps: Vec<Value> = self.steps.iter().map(Step::to_value).collect();
        json!({ "state": state.as_str(), "steps": steps })
    }
}
