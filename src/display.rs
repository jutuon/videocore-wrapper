
use std::marker::PhantomData;
use std::sync::Arc;
use std::ptr;
use std::fmt;

use videocore::dispmanx;
use videocore::dispmanx::{Modeinfo, Transform};
use videocore::display::VCOSInputFormat;
use videocore::image;

use BCMHostHandle;

#[derive(Debug)]
pub struct DisplayHandle {
    bcm_host_handle: BCMHostHandle,
    raw_display: dispmanx::DisplayHandle,
    _marker: PhantomData<dispmanx::DisplayHandle>,
}

impl Drop for DisplayHandle {
    /// Calls `dispmanx::display_close`
    fn drop(&mut self) {
        // TODO: try to print to stderr if there was error
        dispmanx::display_close(self.raw_display);
    }
}

/// Dispmanx display
#[derive(Debug)]
pub struct Display {
    display_handle: Arc<DisplayHandle>,
}

impl Display {
    pub(crate) fn new(bcm_host_handle: &BCMHostHandle, display_id: DisplayID) -> Display {
        let raw_display = dispmanx::display_open(display_id as u32);

        let display_handle = DisplayHandle {
            bcm_host_handle: bcm_host_handle.clone(),
            raw_display,
            _marker: PhantomData,
        };

        Self {
            display_handle: Arc::new(display_handle),
        }
    }

    pub fn info(&self) -> Result<Modeinfo, ()> {
        let mut mode_info = Modeinfo {
            width: 0,
            height: 0,
            transform: Transform::NO_ROTATE,
            input_format: VCOSInputFormat::INVALID,
        };

        if dispmanx::display_get_info(self.raw_display(), &mut mode_info) {
            Ok(mode_info)
        } else {
            Err(())
        }
    }

    pub fn raw_display(&self) -> dispmanx::DisplayHandle {
        self.display_handle.raw_display
    }

    pub fn display_handle(&self) -> &Arc<DisplayHandle> {
        &self.display_handle
    }
}




#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum DisplayID {
    MainLCD = dispmanx::DISPMANX_ID_MAIN_LCD,
    AuxLCD = dispmanx::DISPMANX_ID_AUX_LCD,
    HDMI = dispmanx::DISPMANX_ID_HDMI,
    SDTV = dispmanx::DISPMANX_ID_SDTV,
    ForceLCD = dispmanx::DISPMANX_ID_FORCE_LCD,
    ForceTV = dispmanx::DISPMANX_ID_FORCE_TV,
    ForceOther = dispmanx::DISPMANX_ID_FORCE_OTHER,
}

#[derive(Debug)]
pub struct UpdateBuilder {
    _bcm_host_handle: BCMHostHandle,
    update_handle: dispmanx::UpdateHandle,
}

impl UpdateBuilder {
    pub(crate) fn new(bcm_host_handle: &BCMHostHandle, priority: i32) -> Result<Self, ()>  {
        let update_handle = dispmanx::update_start(priority);

        if update_handle == 0 {
            Err(())
        } else {
            Ok(Self {
                _bcm_host_handle: bcm_host_handle.clone(),
                update_handle,
            })
        }
    }

    // TODO: element_add function src, alpha and clamp attributes

    pub fn element_add(
        self,
        display: &Display,
        layer: i32,
        dest_rect: &mut image::Rect,
        // src: Option<ResourceHandle>
        src_rect: &mut image::Rect,
        protection: Protection,
        transform: dispmanx::Transform,
    ) -> Result<Element, ()> {
        let element_handle = dispmanx::element_add(
            self.update_handle,
            display.raw_display(),
            layer,
            dest_rect,
            0, // src
            src_rect,
            protection as dispmanx::Protection,
            ptr::null_mut(), // alpha
            ptr::null_mut(), // clamp
            transform,
        );

        if element_handle == 0 {
            // TODO: Send update to prevent memory leak?
            return Err(());
        }

        if !dispmanx::update_submit_sync(self.update_handle) {
            Ok(Element {
                _display_handle: display.display_handle().clone(),
                element_handle,
                // TODO: Size of c_int and i32 are not equal always.
                width: dest_rect.width,
                height: dest_rect.height,
            })
        } else {
            Err(())
        }
    }
}

#[derive(Debug)]
pub struct Element {
    _display_handle: Arc<DisplayHandle>,
    element_handle: dispmanx::ElementHandle,
    width: i32,
    height: i32,
}

impl Element {
    /// Window with dest_rect width and height
    pub fn into_window(self) -> Window {
        let raw_window = dispmanx::Window {
            element: self.element_handle,
            width: self.width,
            height: self.height,
        };

        Window {
            element: self,
            raw_window
        }
    }
}

impl Drop for Element {
    /// Sends remove update message
    fn drop(&mut self) {
        // TODO: try to print to stderr if there was error
        // TODO: Change priority?



        let update_handle = dispmanx::update_start(0);

        if update_handle == 0 {
            // error
            return;
        }

        if dispmanx::element_remove(update_handle, self.element_handle) {
            // error
            // TODO: Send update to prevent memory leak?
            return;
        }

        if dispmanx::update_submit_sync(update_handle) {
            // error
            return;
        }
    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone)]
pub enum Protection {
    Max = dispmanx::DISPMANX_PROTECTION_MAX,
    None = dispmanx::DISPMANX_PROTECTION_NONE,
    HDCP = dispmanx::DISPMANX_PROTECTION_HDCP,
}

pub struct Window {
    element: Element,
    raw_window: dispmanx::Window,
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Window")
    }
}

impl Window {
    /// Pointer for EGL surface creation
    pub fn raw_window(&mut self) -> *mut dispmanx::Window {
        &mut self.raw_window
    }

    pub fn change_element_attributes(
        &mut self,
        layer: Option<i32>,
        opacity: Option<u8>,
        dest_rect: Option<&image::Rect>,
        src_rect: Option<&image::Rect>,
    ) -> Result<(), ()> {
        let mut flags = ElementChange::empty();

        let layer = if let Some(value) = layer {
            flags |= ElementChange::LAYER;
            value
        } else {
            0
        };

        let opacity = if let Some(value) = opacity {
            flags |= ElementChange::OPACITY;
            value
        } else {
            0
        };

        let dest_rect = if let Some(value) = dest_rect {
            flags |= ElementChange::DEST_RECT;
            value
        } else {
            ptr::null()
        };

        let src_rect = if let Some(value) = src_rect {
            flags |= ElementChange::SRC_RECT;
            value
        } else {
            ptr::null()
        };

        if flags.bits() == 0 {
            return Ok(());
        }

        let update_handle = dispmanx::update_start(0);

        if update_handle == 0 {
            return Err(());
        }

        if dispmanx::element_change_attributes(
            update_handle,
            self.element.element_handle,
            flags.bits(),
            layer,
            opacity,
            dest_rect,
            src_rect,
            0,
            Transform::NO_ROTATE,
        ) {
            return Err(());
        }

        if dispmanx::update_submit_sync(update_handle) {
            Err(())
        } else {
            Ok(())
        }
    }
}

bitflags! {
    pub struct ElementChange: u32 {
        const LAYER = 1 << 0;
        const OPACITY = 1 << 1;
        const DEST_RECT = 1 << 2;
        const SRC_RECT = 1 << 3;
        //const MASK_RESOURCE = 1 << 4;
        //const TRANSFORM = 1 << 5;
    }
}