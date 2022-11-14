//!HID keyboards

use delegate::delegate;
use embedded_time::duration::Milliseconds;
use packed_struct::prelude::*;
use usb_device::class_prelude::*;
use usb_device::UsbError;
use usbd_hid::descriptor::generator_prelude::*;

use crate::hid_class::prelude::*;
use crate::interface::managed::{ManagedInterface, ManagedInterfaceConfig};
use crate::interface::{InterfaceClass, WrappedInterface, WrappedInterfaceConfig};
use crate::page::Keyboard;
use crate::UsbHidError;

/// Interface implementing the HID boot keyboard specification
///
/// **Note:** This is a managed interfaces that support HID idle, [BootKeyboardInterface::tick()] must be called every 1ms/at 1kHz.
pub struct BootKeyboardInterface<'a, B: UsbBus> {
    inner: ManagedInterface<'a, B, BootKeyboardReport>,
}

impl<'a, B> BootKeyboardInterface<'a, B>
where
    B: UsbBus,
{
    delegate! {
        to self.inner {
            /// Call every 1ms / at 1 KHz
            pub fn tick(&self) -> Result<(), UsbHidError>;
        }
    }

    pub fn write_report<K: IntoIterator<Item = Keyboard>>(
        &self,
        keys: K,
    ) -> Result<(), UsbHidError> {
        self.inner
            .write_report(&BootKeyboardReport::new(keys))
            .map(|_| ())
    }

    pub fn read_report(&self) -> usb_device::Result<KeyboardLedsReport> {
        let mut data = 0_u8;
        match self.inner.read_report(core::slice::from_mut(&mut data)) {
            Err(e) => Err(e),
            Ok(_) => Ok(KeyboardLedsReport(data)),
        }
    }

    pub fn default_config(
    ) -> WrappedInterfaceConfig<Self, ManagedInterfaceConfig<'a, BootKeyboardReport>> {
        WrappedInterfaceConfig::new(
            ManagedInterfaceConfig::new(
                RawInterfaceBuilder::new(BOOT_KEYBOARD_REPORT_DESCRIPTOR)
                    .boot_device(InterfaceProtocol::Keyboard)
                    .description("Keyboard")
                    .idle_default(Milliseconds(500))
                    .unwrap()
                    .in_endpoint(UsbPacketSize::Bytes8, Milliseconds(10))
                    .unwrap()
                    //.without_out_endpoint()
                    //Shouldn't require a dedicated out endpoint, but leds are flaky without it
                    .with_out_endpoint(UsbPacketSize::Bytes8, Milliseconds(100))
                    .unwrap()
                    .build(),
            ),
            (),
        )
    }
}

impl<'a, B> InterfaceClass<'a> for BootKeyboardInterface<'a, B>
where
    B: UsbBus,
{
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

impl<'a, B> WrappedInterface<'a, B, ManagedInterface<'a, B, BootKeyboardReport>>
    for BootKeyboardInterface<'a, B>
where
    B: UsbBus,
{
    fn new(interface: ManagedInterface<'a, B, BootKeyboardReport>, _: ()) -> Self {
        Self { inner: interface }
    }
}

/// Report implementing the HID boot keyboard specification
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = KEYBOARD) = {
        (usage_page = KEYBOARD, usage_min = 0xE0, usage_max = 0xE7) = {
            #[packed_bits 8] #[item_settings data,variable,absolute] modifier=input;
        };
        (usage_min = 0x00, usage_max = 0xFF) = {
            #[item_settings constant,variable,absolute] reserved=input;
        };
        (usage_page = LEDS, usage_min = 0x01, usage_max = 0x05) = {
            #[packed_bits 5] #[item_settings data,variable,absolute] leds=output;
        };
        (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0xFF) = {
            #[item_settings data,array,absolute] keys=input;
        };
    }
)]
#[derive(Default, Eq, PartialEq)]
pub struct BootKeyboardReport {
    pub modifier: u8,
    pub reserved: u8,
    pub leds: u8,
    pub keys: [u8; 6],
}

