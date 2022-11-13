//! Abstract Human Interface Device Class for implementing any HID compliant device

use crate::interface::InterfaceHList;
use crate::interface::{InterfaceClass, AsInterfaceClass, UsbAllocatable};
use core::default::Default;
use core::marker::PhantomData;
use descriptor::*;
use frunk::hlist::{HList, Selector};
use frunk::{HCons, HNil};
use log::{error, info, trace, warn};
use packed_struct::prelude::*;
use usb_device::class_prelude::*;
use usb_device::control::Recipient;
use usb_device::control::Request;
use usb_device::control::RequestType;
use usb_device::Result;

pub mod descriptor;
pub mod prelude;
#[cfg(test)]
mod test;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PrimitiveEnum)]
#[repr(u8)]
pub enum HidRequest {
    GetReport = 0x01,
    GetIdle = 0x02,
    GetProtocol = 0x03,
    SetReport = 0x09,
    SetIdle = 0x0A,
    SetProtocol = 0x0B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, PrimitiveEnum)]
#[repr(u8)]
pub enum UsbPacketSize {
    Bytes8 = 8,
    Bytes16 = 16,
    Bytes32 = 32,
    Bytes64 = 64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbHidBuilderError {
    ValueOverflow,
}

#[must_use = "this `UsbHidClassBuilder` must be assigned or consumed by `::build()`"]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct UsbHidClassBuilder<'a, B, InterfaceList> {
    interface_list: InterfaceList,
    _marker: PhantomData<&'a B>,
}

impl<'a, B> UsbHidClassBuilder<'a, B, HNil> {
    pub fn new() -> Self {
        Self {
            interface_list: HNil,
            _marker: Default::default(),
        }
    }
}

impl<'a, B> Default for UsbHidClassBuilder<'a, B, HNil> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, B: UsbBus, I: HList> UsbHidClassBuilder<'a, B, I> {
    pub fn add_interface<Conf, Class>(
        self,
        interface_config: Conf,
    ) -> UsbHidClassBuilder<'a, B, HCons<Conf, I>>
    where
        Conf: UsbAllocatable<'a, B, Allocated = Class>,
        Class: AsInterfaceClass<'a>,
    {
        UsbHidClassBuilder {
            interface_list: self.interface_list.prepend(interface_config),
            _marker: Default::default(),
        }
    }
}

impl<'a, B, C, Tail> UsbHidClassBuilder<'a, B, HCons<C, Tail>>
where
    B: UsbBus,
    Tail: UsbAllocatable<'a, B>,
    C: UsbAllocatable<'a, B>,
{
    pub fn build(
        self,
        usb_alloc: &'a UsbBusAllocator<B>,
    ) -> UsbHidClass<B, HCons<C::Allocated, Tail::Allocated>> {
        UsbHidClass {
            interfaces: self.interface_list.allocate(usb_alloc),
            _marker: Default::default(),
        }
    }
}

pub type BuilderResult<B> = core::result::Result<B, UsbHidBuilderError>;

/// USB Human Interface Device class
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct UsbHidClass<B, I> {
    interfaces: I,
    _marker: PhantomData<B>,
}

impl<'a, B, InterfaceList: InterfaceHList<'a>> UsbHidClass<B, InterfaceList> {
    pub fn interface<T, Index>(&self) -> &T
    where
        InterfaceList: Selector<T, Index>,
    {
        self.interfaces.get()
    }

    pub fn interfaces(&'a self) -> InterfaceList::Output {
        self.interfaces.to_ref()
    }
}

impl<B: UsbBus, I> UsbHidClass<B, I> {
    fn get_descriptor(transfer: ControlIn<B>, interface: &dyn InterfaceClass<'_>) {
        let request: &Request = transfer.request();
        match DescriptorType::from_primitive((request.value >> 8) as u8) {
            Some(DescriptorType::Report) => {
                match transfer.accept_with(interface.report_descriptor()) {
                    Err(e) => {},
                    Ok(_) => {}
                }
            }
            Some(DescriptorType::Hid) => {
                let mut buffer = [0; 9];
                buffer[0] = buffer.len() as u8;
                buffer[1] = DescriptorType::Hid as u8;
                (buffer[2..]).copy_from_slice(&interface.hid_descriptor_body());
                match transfer.accept_with(&buffer) {
                    Err(e) => {
                    }
                    Ok(_) => {
                    }
                }
            }
            _ => {
            }
        }
    }
}

