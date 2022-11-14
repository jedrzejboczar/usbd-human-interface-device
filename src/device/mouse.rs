//!HID mice
use crate::hid_class::descriptor::HidProtocol;
use core::default::Default;
use delegate::delegate;
use embedded_time::duration::Milliseconds;
use log::error;
use packed_struct::prelude::*;
use usb_device::bus::{InterfaceNumber, StringIndex, UsbBus};
use usb_device::class_prelude::DescriptorWriter;
use usbd_hid::descriptor::generator_prelude::*;

use crate::hid_class::prelude::*;
use crate::interface::raw::{RawInterface, RawInterfaceConfig};
use crate::interface::{InterfaceClass, WrappedInterface, WrappedInterfaceConfig};
use crate::UsbHidError;

/// HID Mouse report descriptor conforming to the Boot specification
///
/// This aims to be compatible with BIOS and other reduced functionality USB hosts
///
/// This is defined in Appendix B.2 & E.10 of [Device Class Definition for Human
/// Interface Devices (Hid) Version 1.11](<https://www.usb.org/sites/default/files/hid1_11.pdf>)
#[rustfmt::skip]
pub const BOOT_MOUSE_REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop),
    0x09, 0x02, // Usage (Mouse),
    0xA1, 0x01, // Collection (Application),
    0x09, 0x01, //   Usage (Pointer),
    0xA1, 0x00, //   Collection (Physical),
    0x95, 0x03, //     Report Count (3),
    0x75, 0x01, //     Report Size (1),
    0x05, 0x09, //     Usage Page (Buttons),
    0x19, 0x01, //     Usage Minimum (1),
    0x29, 0x03, //     Usage Maximum (3),
    0x15, 0x00, //     Logical Minimum (0),
    0x25, 0x01, //     Logical Maximum (1),
    0x81, 0x02, //     Input (Data, Variable, Absolute),
    0x95, 0x01, //     Report Count (1),
    0x75, 0x05, //     Report Size (5),
    0x81, 0x01, //     Input (Constant),
    0x75, 0x08, //     Report Size (8),
    0x95, 0x02, //     Report Count (2),
    0x05, 0x01, //     Usage Page (Generic Desktop),
    0x09, 0x30, //     Usage (X),
    0x09, 0x31, //     Usage (Y),
    0x15, 0x81, //     Logical Minimum (-127),
    0x25, 0x7F, //     Logical Maximum (127),
    0x81, 0x06, //     Input (Data, Variable, Relative),
    0xC0, //   End Collection,
    0xC0, // End Collection
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default, PackedStruct)]
#[packed_struct(endian = "lsb", size_bytes = "3")]
pub struct BootMouseReport {
    #[packed_field]
    pub buttons: u8,
    #[packed_field]
    pub x: i8,
    #[packed_field]
    pub y: i8,
}

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = MOUSE) = {
        (collection = PHYSICAL, usage = POINTER) = {
            (usage_page = BUTTON, usage_min = BUTTON_1, usage_max = BUTTON_3) = {
                #[packed_bits 3] #[item_settings data,variable,absolute] buttons=input;
            };
            (usage_page = GENERIC_DESKTOP,) = {
                (usage = X,) = {
                    #[item_settings data,variable,relative] x=input;
                };
                (usage = Y,) = {
                    #[item_settings data,variable,relative] y=input;
                };
            };
        };
    }
)]
#[derive(Default, Eq, PartialEq)]
pub struct BootMouseReportNew {
    pub buttons: u8,
    pub x: i8,
    pub y: i8,
}