/// State of HID keyboard LEDs
bitfield! {
    #[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
    pub struct KeyboardLedsReport(u8);
    pub num_lock, set_num_lock: 0;
    pub caps_lock, set_caps_lock: 1;
    pub scroll_lock, set_scroll_lock: 2;
    pub compose, set_compose: 3;
    pub kana, set_kana: 4;
}


impl BootKeyboardReport {
    pub fn new<K: IntoIterator<Item = Keyboard>>(keys: K) -> Self {
        let mut report = Self::default();

        let mut error = false;
        let mut i = 0;
        for k in keys.into_iter() {
            match k {
                Keyboard::RightGui => report.modifier |= 1 << 0,
                Keyboard::RightAlt => report.modifier |= 1 << 1,
                Keyboard::RightShift => report.modifier |= 1 << 2,
                Keyboard::RightCtrl => report.modifier |= 1 << 3,
                Keyboard::LeftGui => report.modifier |= 1 << 4,
                Keyboard::LeftAlt => report.modifier |= 1 << 5,
                Keyboard::LeftShift => report.modifier |= 1 << 6,
                Keyboard::LeftCtrl => report.modifier |= 1 << 7,
                Keyboard::NoEventIndicated => {}
                Keyboard::ErrorRollOver | Keyboard::POSTFail | Keyboard::ErrorUndefine => {
                    if !error {
                        error = true;
                        i = report.keys.len();
                        report.keys.fill(k.into());
                    }
                }
                _ => {
                    if error {
                        continue;
                    }

                    if i < report.keys.len() {
                        report.keys[i] = k.into();
                        i += 1;
                    } else {
                        error = true;
                        i = report.keys.len();
                        report.keys.fill(Keyboard::ErrorRollOver);
                    }
                }
            }
        }
        report
    }
}

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = KEYBOARD) = {
        (usage_page = KEYBOARD, usage_min = 0xE0, usage_max = 0xE7) = {
            #[packed_bits 8] #[item_settings data,variable,absolute] modifier=input;
        };
        (usage_min = 0x00, usage_max = 0xFF) = {
            #[item_settings constant,variable,absolute] reserved=input;
        };
        (usage_page = LEDS, usage_min = 0x01, usage_max = 0x05) = {
            #[packed_bits 5] #[item_settings data,variable,absolute] leds=output;
        };
        (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0xFF) = {
            #[item_settings data,array,absolute] boot_keys=input;
        };
        (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0x87) = {
            #[packed_bits 136] #[item_settings data,variable,absolute] nkro_keys=input;
        };
    }
)]
#[derive(Default, Eq, PartialEq)]
pub struct NKROBootKeyboardReport {
    pub modifier: u8,
    pub reserved: u8,
    pub leds: u8,
    pub boot_keys: [u8; 6],
    pub nkro_keys: [u8; 17],
}

impl NKROBootKeyboardReport {
    pub fn new<K: IntoIterator<Item = Keyboard>>(keys: K) -> Self {
        let mut report = Self::default();

        let mut boot_keys_error = false;
        let mut i = 0;
        for k in keys.into_iter() {
            match k {
                Keyboard::RightGui => report.modifier |= 1 << 0,
                Keyboard::RightAlt => report.modifier |= 1 << 1,
                Keyboard::RightShift => report.modifier |= 1 << 2,
                Keyboard::RightCtrl => report.modifier |= 1 << 3,
                Keyboard::LeftGui => report.modifier |= 1 << 4,
                Keyboard::LeftAlt => report.modifier |= 1 << 5,
                Keyboard::LeftShift => report.modifier |= 1 << 6,
                Keyboard::LeftCtrl => report.modifier |= 1 << 7,
                Keyboard::NoEventIndicated => {}
                Keyboard::ErrorRollOver | Keyboard::POSTFail | Keyboard::ErrorUndefine => {
                    report.nkro_keys[0] |= 1 << k as u8;

                    if !boot_keys_error {
                        boot_keys_error = true;
                        i = report.boot_keys.len();
                        report.boot_keys.fill(k.into());
                    }
                }
                _ => {
                    if (k as usize) < report.nkro_keys.len() * 8 {
                        let byte = (k as usize) / 8;
                        let bit = (k as u8) % 8;
                        report.nkro_keys[byte] |= 1 << bit;
                    }

                    if boot_keys_error {
                        continue;
                    }

                    if i < report.boot_keys.len() {
                        report.boot_keys[i] = k.into();
                        i += 1;
                    } else {
                        boot_keys_error = true;
                        i = report.boot_keys.len();
                        report.boot_keys.fill(Keyboard::ErrorRollOver.into());
                    }
                }
            }
        }
        report
    }
}