impl<'a, B, I> UsbClass<B> for UsbHidClass<B, I>
where
    B: UsbBus,
    I: InterfaceHList<'a>,
{
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
        self.interfaces.write_descriptors(writer)?;
        Ok(())
    }

    fn get_string(&self, index: StringIndex, lang_id: u16) -> Option<&str> {
        self.interfaces.get_string(index, lang_id)
    }

    fn reset(&mut self) {
        self.interfaces.reset();
    }

    fn control_out(&mut self, transfer: ControlOut<B>) {
        let request: &Request = transfer.request();

        //only respond to Class requests for this interface
        if !(request.request_type == RequestType::Class
            && request.recipient == Recipient::Interface)
        {
            return;
        }

        let interface = u8::try_from(request.index)
            .ok()
            .and_then(|id| self.interfaces.get_id_mut(id));

        if interface.is_none() {
            return;
        }

        let interface = interface.unwrap();

        match HidRequest::from_primitive(request.request) {
            Some(HidRequest::SetReport) => {
                interface.set_report(transfer.data()).ok();
                transfer.accept().ok();
            }
            Some(HidRequest::SetIdle) => {
                if request.length != 0 {
                }

                interface.set_idle((request.value & 0xFF) as u8, (request.value >> 8) as u8);
                transfer.accept().ok();
            }
            Some(HidRequest::SetProtocol) => {
                if request.length != 0 {
                }
                if let Some(protocol) = HidProtocol::from_primitive((request.value & 0xFF) as u8) {
                    interface.set_protocol(protocol);
                    transfer.accept().ok();
                } else {
                }
            }
            _ => {

            }
        }
    }

    fn control_in(&mut self, transfer: ControlIn<B>) {
        let request: &Request = transfer.request();
        //only respond to requests for this interface
        if !(request.recipient == Recipient::Interface) {
            return;
        }

        let interface_id = u8::try_from(request.index).ok();

        if interface_id.is_none() {
            return;
        }
        let interface_id = interface_id.unwrap();

        match request.request_type {
            RequestType::Standard => {
                let interface = self.interfaces.get_id(interface_id);

                if interface.is_none() {
                    return;
                }
                let interface = interface.unwrap();

                if request.request == Request::GET_DESCRIPTOR {
                    Self::get_descriptor(transfer, interface);
                }
            }

            RequestType::Class => {
                let interface = self.interfaces.get_id_mut(interface_id);

                if interface.is_none() {
                    return;
                }
                let interface = interface.unwrap();

                match HidRequest::from_primitive(request.request) {
                    Some(HidRequest::GetReport) => {
                        let mut data = [0_u8; 64];
                        if let Ok(n) = interface.get_report(&mut data) {
                            if n != transfer.request().length as usize {
                            }
                            match transfer.accept_with(&data[..n]) {
                                Err(e) => {},
                                Ok(()) => {
                                    interface.get_report_ack().unwrap();
                                }
                            }
                        }
                    }
                    Some(HidRequest::GetIdle) => {
                        if request.length != 1 {
                        }

                        let report_id = (request.value & 0xFF) as u8;
                        let idle = interface.get_idle(report_id);
                        match transfer.accept_with(&[idle]) {
                            Err(e) => {},
                            Ok(_) => {},
                        }
                    }
                    Some(HidRequest::GetProtocol) => {
                        if request.length != 1 {

                        }

                        let protocol = interface.get_protocol();
                        match transfer.accept_with(&[protocol as u8]) {
                            Err(e) => {},
                            Ok(_) => {},
                        }
                    }
                    _ => {

                    }
                }
            }
            _ => {}
        }
    }
}