/// Boot compatible mouse with wheel, pan and eight buttons
///
/// Reference: <https://docs.microsoft.com/en-us/previous-versions/windows/hardware/design/dn613912(v=vs.85)>
///            <https://www.microchip.com/forums/tm.aspx?m=391435>
#[rustfmt::skip]
pub const WHEEL_MOUSE_REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop),
    0x09, 0x02,        // Usage (Mouse),
    0xA1, 0x01,        // Collection (Application),
    0x09, 0x01,        //   Usage (Pointer),
    0xA1, 0x00,        //   Collection (Physical),
    0x95, 0x08,        //     Report Count (8),
    0x75, 0x01,        //     Report Size (1),
    0x05, 0x09,        //     Usage Page (Buttons),
    0x19, 0x01,        //     Usage Minimum (1),
    0x29, 0x08,        //     Usage Maximum (8),
    0x15, 0x00,        //     Logical Minimum (0),
    0x25, 0x01,        //     Logical Maximum (1),
    0x81, 0x02,        //     Input (Data, Variable, Absolute),
    0x75, 0x08,        //     Report Size (8),
    0x95, 0x02,        //     Report Count (2),
    0x05, 0x01,        //     Usage Page (Generic Desktop),
    0x09, 0x30,        //     Usage (X),
    0x09, 0x31,        //     Usage (Y),
    0x15, 0x81,        //     Logical Minimum (-127),
    0x25, 0x7F,        //     Logical Maximum (127),
    0x81, 0x06,        //     Input (Data, Variable, Relative),
    0x15, 0x81,        //     Logical Minimum (-127)
    0x25, 0x7F,        //     Logical Maximum (127)
    0x09, 0x38,        //     Usage (Wheel)
    0x75, 0x08,        //     Report Size (8)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
    0x05, 0x0C,        //     Usage Page (Consumer)
    0x0A, 0x38, 0x02,  //     Usage (AC Pan)
    0x95, 0x01,        //     Report Count (1)
    0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
    0xC0,              //   End Collection
    0xC0,              // End Collection
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default, PackedStruct)]
#[packed_struct(endian = "lsb")]
pub struct WheelMouseReport {
    #[packed_field]
    pub buttons: u8,
    #[packed_field]
    pub x: i8,
    #[packed_field]
    pub y: i8,
    #[packed_field]
    pub vertical_wheel: i8,
    #[packed_field]
    pub horizontal_wheel: i8,
}

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = MOUSE) = {
        (collection = PHYSICAL, usage = POINTER) = {
            (usage_page = BUTTON, usage_min = BUTTON_1, usage_max = BUTTON_8) = {
                #[packed_bits 8] #[item_settings data,variable,absolute] buttons=input;
            };
            (usage_page = GENERIC_DESKTOP,) = {
                (usage = X,) = {
                    #[item_settings data,variable,relative] x=input;
                };
                (usage = Y,) = {
                    #[item_settings data,variable,relative] y=input;
                };
                (usage = WHEEL,) = {
                    #[item_settings data,variable,relative] vertical_wheel=input;
                };
            };
            (usage_page = CONSUMER,) = {
                (usage = AC_PAN,) = {
                    #[item_settings data,variable,relative] horizontal_wheel=input;
                };
            };
        };
    }
)]
#[derive(Default, Eq, PartialEq)]
pub struct WheelMouseReportNew {
    pub buttons: u8,
    pub x: i8,
    pub y: i8,
    pub vertical_wheel: i8,
    pub horizontal_wheel: i8,
}

pub struct BootMouseInterface<'a, B: UsbBus> {
    inner: RawInterface<'a, B>,
}

impl<'a, B: UsbBus> BootMouseInterface<'a, B> {
    pub fn write_report(&self, report: &BootMouseReport) -> Result<(), UsbHidError> {
        let data = report.pack().map_err(|e| {
            error!("Error packing BootMouseReport: {:?}", e);
            UsbHidError::SerializationError
        })?;
        self.inner
            .write_report(&data)
            .map(|_| ())
            .map_err(UsbHidError::from)
    }

    pub fn default_config() -> WrappedInterfaceConfig<Self, RawInterfaceConfig<'a>> {
        WrappedInterfaceConfig::new(
            RawInterfaceBuilder::new(BOOT_MOUSE_REPORT_DESCRIPTOR)
                .boot_device(InterfaceProtocol::Mouse)
                .description("Mouse")
                .in_endpoint(UsbPacketSize::Bytes8, Milliseconds(10))
                .unwrap()
                .without_out_endpoint()
                .build(),
            (),
        )
    }
}

