
pub extern crate videocore;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;

pub mod display;

use std::sync::{Mutex, Arc};
use std::marker::PhantomData;

use videocore::bcm_host;

lazy_static! {
    static ref INIT_FLAG: Mutex<bool> = Mutex::new(false);
}

#[derive(Debug)]
/// Initialization error
pub enum BCMHostInitError {
    AlreadyInitialized,
}

#[derive(Debug)]
struct BCMHost {
    _marker: PhantomData<()>,
}

impl BCMHost {
    fn new() -> Self {
        BCMHost {
            _marker: PhantomData
        }
    }
}

impl Drop for BCMHost {
    fn drop(&mut self) {
        bcm_host::deinit();
    }
}

#[derive(Debug, Clone)]
pub struct BCMHostHandle {
    handle: Arc<BCMHost>,
}

impl BCMHostHandle {
    pub fn init() -> Result<Self, BCMHostInitError> {
        let mut init_flag_guard = INIT_FLAG.lock().unwrap();

        if *init_flag_guard {
            Err(BCMHostInitError::AlreadyInitialized)
        } else {
            bcm_host::init();

            *init_flag_guard = true;

            Ok(BCMHostHandle {
                handle: Arc::new(BCMHost::new())
            })
        }
    }

    pub fn peripheral_address(&self) -> u32 {
        bcm_host::get_peripheral_address()
    }

    pub fn peripheral_size(&self) -> u32 {
        bcm_host::get_peripheral_size()
    }

    pub fn sdram_address(&self) -> u32 {
        bcm_host::get_sdram_address()
    }

    // TODO: Limit display creation?
    //       Calling display creation function multiple times
    //       with same display id may return same display.

    pub fn dispmanx_display(&self, display_id: display::DisplayID) -> display::Display {
        display::Display::new(self, display_id)
    }

    pub fn dispmanx_update_builder(&self, priority: i32) -> Result<display::UpdateBuilder, ()> {
        display::UpdateBuilder::new(self, priority)
    }

    /*
    pub fn graphics_display_size(&self, display_number: u16) -> Option<bcm_host::GraphicsDisplaySize> {
        bcm_host::graphics_get_display_size(display_number)
    }
    */
}