/// Interface implementing a NKRO keyboard compatible with the HID boot keyboard specification
///
/// **Note:** This is a managed interfaces that support HID idle, [NKROBootKeyboardInterface::tick()] must be called every 1ms/ at 1kHz.
pub struct NKROBootKeyboardInterface<'a, B: UsbBus> {
    inner: ManagedInterface<'a, B, NKROBootKeyboardReport>,
}

impl<'a, B> NKROBootKeyboardInterface<'a, B>
where
    B: UsbBus,
{
    delegate! {
        to self.inner {
            /// Call every 1ms / at 1 KHz
            pub fn tick(&self) -> Result<(), UsbHidError>;
        }
    }

    pub fn write_report<K: IntoIterator<Item = Keyboard>>(
        &self,
        keys: K,
    ) -> Result<(), UsbHidError> {
        self.inner
            .write_report(&NKROBootKeyboardReport::new(keys))
            .map(|_| ())
    }

    pub fn read_report(&self) -> usb_device::Result<KeyboardLedsReport> {
        let mut data = 0_u8;
        match self.inner.read_report(core::slice::from_mut(&mut data)) {
            Err(e) => Err(e),
            Ok(_) => Ok(KeyboardLedsReport(data)),
        }
    }

    pub fn default_config(
    ) -> WrappedInterfaceConfig<Self, ManagedInterfaceConfig<'a, NKROBootKeyboardReport>> {
        WrappedInterfaceConfig::new(
            ManagedInterfaceConfig::new(
                RawInterfaceBuilder::new(NKRO_BOOT_KEYBOARD_REPORT_DESCRIPTOR)
                    .description("NKRO Keyboard")
                    .boot_device(InterfaceProtocol::Keyboard)
                    .idle_default(Milliseconds(500))
                    .unwrap()
                    .in_endpoint(UsbPacketSize::Bytes32, Milliseconds(10))
                    .unwrap()
                    .with_out_endpoint(UsbPacketSize::Bytes8, Milliseconds(100))
                    .unwrap()
                    .build(),
            ),
            (),
        )
    }
}

