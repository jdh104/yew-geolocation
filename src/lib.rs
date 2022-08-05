
#![recursion_limit = "128"]

/// serializes a type as a different repr type using the given conversion functions
#[macro_export]
macro_rules! serde_conv {
	($m:ident, $t:ty, /*$ser:expr,*/ $de:expr) => {
		pub mod $m {
			use ::serde::{/*Serialize, Serializer,*/ Deserialize, Deserializer};
			use super::*;
/*
			pub fn serialize<S: Serializer>(x: &$t, serializer: S) -> Result<S::Ok, S::Error> {
				let y = $ser(*x);
				y.serialize(serializer)
			}
*/
			pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<$t, D::Error> {
				let y = Deserialize::deserialize(deserializer)?;
				Ok($de(y))
			}
		}
	}
}

serde_conv!(serde_pos_err_code, PositionErrorCode, |x: u16| match x {
	1 => PositionErrorCode::PermissionDenied,
	2 => PositionErrorCode::PositionUnavailable,
	3 => PositionErrorCode::Timeout,
	_ => unreachable!()
});

use yew::prelude::*;
use serde_derive::*;
use smart_default::*;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use web_sys::{error, Geolocation};

// https://w3c.github.io/geolocation-api/#idl-index

pub type DOMTimeStamp = u64;

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Position {
    pub coords: Coordinates,
    pub timestamp: DOMTimeStamp,
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: f64,
    #[serde(rename = "altitudeAccuracy")]
    pub altitude_accuracy: Option<f64>,
    pub heading: Option<f64>,
    pub speed: Option<f64>,
}

#[derive(Debug, Copy, Clone, Serialize, SmartDefault)]
pub struct PositionOptions {
    #[serde(rename = "enableHighAccuracy")]
    pub enable_high_accuracy: bool,
    #[default = "0xFFFFFFFF"]
    #[serde(rename = "timeout")]
    pub timeout_ms: u32,
    #[serde(rename = "maximumAge")]
    pub maximum_age: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionError {
    #[serde(with = "serde_pos_err_code")]
    pub code: PositionErrorCode,
    pub message: String,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PositionErrorCode {
    PermissionDenied = 1,
    PositionUnavailable = 2,
    Timeout = 3,
}

#[derive(Default)]
pub struct GeolocationService {}

impl GeolocationService {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_geolocation(&self) -> Option<Geolocation> {
        web_sys::window().and_then(|w| w.navigator().geolocation().ok())
    }

    pub fn get_current_position(&self,
                                success_cb: Callback<Position>,
                                error_cb: Option<Callback<PositionError>>,
                                options: Option<PositionOptions>)
    {
        let js_error_cb: Option<js_sys::Function> = error_cb.map(|cb| cb.into());

        match self.get_geolocation() {
            None => error_cb.unwrap_or_default().emit(PositionError{
                code: PositionErrorCode::PermissionDenied,
                message: "Could not get a handle on 'window.navigator.geolocation'".into(),
            }),
            Some(geolocation) => match options {
                None => geolocation.get_current_position_with_error_callback(success_cb.into(), js_error_cb),
                Some(opts) => geolocation.get_current_position_with_error_callback_and_options(success_cb.into(), js_error_cb, opts.into()),
            }


                //geolocation.get_current_position_with_error_callback_and_options(success_cb.into())
        }
    }

    pub fn watch_position(&mut self,
                          success_cb: Callback<Position>,
                          error_cb: Option<Callback<PositionError>>,
                          options: Option<PositionOptions>) -> WatchPositionTask
    {

    }
}

pub struct WatchPositionTask(Option<JsValue>);

// pulled straight from old (< v0.19) yew::services module
/// An universal task of a service.
/// It have to be canceled when dropped.
pub trait Task {
    /// Returns `true` if task is active.
    fn is_active(&self) -> bool;

    /// Cancel current service's routine.
    fn cancel(&mut self);
}

impl Task for WatchPositionTask {
    fn is_active(&self) -> bool {
        self.0.is_some()
    }

    fn cancel(&mut self) {

    }
}

impl Drop for WatchPositionTask {
    fn drop(&mut self) {
        if self.is_active() {
            self.cancel();
        }
    }
}
