#![feature(extern_types)]
#![recursion_limit = "128"]

/// serializes a type as a different repr type using the given conversion functions
#[macro_export]
macro_rules! serde_conv {
    ($m:ident, $t:ty, /*$ser:expr,*/ $de:expr) => {
        pub mod $m {
            use super::*;
            use ::serde::{/*Serialize, Serializer,*/ Deserialize, Deserializer};
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
    };
}

macro_rules! cb_to_js_fn {
    ($cb:ident) => {
        Closure::wrap(Box::new(move |v: JsValue| $cb.emit(v.into())) as Box<dyn Fn(JsValue)>)
    };
}

serde_conv!(serde_pos_err_code, PositionErrorCode, |x: u16| match x {
    1 => PositionErrorCode::PermissionDenied,
    2 => PositionErrorCode::PositionUnavailable,
    3 => PositionErrorCode::Timeout,
    4 => PositionErrorCode::FailedToDeserialize,
    5 => PositionErrorCode::NoBrowserSupport,
    _ => unreachable!(),
});

use measurements::{Angle, Length, Speed};
use serde_derive::*;
use smart_default::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::Geolocation;
use yew::prelude::*;

// https://w3c.github.io/geolocation-api/#idl-index

pub type DOMTimeStamp = u64;

#[derive(Debug, Copy, Clone)]
pub struct Position {
    pub coords: Option<Coordinates>,
    pub timestamp: Option<DOMTimeStamp>,
}

impl From<JsValue> for Position {
    fn from(js_val: JsValue) -> Self {
        let geo_pos = GeolocationPosition::from(js_val);

        Position {
            coords: geo_pos.coords().map(|coords| Coordinates {
                latitude: coords.latitude(),
                longitude: coords.longitude(),
                accuracy: coords.accuracy().map(Length::from_meters),
                altitude: coords.altitude().map(Length::from_meters),
                altitude_accuracy: coords.altitude_accuracy().map(Length::from_meters),
                heading: coords.heading().map(Angle::from_degrees),
                speed: coords.speed().map(Speed::from_meters_per_second),
            }),

            timestamp: geo_pos.timestamp(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Coordinates {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<Length>,
    pub accuracy: Option<Length>,
    pub altitude_accuracy: Option<Length>,
    pub heading: Option<Angle>,
    pub speed: Option<Speed>,
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

impl From<PositionOptions> for web_sys::PositionOptions {
    fn from(opts: PositionOptions) -> Self {
        let mut ret = web_sys::PositionOptions::new();

        ret.enable_high_accuracy(opts.enable_high_accuracy)
            .maximum_age(opts.maximum_age)
            .timeout(opts.timeout_ms);

        ret
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionError {
    #[serde(with = "serde_pos_err_code")]
    pub code: PositionErrorCode,
    pub message: String,
}

impl From<JsValue> for PositionError {
    fn from(js_val: JsValue) -> Self {
        js_val.into_serde().unwrap_or_else(|err| PositionError {
            code: PositionErrorCode::FailedToDeserialize,
            message: format!("{err:?}"),
        })
    }
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PositionErrorCode {
    PermissionDenied = 1,
    PositionUnavailable = 2,
    Timeout = 3,
    FailedToDeserialize = 4,
    NoBrowserSupport = 5,
}

#[derive(Default)]
pub struct GeolocationService {}

#[wasm_bindgen]
extern "C" {
    #[derive(Debug)]
    type GeolocationCoordinates;

    #[wasm_bindgen(method, getter)]
    fn latitude(this: &GeolocationCoordinates) -> Option<f64>;

    #[wasm_bindgen(method, getter)]
    fn longitude(this: &GeolocationCoordinates) -> Option<f64>;

    #[wasm_bindgen(method, getter)]
    fn altitude(this: &GeolocationCoordinates) -> Option<f64>;

    #[wasm_bindgen(method, getter)]
    fn accuracy(this: &GeolocationCoordinates) -> Option<f64>;

    #[wasm_bindgen(method, getter, js_name = altitudeAccuracy)]
    fn altitude_accuracy(this: &GeolocationCoordinates) -> Option<f64>;

    #[wasm_bindgen(method, getter)]
    fn heading(this: &GeolocationCoordinates) -> Option<f64>;

    #[wasm_bindgen(method, getter)]
    fn speed(this: &GeolocationCoordinates) -> Option<f64>;

    #[derive(Debug)]
    pub type GeolocationPosition;

    #[wasm_bindgen(method, getter)]
    fn coords(this: &GeolocationPosition) -> Option<GeolocationCoordinates>;

    #[wasm_bindgen(method, getter)]
    fn timestamp(this: &GeolocationPosition) -> Option<DOMTimeStamp>;
}

impl GeolocationService {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_geolocation() -> Option<Geolocation> {
        web_sys::window().and_then(|w| w.navigator().geolocation().ok())
    }

    pub fn get_current_position(
        &self,
        success_cb: Callback<Position>,
        error_cb: Option<Callback<PositionError>>,
        options: Option<PositionOptions>,
    ) {
        match GeolocationService::get_geolocation() {
            None => {
                match error_cb {
                    None => {}
                    Some(cb) => cb.emit(PositionError {
                        code: PositionErrorCode::NoBrowserSupport,
                        message: "Could not get a handle on 'window.navigator.geolocation'".into(),
                    }),
                };
            }
            Some(geolocation) => {
                let on_success = cb_to_js_fn!(success_cb);
                match error_cb {
                    None => {
                        // don't care to handle this error since caller didn't give us an `error_cb`
                        let _ =
                            geolocation.get_current_position(&on_success.as_ref().unchecked_ref());
                    }
                    Some(err_cb) => {
                        let on_fail = cb_to_js_fn!(err_cb);
                        let opts = options.unwrap_or_default();

                        // don't know why we would get an error here when we are supplying an error callback
                        let _ = geolocation.get_current_position_with_error_callback_and_options(
                            &on_success.as_ref().unchecked_ref(),
                            Some(&on_fail.as_ref().unchecked_ref()),
                            &web_sys::PositionOptions::from(opts),
                        );

                        //match options {
                        //    None => {
                        //        // don't know why we would get an error here when we are supplying an error callback
                        //        let _ = geolocation.get_current_position_with_error_callback(
                        //            &on_success.as_ref().unchecked_ref(),
                        //            Some(&on_fail.as_ref().unchecked_ref()),
                        //        );
                        //    }
                        //    Some(opts) => {
                        //        // don't know why we would get an error here when we are supplying an error callback
                        //        let _ = geolocation
                        //            .get_current_position_with_error_callback_and_options(
                        //                &on_success.as_ref().unchecked_ref(),
                        //                Some(&on_fail.as_ref().unchecked_ref()),
                        //                &web_sys::PositionOptions::from(opts),
                        //            );
                        //    }
                        //}

                        on_fail.forget();
                    }
                }
                on_success.forget();
            }
        }
    }

    pub fn watch_position(
        &mut self,
        success_cb: Callback<Position>,
        error_cb: Option<Callback<PositionError>>,
        options: Option<PositionOptions>,
    ) -> Option<WatchPositionTask> {
        GeolocationService::get_geolocation()
            .and_then(|geolocation| {
                let when_success = cb_to_js_fn!(success_cb);

                let watch_id: Option<i32> = match error_cb {
                    None => geolocation
                        .watch_position(&when_success.as_ref().unchecked_ref())
                        .ok(),
                    Some(err_cb) => {
                        let when_fail = cb_to_js_fn!(err_cb);
                        let opts = options.unwrap_or_default();

                        let result = geolocation
                            .watch_position_with_error_callback_and_options(
                                &when_success.as_ref().unchecked_ref(),
                                Some(&when_fail.as_ref().unchecked_ref()),
                                &web_sys::PositionOptions::from(opts),
                            )
                            .ok();

                        when_fail.forget();
                        result
                    }
                };

                when_success.forget();

                watch_id
            })
            .map(|watch_id| watch_id.into())
    }
}

/// A handle on a geolocation watch_position task, which will be cleared on drop
pub struct WatchPositionTask {
    watch_id: i32,
    active: bool,
}

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
        self.active
    }

    fn cancel(&mut self) {
        for geolocation in GeolocationService::get_geolocation().iter() {
            geolocation.clear_watch(self.watch_id);
            self.active = false;
        }
    }
}

impl From<i32> for WatchPositionTask {
    fn from(watch_id: i32) -> Self {
        WatchPositionTask {
            watch_id,
            active: true,
        }
    }
}

impl Drop for WatchPositionTask {
    fn drop(&mut self) {
        if self.is_active() {
            self.cancel();
        }
    }
}