impl<'a, B> InterfaceClass<'a> for NKROBootKeyboardInterface<'a, B>
where
    B: UsbBus,
{
    delegate! {
        to self.inner{
            fn report_descriptor(&self) -> &'_ [u8];
            fn id(&self) -> InterfaceNumber;
            fn write_descriptors(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()>;
            fn get_string(&self, index: StringIndex, _lang_id: u16) -> Option<&'_ str>;
            fn set_report(&mut self, data: &[u8]) -> usb_device::Result<()>;
            fn get_report(&mut self, data: &mut [u8]) -> usb_device::Result<usize>;
            fn get_report_ack(&mut self) -> usb_device::Result<()>;
            fn get_idle(&self, report_id: u8) -> u8;
            fn set_protocol(&mut self, protocol: HidProtocol);
            fn get_protocol(&self) -> HidProtocol;
            fn reset(&mut self);
            fn set_idle(&mut self, report_id: u8, value: u8);
        }
    }
}

impl<'a, B> WrappedInterface<'a, B, ManagedInterface<'a, B, NKROBootKeyboardReport>>
    for NKROBootKeyboardInterface<'a, B>
where
    B: 'a + UsbBus,
{
    fn new(interface: ManagedInterface<'a, B, NKROBootKeyboardReport>, _: ()) -> Self {
        Self { inner: interface }
    }
}

/// HID Keyboard report descriptor implementing an NKRO keyboard as a bitmap.
///
/// N.B. This is not compatible with the HID boot specification
//18 bytes - derived from https://learn.adafruit.com/custom-hid-devices-in-circuitpython/n-key-rollover-nkro-hid-device
//First byte modifiers, 17 byte key bit array
#[rustfmt::skip]
pub const NKRO_COMPACT_KEYBOARD_REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01,                     // Usage Page (Generic Desktop),
    0x09, 0x06,                     // Usage (Keyboard),
    0xA1, 0x01,                     // Collection (Application),
    // bitmap of modifiers
    0x75, 0x01,                     //   Report Size (1),
    0x95, 0x08,                     //   Report Count (8),
    0x05, 0x07,                     //   Usage Page (Key Codes),
    0x19, 0xE0,                     //   Usage Minimum (224),
    0x29, 0xE7,                     //   Usage Maximum (231),
    0x15, 0x00,                     //   Logical Minimum (0),
    0x25, 0x01,                     //   Logical Maximum (1),
    0x81, 0x02,                     //   Input (Data, Variable, Absolute), ;Modifier byte
    // LED output report
    0x95, 0x05,                     //   Report Count (5),
    0x75, 0x01,                     //   Report Size (1),
    0x05, 0x08,                     //   Usage Page (LEDs),
    0x19, 0x01,                     //   Usage Minimum (1),
    0x29, 0x05,                     //   Usage Maximum (5),
    0x91, 0x02,                     //   Output (Data, Variable, Absolute),
    0x95, 0x01,                     //   Report Count (1),
    0x75, 0x03,                     //   Report Size (3),
    0x91, 0x03,                     //   Output (Constant),
    // bitmap of keys
    0x95, 0x88,                     //   Report Count () - (REPORT_BYTES-1)*8
    0x75, 0x01,                     //   Report Size (1),
    0x15, 0x00,                     //   Logical Minimum (0),
    0x25, 0x01,                     //   Logical Maximum(1),
    0x05, 0x07,                     //   Usage Page (Key Codes),
    0x19, 0x00,                     //   Usage Minimum (0),
    0x29, 0x87,                     //   Usage Maximum (), - (REPORT_BYTES-1)*8-1
    0x81, 0x02,                     //   Input (Data, Variable, Absolute),
    0xc0                            // End Collection
];

#[cfg(test)]
mod test {
    use super::*;
    use ssmarshal::serialize;

    #[test]
    fn boot_keyboard_report_mixed() {
        let report = BootKeyboardReport::new([
            Keyboard::LeftAlt,
            Keyboard::A,
            Keyboard::B,
            Keyboard::C,
            Keyboard::RightGUI,
        ]);
        let mut buf = [0_u8; 8];
        serialize(&mut buf, &report).unwrap();

        assert_eq!(
            buf,
            [
                0x1_u8 << (Keyboard::LeftAlt as u8 - Keyboard::LeftControl as u8)
                    | 0x1_u8 << (Keyboard::RightGUI as u8 - Keyboard::LeftControl as u8),
                0,
                Keyboard::A as u8,
                Keyboard::B as u8,
                Keyboard::C as u8,
                0,
                0,
                0
            ]
        );
    }