impl<'a, B: UsbBus> InterfaceClass<'a> for BootMouseInterface<'a, B> {
    delegate! {
        to self.inner{
           fn report_descriptor(&self) -> &'_ [u8];
           fn id(&self) -> InterfaceNumber;
           fn write_descriptors(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()>;
           fn get_string(&self, index: StringIndex, _lang_id: u16) -> Option<&'_ str>;
           fn reset(&mut self);
           fn set_report(&mut self, data: &[u8]) -> usb_device::Result<()>;
           fn get_report(&mut self, data: &mut [u8]) -> usb_device::Result<usize>;
           fn get_report_ack(&mut self) -> usb_device::Result<()>;
           fn set_idle(&mut self, report_id: u8, value: u8);
           fn get_idle(&self, report_id: u8) -> u8;
           fn set_protocol(&mut self, protocol: HidProtocol);
           fn get_protocol(&self) -> HidProtocol;
        }
    }
}

impl<'a, B: UsbBus> WrappedInterface<'a, B, RawInterface<'a, B>> for BootMouseInterface<'a, B> {
    fn new(interface: RawInterface<'a, B>, _: ()) -> Self {
        Self { inner: interface }
    }
}
pub struct WheelMouseInterface<'a, B: UsbBus> {
    inner: RawInterface<'a, B>,
}

impl<'a, B: UsbBus> WheelMouseInterface<'a, B> {
    pub fn write_report(&self, report: &WheelMouseReport) -> Result<(), UsbHidError> {
        let data = report.pack().map_err(|e| {
            error!("Error packing WheelMouseReport: {:?}", e);
            UsbHidError::SerializationError
        })?;
        self.inner
            .write_report(&data)
            .map(|_| ())
            .map_err(UsbHidError::from)
    }

    pub fn default_config() -> WrappedInterfaceConfig<Self, RawInterfaceConfig<'a>> {
        WrappedInterfaceConfig::new(
            RawInterfaceBuilder::new(WHEEL_MOUSE_REPORT_DESCRIPTOR)
                .boot_device(InterfaceProtocol::Mouse)
                .description("Wheel Mouse")
                .in_endpoint(UsbPacketSize::Bytes8, Milliseconds(10))
                .unwrap()
                .without_out_endpoint()
                .build(),
            (),
        )
    }
}

impl<'a, B: UsbBus> InterfaceClass<'a> for WheelMouseInterface<'a, B> {
    delegate! {
        to self.inner{
           fn report_descriptor(&self) -> &'_ [u8];
           fn id(&self) -> InterfaceNumber;
           fn write_descriptors(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()>;
           fn get_string(&self, index: StringIndex, _lang_id: u16) -> Option<&'_ str>;
           fn reset(&mut self);
           fn set_report(&mut self, data: &[u8]) -> usb_device::Result<()>;
           fn get_report(&mut self, data: &mut [u8]) -> usb_device::Result<usize>;
           fn get_report_ack(&mut self) -> usb_device::Result<()>;
           fn set_idle(&mut self, report_id: u8, value: u8);
           fn get_idle(&self, report_id: u8) -> u8;
           fn set_protocol(&mut self, protocol: HidProtocol);
           fn get_protocol(&self) -> HidProtocol;
        }
    }
}

