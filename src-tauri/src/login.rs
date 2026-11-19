use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Unknown,
    Disabled,
    Enabled,
    NeedsApproval,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Sin confirmar",
            Self::Disabled => "Desactivado",
            Self::Enabled => "Activado",
            Self::NeedsApproval => "Requiere autorización en macOS",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Query,
    Set(bool),
}
pub struct Backend {
    #[cfg(feature = "test-profile")]
    simulated: State,
}
impl Backend {
    pub fn new(previous: State) -> Self {
        #[cfg(not(feature = "test-profile"))]
        let _ = previous;
        Self {
            #[cfg(feature = "test-profile")]
            simulated: if previous == State::Enabled {
                previous
            } else {
                State::Disabled
            },
        }
    }
    pub fn run(&mut self, action: Action) -> (State, Option<String>) {
        #[cfg(feature = "test-profile")]
        {
            if let Action::Set(v) = action {
                self.simulated = if v { State::Enabled } else { State::Disabled };
            }
            (self.simulated, None)
        }
        #[cfg(not(feature = "test-profile"))]
        {
            use std::ffi::CStr;
            extern "C" {
                fn sw_login(op: i32, message: *mut std::ffi::c_char, size: usize) -> i32;
            }
            let op = match action {
                Action::Query => 0,
                Action::Set(true) => 1,
                Action::Set(false) => 2,
            };
            let mut buf = [0i8; 512];
            let state = match unsafe { sw_login(op, buf.as_mut_ptr(), buf.len()) } {
                0 => State::Disabled,
                1 => State::Enabled,
                2 => State::NeedsApproval,
                _ => State::Unknown,
            };
            let msg = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy();
            (state, (!msg.is_empty()).then(|| msg.into_owned()))
        }
    }
}