    #[test]
    fn boot_keyboard_report_keys() {
        let report = BootKeyboardReport::new([
            Keyboard::A,
            Keyboard::B,
            Keyboard::C,
            Keyboard::D,
            Keyboard::E,
            Keyboard::F,
        ]);
        let mut buf = [0_u8; 8];
        serialize(&mut buf, &report).unwrap();

        assert_eq!(
            buf,
            [
                0,
                0,
                Keyboard::A as u8,
                Keyboard::B as u8,
                Keyboard::C as u8,
                Keyboard::D as u8,
                Keyboard::E as u8,
                Keyboard::F as u8
            ]
        );
    }

    #[test]
    fn boot_keyboard_report_rollover() {
        let report = BootKeyboardReport::new([
            Keyboard::LeftAlt,
            Keyboard::A,
            Keyboard::B,
            Keyboard::C,
            Keyboard::D,
            Keyboard::E,
            Keyboard::F,
            Keyboard::G,
            Keyboard::RightGUI,
        ]);
        let mut buf = [0_u8; 8];
        serialize(&mut buf, &report).unwrap();

        assert_eq!(
            buf,
            [
                0x1_u8 << (Keyboard::LeftAlt as u8 - Keyboard::LeftControl as u8)
                    | 0x1_u8 << (Keyboard::RightGUI as u8 - Keyboard::LeftControl as u8),
                0,
                Keyboard::ErrorRollOver as u8,
                Keyboard::ErrorRollOver as u8,
                Keyboard::ErrorRollOver as u8,
                Keyboard::ErrorRollOver as u8,
                Keyboard::ErrorRollOver as u8,
                Keyboard::ErrorRollOver as u8,
            ]
        );
    }

    #[test]
    fn boot_keyboard_report_ser() {
        use Keyboard::*;
        let report = BootKeyboardReport {
            modifier: 0b0001_0100,
            reserved: 0,
            leds: 0,
            keys: [A as u8, B as u8, C as u8, D as u8, 0, 0],
        };
        let mut buf = [0u8; 8];
        let size = serialize(&mut buf, &report).unwrap();
        assert_eq!(size, 8);
        assert_eq!(&buf[..size], &[0b0001_0100, 0, 4, 5, 6, 7, 0, 0]);
    }

    #[test]
    fn boot_keyboard_report_descriptor() {
        let expected = &[
            0x05, 0x01,        // Usage Page (Generic Desktop Ctrls)
            0x09, 0x06,        // Usage (Keyboard)
            0xA1, 0x01,        // Collection (Application)
            0x05, 0x07,        //   Usage Page (Kbrd/Keypad)
            0x19, 0xE0,        //   Usage Minimum (0xE0)
            0x29, 0xE7,        //   Usage Maximum (0xE7)
            0x15, 0x00,        //   Logical Minimum (0)
            0x25, 0x01,        //   Logical Maximum (1)
            0x75, 0x01,        //   Report Size (1)
            0x95, 0x08,        //   Report Count (8)
            0x81, 0x02,        //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x19, 0x00,        //   Usage Minimum (0x00)
            0x29, 0xFF,        //   Usage Maximum (0xFF)
            0x26, 0xFF, 0x00,  //   Logical Maximum (255)
            0x75, 0x08,        //   Report Size (8)
            0x95, 0x01,        //   Report Count (1)
            0x81, 0x03,        //   Input (Const,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x05, 0x08,        //   Usage Page (LEDs)
            0x19, 0x01,        //   Usage Minimum (Num Lock)
            0x29, 0x05,        //   Usage Maximum (Kana)
            0x25, 0x01,        //   Logical Maximum (1)
            0x75, 0x01,        //   Report Size (1)
            0x95, 0x05,        //   Report Count (5)
            0x91, 0x02,        //   Output (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
            0x95, 0x03,        //   Report Count (3)
            0x91, 0x03,        //   Output (Const,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
            0x05, 0x07,        //   Usage Page (Kbrd/Keypad)
            0x19, 0x00,        //   Usage Minimum (0x00)
            0x29, 0xFF,        //   Usage Maximum (0xFF)
            0x26, 0xFF, 0x00,  //   Logical Maximum (255)
            0x75, 0x08,        //   Report Size (8)
            0x95, 0x06,        //   Report Count (6)
            0x81, 0x00,        //   Input (Data,Array,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0xC0,              // End Collection
        ];
        assert_eq!(BootKeyboardReport::desc(), expected);
    }

