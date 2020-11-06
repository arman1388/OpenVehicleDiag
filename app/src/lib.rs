#[macro_use]
extern crate napi;
#[macro_use]
extern crate napi_derive;

use napi::{CallContext, Result, JsString, Status, Error, JsUnknown, JsFunction, JsUndefined, Module, JsNumber};

use serde_json;
use serde::*;
use std::ffi::*;
use std::convert::{TryInto, TryFrom};

use J2534Common::*;
mod passthru;
use passthru::*;

#[derive(Debug, Serialize, Deserialize)]
struct LoadErr {
  err: String
}

#[derive(Debug, Serialize, Deserialize)]
struct Device {
  dev_id: u32
}

#[derive(Debug, Serialize, Deserialize)]
struct Voltage {
  mv: u32
}

#[js_function]
pub fn get_device_list(mut ctx: CallContext) -> Result<JsUnknown> {
  Ok(match passthru::PassthruDevice::find_all() {
    Ok(dev) => ctx.env.to_js_value(&dev)?,
    Err(e) =>  ctx.env.to_js_value(&e)?,
  })
}

#[js_function(1)]
pub fn connect_device(mut ctx: CallContext) -> Result<JsUnknown> {
  let v = ctx.get::<JsUnknown>(0)?;
  let deser: PassthruDevice = ctx.env.from_js_value(v)?;

  if passthru::DRIVER.read().unwrap().is_some() {
    return ctx.env.to_js_value(&LoadErr{ err: "Driver in use!".to_string() });
  }

  match PassthruDrv::load_lib(deser.drv_path) {
    Ok (d) => passthru::DRIVER.write().unwrap().replace(d),
    Err(x) => return ctx.env.to_js_value(&LoadErr{ err: x.to_string() })
  };
  match &passthru::DRIVER.write().unwrap().as_ref().unwrap().open() {
    Ok(idx) => ctx.env.to_js_value(&Device{ dev_id: *idx }),
    Err(_) => ctx.env.to_js_value(&LoadErr{ err: "ERR_FAILED".to_string() })
  }
}

#[js_function(1)]
pub fn get_vbatt(mut ctx: CallContext) -> Result<JsUnknown> {
  let idx: u32 = u32::try_from(ctx.get::<JsNumber>(0)?)?;
  if passthru::DRIVER.read().unwrap().is_none() {
    return ctx.env.to_js_value(&LoadErr{ err: "No driver!".to_string() });
  }


  /*
  let input_arr: *mut c_void = &mut input as *mut _ as *mut c_void;
  let output_arr: *mut c_void = &mut output as *mut _ as *mut c_void;
  */
  let mut voltage = 0;

  match &passthru::DRIVER.write().unwrap().as_ref().unwrap().ioctl(idx, IoctlID::READ_VBATT, std::ptr::null_mut::<c_void>(), (&mut voltage) as *mut _ as *mut c_void) {
    0x00 => ctx.env.to_js_value(&Voltage{mv: voltage}),
    n => ctx.env.to_js_value(&LoadErr{ err: format!("Error code {}!", n) })
  }
}


register_module!(ovd, init);

fn init(module: &mut Module) -> Result<()> {
  module.create_named_method("get_device_list", get_device_list)?;
  module.create_named_method("connect_device", connect_device)?;
  module.create_named_method("get_vbatt", get_vbatt)?;
  Ok(())
}