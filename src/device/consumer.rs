//!HID consumer control devices

use delegate::delegate;
use embedded_time::duration::Milliseconds;
use log::error;
use ssmarshal::serialize;
use usb_device::class_prelude::*;
use usb_device::UsbError;
use usbd_hid::descriptor::generator_prelude::*;

use crate::hid_class::prelude::*;
use crate::interface::raw::{RawInterface, RawInterfaceConfig};
use crate::interface::{InterfaceClass, WrappedInterface, WrappedInterfaceConfig};

///Consumer control report descriptor - Four `u16` consumer control usage codes as an array (8 bytes)
#[rustfmt::skip]
pub const MULTIPLE_CODE_REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x0C, // Usage Page (Consumer),
    0x09, 0x01, // Usage (Consumer Control),
    0xA1, 0x01, // Collection (Application),
    0x75, 0x10, //     Report Size(16)
    0x95, 0x04, //     Report Count(4)
    0x15, 0x00, //     Logical Minimum(0)
    0x26, 0x9C, 0x02, //     Logical Maximum(0x029C)
    0x19, 0x00, //     Usage Minimum(0)
    0x2A, 0x9C, 0x02, //     Usage Maximum(0x029C)
    0x81, 0x00, //     Input (Array, Data, Variable)
    0xC0, // End Collection
];

/// Consumer control report descriptor - Four `u16` consumer control usage codes as an array (8 bytes)
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
#[repr(C, packed)]
pub struct MultipleConsumerReport {
    pub codes: [u16; 4],
}

// Manual implementation as usbd-hid-macros does not support non-u8 arrays
impl usbd_hid::descriptor::SerializedDescriptor for MultipleConsumerReport {
    fn desc() -> &'static [u8] {
        MULTIPLE_CODE_REPORT_DESCRIPTOR
    }
}
impl serde::ser::Serialize for MultipleConsumerReport {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
where
        S: serde::ser::Serializer,
    {
        let mut s = serializer.serialize_tuple(4)?;
        // Work around unaligned references by copying (curly braces),see https://github.com/rust-lang/rust/issues/82523
        s.serialize_element(&{ self.codes[0] })?;
        s.serialize_element(&{ self.codes[1] })?;
        s.serialize_element(&{ self.codes[2] })?;
        s.serialize_element(&{ self.codes[3] })?;
        s.end()
    }
}
impl usbd_hid::descriptor::AsInputReport for MultipleConsumerReport {}

///Fixed functionality consumer control report descriptor
///
/// Based on [Logitech Gaming Keyboard](http://www.usblyzer.com/reports/usb-properties/usb-keyboard.html)
/// dumped by [USBlyzer](http://www.usblyzer.com/)
///
/// Single bit packed `u8` report
/// * Bit 0 - Scan Next Track
/// * Bit 1 - Scan Previous Track
/// * Bit 2 - Stop
/// * Bit 3 - Play/Pause
/// * Bit 4 - Mute
/// * Bit 5 - Volume Increment
/// * Bit 6 - Volume Decrement
/// * Bit 7 - Reserved
// TODO: support names of more usage codes in usbd-hid-macros
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = CONSUMER, usage = CONSUMER_CONTROL) = {
        (
            usage_page = CONSUMER,
            usage = 0xB5, // ScanNextTrack
            usage = 0xB6, // ScanPreviousTrack
            usage = 0xB7, // Stop
            usage = 0xCD, // PlayPause
            usage = 0xE2, // Mute
            usage = 0xE9, // VolumeIncrement
            usage = 0xEA, // VolumeDecrement
        ) = {
            #[packed_bits 7] #[item_settings data,variable,absolute] codes = input;
        };
    }
)]
#[derive(Default, Eq, PartialEq)]
pub struct FixedFunctionReport {
    pub codes: u8,
}

pub struct ConsumerControlInterface<'a, B: UsbBus> {
    inner: RawInterface<'a, B>,
}

impl<'a, B: UsbBus> ConsumerControlInterface<'a, B> {
    pub fn write_report(&self, report: &MultipleConsumerReport) -> usb_device::Result<usize> {
        let mut data = [0u8; core::mem::size_of::<MultipleConsumerReport>()];
        let size = serialize(&mut data, report).map_err(|e| {
            error!("Error packing MultipleConsumerReport: {:?}", e);
            UsbError::ParseError
        })?;
        self.inner.write_report(&data[..size])
    }

