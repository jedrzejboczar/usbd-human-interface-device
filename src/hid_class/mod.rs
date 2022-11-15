//! Abstract Human Interface Device Class for implementing any HID compliant device

use crate::interface::InterfaceHList;
use crate::interface::{InterfaceClass, UsbAllocatable};
use core::default::Default;
use core::marker::PhantomData;
use descriptor::*;
use frunk::hlist::{HList, Selector};
use frunk::{HCons, HNil};
use log::{error, info, trace, warn};
use usb_device::class_prelude::*;
use usb_device::control::Recipient;
use usb_device::control::Request;
use usb_device::control::RequestType;
use usb_device::Result;

pub mod descriptor;
pub mod prelude;
#[cfg(test)]
mod test;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HidRequest {
    GetReport = 0x01,
    GetIdle = 0x02,
    GetProtocol = 0x03,
    SetReport = 0x09,
    SetIdle = 0x0A,
    SetProtocol = 0x0B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        Class: InterfaceClass<'a>,
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
        const REPORT: u8 = DescriptorType::Report as u8;
        const HID: u8 = DescriptorType::Hid as u8;
        match (request.value >> 8) as u8 {
            REPORT => {
                match transfer.accept_with(interface.report_descriptor()) {
                    Err(e) => error!("Failed to send report descriptor - {:?}", e),
                    Ok(_) => {
                        trace!("Sent report descriptor")
                    }
                }
            }
            HID => {
                let mut buffer = [0; 9];
                buffer[0] = buffer.len() as u8;
                buffer[1] = DescriptorType::Hid as u8;
                (buffer[2..]).copy_from_slice(&interface.hid_descriptor_body());
                match transfer.accept_with(&buffer) {
                    Err(e) => {
                        error!("Failed to send Hid descriptor - {:?}", e);
                    }
                    Ok(_) => {
                        trace!("Sent hid descriptor")
                    }
                }
            }
            _ => {
                warn!(
                    "Unsupported descriptor type, request type:{:X?}, request:{:X}, value:{:X}",
                    request.request_type, request.request, request.value
                );
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
        info!("wrote class config descriptor");
        Ok(())
    }

    fn get_string(&self, index: StringIndex, lang_id: u16) -> Option<&str> {
        self.interfaces.get_string(index, lang_id)
    }

    fn reset(&mut self) {
        info!("Reset");
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

        trace!(
            "ctrl_out: request type: {:?}, request: {:X}, value: {:X}",
            request.request_type,
            request.request,
            request.value
        );

        const SET_REPORT: u8 = HidRequest::SetReport as u8;
        const SET_IDLE: u8 = HidRequest::SetIdle as u8;
        const SET_PROTOCOL: u8 = HidRequest::SetProtocol as u8;

        match request.request {
            SET_REPORT => {
                interface.set_report(transfer.data()).ok();
                transfer.accept().ok();
            }
            SET_IDLE => {
                if request.length != 0 {
                    warn!(
                        "Expected SetIdle to have length 0, received {:X}",
                        request.length
                    );
                }

                interface.set_idle((request.value & 0xFF) as u8, (request.value >> 8) as u8);
                transfer.accept().ok();
            }
            SET_PROTOCOL => {
                if request.length != 0 {
                    warn!(
                        "Expected SetProtocol to have length 0, received {:X}",
                        request.length
                    );
                }
                const BOOT: u8 = 0x00;
                const REPORT: u8 = 0x01;
                match (request.value & 0xFF) as u8 {
                    BOOT => {
                        interface.set_protocol(HidProtocol::Boot);
                        transfer.accept().ok();
                    },
                    REPORT => {
                        interface.set_protocol(HidProtocol::Report);
                        transfer.accept().ok();
                    },
                    _ => {
                        error!(
                            "Unable to set protocol, unsupported value:{:X}",
                            request.value
                        );
                    }
                }
            }
            _ => {
                warn!(
                    "Unsupported control_out request type: {:?}, request: {:X}, value: {:X}",
                    request.request_type, request.request, request.value
                );
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

        trace!(
            "ctrl_in: request type: {:?}, request: {:X}, value: {:X}",
            request.request_type,
            request.request,
            request.value
        );

        match request.request_type {
            RequestType::Standard => {
                let interface = self.interfaces.get_id(interface_id);

                if interface.is_none() {
                    return;
                }
                let interface = interface.unwrap();

                if request.request == Request::GET_DESCRIPTOR {
                    info!("Get descriptor");
                    Self::get_descriptor(transfer, interface);
                }
            }

            RequestType::Class => {
                let interface = self.interfaces.get_id_mut(interface_id);

                if interface.is_none() {
                    return;
                }
                let interface = interface.unwrap();

                const GET_REPORT: u8 = HidRequest::GetReport as u8;
                const GET_IDLE: u8 = HidRequest::GetIdle as u8;
                const GET_PROTOCOL: u8 = HidRequest::GetProtocol as u8;

                match request.request {
                    GET_REPORT => {
                        let mut data = [0_u8; 64];
                        if let Ok(n) = interface.get_report(&mut data) {
                            if n != transfer.request().length as usize {
                                warn!(
                                    "GetReport expected {:X} bytes, got {:X} bytes",
                                    transfer.request().length,
                                    data.len()
                                );
                            }
                            match transfer.accept_with(&data[..n]) {
                                Err(e) => error!("Failed to send report - {:?}", e),
                                Ok(()) => {
                                    trace!("Sent report, {:X} bytes", n);
                                    interface.get_report_ack().unwrap();
                                }
                            }
                        }
                    }
                    GET_IDLE => {
                        if request.length != 1 {
                            warn!(
                                "Expected GetIdle to have length 1, received {:X}",
                                request.length
                            );
                        }

                        let report_id = (request.value & 0xFF) as u8;
                        let idle = interface.get_idle(report_id);
                        match transfer.accept_with(&[idle]) {
                            Err(e) => error!("Failed to send idle data - {:?}", e),
                            Ok(_) => info!("Get Idle for ID{:X}: {:X}", report_id, idle),
                        }
                    }
                    GET_PROTOCOL => {
                        if request.length != 1 {
                            warn!(
                                "Expected GetProtocol to have length 1, received {:X}",
                                request.length
                            );
                        }

                        let protocol = interface.get_protocol();
                        match transfer.accept_with(&[protocol as u8]) {
                            Err(e) => error!("Failed to send protocol data - {:?}", e),
                            Ok(_) => info!("Get protocol: {:?}", protocol),
                        }
                    }
                    _ => {
                        warn!(
                            "Unsupported control_in request type: {:?}, request: {:X}, value: {:X}",
                            request.request_type, request.request, request.value
                        );
                    }
                }
            }
            _ => {}
        }
    }
}