    #[test]
    fn nkro_boot_keyboard_report_ser() {
        use Keyboard::*;
        let report = NKROBootKeyboardReport {
            modifier: 0b0001_0100,
            reserved: 0,
            leds: 0,
            boot_keys: [A as u8, B as u8, C as u8, D as u8, 0, 0],
            nkro_keys: [0x10, 0, 0, 0, 0, 0, 0, 0, 0x04, 0, 0, 0, 0, 0, 0, 0x20, 0],
        };
        let mut buf = [0u8; 8 + 17];
        let size = serialize(&mut buf, &report).unwrap();
        assert_eq!(size, 8 + 17);
        assert_eq!(&buf[..size], &[
            0b0001_0100, 0,
            4, 5, 6, 7, 0, 0,
            0x10, 0, 0, 0, 0, 0, 0, 0, 0x04, 0, 0, 0, 0, 0, 0, 0x20, 0,
        ]);
    }

    #[test]
    fn nkro_boot_keyboard_report_descriptor() {
        let expected = &[
            0x05, 0x01,        // Usage Page (Generic Desktop Ctrls)
            0x09, 0x06,        // Usage (Keyboard)
            0xA1, 0x01,        // Collection (Application)
            0x05, 0x07,        //   Usage Page (Kbrd/Keypad)
            0x19, 0xE0,        //   Usage Minimum (0xE0)
            0x29, 0xE7,        //   Usage Maximum (0xE7)
            0x15, 0x00,        //   Logical Minimum (0)
            0x25, 0x01,        //   Logical Maximum (1)
            0x75, 0x01,        //   Report Size (1)
            0x95, 0x08,        //   Report Count (8)
            0x81, 0x02,        //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x19, 0x00,        //   Usage Minimum (0x00)
            0x29, 0xFF,        //   Usage Maximum (0xFF)
            0x26, 0xFF, 0x00,  //   Logical Maximum (255)
            0x75, 0x08,        //   Report Size (8)
            0x95, 0x01,        //   Report Count (1)
            0x81, 0x03,        //   Input (Const,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x05, 0x08,        //   Usage Page (LEDs)
            0x19, 0x01,        //   Usage Minimum (Num Lock)
            0x29, 0x05,        //   Usage Maximum (Kana)
            0x25, 0x01,        //   Logical Maximum (1)
            0x75, 0x01,        //   Report Size (1)
            0x95, 0x05,        //   Report Count (5)
            0x91, 0x02,        //   Output (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
            0x95, 0x03,        //   Report Count (3)
            0x91, 0x03,        //   Output (Const,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
            0x05, 0x07,        //   Usage Page (Kbrd/Keypad)
            0x19, 0x00,        //   Usage Minimum (0x00)
            0x29, 0xFF,        //   Usage Maximum (0xFF)
            0x26, 0xFF, 0x00,  //   Logical Maximum (255)
            0x75, 0x08,        //   Report Size (8)
            0x95, 0x06,        //   Report Count (6)
            0x81, 0x00,        //   Input (Data,Array,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0x05, 0x07,        //   Usage Page (Kbrd/Keypad)
            0x19, 0x00,        //   Usage Minimum (0x00)
            0x29, 0x87,        //   Usage Maximum (0x87)
            0x25, 0x01,        //   Logical Maximum (1)
            0x75, 0x01,        //   Report Size (1)
            0x95, 0x88,        //   Report Count (-120)
            0x81, 0x02,        //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
            0xC0,              // End Collection
        ];
        assert_eq!(NKROBootKeyboardReport::desc(), expected);
    }
}