    pub fn default_config() -> WrappedInterfaceConfig<Self, RawInterfaceConfig<'a>> {
        WrappedInterfaceConfig::new(
            RawInterfaceBuilder::new(MULTIPLE_CODE_REPORT_DESCRIPTOR)
                .description("Consumer Control")
                .in_endpoint(UsbPacketSize::Bytes8, Milliseconds(50))
                .unwrap()
                .without_out_endpoint()
                .build(),
            (),
        )
    }
}

impl<'a, B: UsbBus> InterfaceClass<'a> for ConsumerControlInterface<'a, B> {
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

impl<'a, B: UsbBus> WrappedInterface<'a, B, RawInterface<'a, B>>
    for ConsumerControlInterface<'a, B>
{
    fn new(interface: RawInterface<'a, B>, _: ()) -> Self {
        Self { inner: interface }
    }
}

pub struct ConsumerControlFixedInterface<'a, B: UsbBus> {
    inner: RawInterface<'a, B>,
}

impl<'a, B: UsbBus> ConsumerControlFixedInterface<'a, B> {
    pub fn write_report(&self, report: &FixedFunctionReport) -> usb_device::Result<usize> {
        let mut data = [0u8; core::mem::size_of::<FixedFunctionReport>()];
        let size = serialize(&mut data, report).map_err(|e| {
            error!("Error packing MultipleConsumerReport: {:?}", e);
            UsbError::ParseError
        })?;
        self.inner.write_report(&data[..size])
    }

    pub fn default_config() -> WrappedInterfaceConfig<Self, RawInterfaceConfig<'a>> {
        WrappedInterfaceConfig::new(
            RawInterfaceBuilder::new(FixedFunctionReport::desc())
                .description("Consumer Control")
                .in_endpoint(UsbPacketSize::Bytes8, Milliseconds(50))
                .unwrap()
                .without_out_endpoint()
                .build(),
            (),
        )
    }
}

impl<'a, B: UsbBus> InterfaceClass<'a> for ConsumerControlFixedInterface<'a, B> {
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

impl<'a, B: UsbBus> WrappedInterface<'a, B, RawInterface<'a, B>>
    for ConsumerControlFixedInterface<'a, B>
{
    fn new(interface: RawInterface<'a, B>, _: ()) -> Self {
        Self { inner: interface }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_consumer_report_ser() {
        let report = MultipleConsumerReport { codes: [0x0299, 0x1AB, 0x029b, 0x029c] };
        let mut buf = [0u8; 8];
        let size = serialize(&mut buf, &report).unwrap();
        assert_eq!(size, 8);
        assert_eq!(&buf[..size], &[0x99, 0x02, 0xab, 0x01, 0x9b, 0x02, 0x9c, 0x02]);
    }

    #[test]
    fn fixed_function_report_ser() {
        let report = FixedFunctionReport {
            codes: 0b00100100, // Stop, VolumeIncrement
        };
        let mut buf = [0u8; 1];
        let size = serialize(&mut buf, &report).unwrap();
        assert_eq!(size, 1);
        assert_eq!(&buf[..size], &[0b00100100]);
    }

    #[test]
    fn fixed_function_report_descriptor() {
        let expected = &[
            0x05, 0x0C, //        Usage Page (Consumer Devices)
            0x09, 0x01, //        Usage (Consumer Control)
            0xA1, 0x01, //        Collection (Application)
            0x05, 0x0C, //            Usage Page (Consumer Devices)
            0x09, 0xB5, //            Usage (Scan Next Track)
            0x09, 0xB6, //            Usage (Scan Previous Track)
            0x09, 0xB7, //            Usage (Stop)
            0x09, 0xCD, //            Usage (Play/Pause)
            0x09, 0xE2, //            Usage (Mute)
            0x09, 0xE9, //            Usage (Volume Increment)
            0x09, 0xEA, //            Usage (Volume Decrement)
            0x15, 0x00, //            Logical Minimum (0)
            0x25, 0x01, //            Logical Maximum (1)
            0x75, 0x01, //            Report Size (1)
            0x95, 0x07, //            Report Count (7)
            0x81, 0x02, //            Input (Data,Var,Abs,NWrp,Lin,Pref,NNul,Bit)
            0x95, 0x01, //            Report Count (1)
            0x81, 0x03, //            Input (Const,Var,Abs)
            0xC0, //        End Collection
        ];
        assert_eq!(FixedFunctionReport::desc(), expected);
    }
}