impl<'a, B: UsbBus> WrappedInterface<'a, B, RawInterface<'a, B>> for WheelMouseInterface<'a, B> {
    fn new(interface: RawInterface<'a, B>, _: ()) -> Self {
        Self { inner: interface }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssmarshal::serialize;

    #[test]
    fn boot_mouse_report_ser() {
        let report = BootMouseReportNew {
            buttons: 0b0000_0011,
            x: 120,
            y: -99,
        };
        let mut buf = [0u8; 3];
        let size = serialize(&mut buf, &report).unwrap();
        assert_eq!(size, 3);
        assert_eq!(&buf[..size], &[0b0000_0011, 120, -99_i8 as u8]);
    }

    #[test]
    fn boot_mouse_report_descriptor() {
        let expected = &[
            0x05, 0x01,        // Usage Page (Generic Desktop Ctrls)
            0x09, 0x02,        // Usage (Mouse)
            0xA1, 0x01,        // Collection (Application)
            0x09, 0x01,        //   Usage (Pointer)
            0xA1, 0x00,        //   Collection (Physical)
            0x05, 0x09,        //     Usage Page (Button)
            0x19, 0x01,        //     Usage Minimum (0x01)
            0x29, 0x03,        //     Usage Maximum (0x03)
            0x15, 0x00,        //     Logical Minimum (0)
            0x25, 0x01,        //     Logical Maximum (1)
            0x75, 0x01,        //     Report Size (1)
            0x95, 0x03,        //     Report Count (3)
            0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x95, 0x05,        //     Report Count (5)
            0x81, 0x03,        //     Input (Const,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x05, 0x01,        //     Usage Page (Generic Desktop Ctrls)
            0x09, 0x30,        //     Usage (X)
            // FIXME: usbd-hid-macros serializes this as i32 but i8 would be enough
            0x17, 0x81, 0xFF, 0xFF, 0xFF,  //     Logical Minimum (-128)
            0x25, 0x7F,        //     Logical Maximum (127)
            0x75, 0x08,        //     Report Size (8)
            0x95, 0x01,        //     Report Count (1)
            0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
            0x09, 0x31,        //     Usage (Y)
            0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
            0xC0,              //   End Collection
            0xC0,              // End Collection
        ];
        assert_eq!(BootMouseReportNew::desc(), expected);
    }


    #[test]
    fn wheel_mouse_report_ser() {
        let report = WheelMouseReportNew {
            buttons: 0b0000_0011,
            x: 120,
            y: -99,
            vertical_wheel: 11,
            horizontal_wheel: -3,
        };
        let mut buf = [0u8; 5];
        let size = serialize(&mut buf, &report).unwrap();
        assert_eq!(size, 5);
        assert_eq!(&buf[..size], &[0b0000_0011, 120, -99_i8 as u8, 11, -3_i8 as u8]);
    }

    #[test]
    fn wheel_mouse_report_descriptor() {
        let expected = &[
            0x05, 0x01,        // Usage Page (Generic Desktop Ctrls)
            0x09, 0x02,        // Usage (Mouse)
            0xA1, 0x01,        // Collection (Application)
            0x09, 0x01,        //   Usage (Pointer)
            0xA1, 0x00,        //   Collection (Physical)
            0x05, 0x09,        //     Usage Page (Button)
            0x19, 0x01,        //     Usage Minimum (0x01)
            0x29, 0x08,        //     Usage Maximum (0x08)
            0x15, 0x00,        //     Logical Minimum (0)
            0x25, 0x01,        //     Logical Maximum (1)
            0x75, 0x01,        //     Report Size (1)
            0x95, 0x08,        //     Report Count (8)
            0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x05, 0x01,        //     Usage Page (Generic Desktop Ctrls)
            0x09, 0x30,        //     Usage (X)
            0x17, 0x81, 0xFF, 0xFF, 0xFF,  //     Logical Minimum (-128)
            0x25, 0x7F,        //     Logical Maximum (127)
            0x75, 0x08,        //     Report Size (8)
            0x95, 0x01,        //     Report Count (1)
            0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
            0x09, 0x31,        //     Usage (Y)
            0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
            0x09, 0x38,        //     Usage (Wheel)
            0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
            0x05, 0x0C,        //     Usage Page (Consumer)
            0x0A, 0x38, 0x02,  //     Usage (AC Pan)
            0x81, 0x06,        //     Input (Data,Var,Rel,No Wrap,Linear,Preferred State,No Null Position)
            0xC0,              //   End Collection
            0xC0,              // End Collection
        ];
        assert_eq!(WheelMouseReportNew::desc(), expected);
    }
}
