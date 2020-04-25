extern crate rusb;
use rusb::{
    Context, Device, DeviceDescriptor, DeviceHandle, Direction, Result, TransferType, UsbContext,
};
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        println!("usage: async <vendor-id> <product-id>");
        return;
    }

    let vid: u16 = FromStr::from_str(args[1].as_ref()).unwrap();
    let pid: u16 = FromStr::from_str(args[2].as_ref()).unwrap();

    match Context::new() {
        Ok(mut context) => match open_device(&mut context, vid, pid) {
            Some((mut device, device_desc, mut handle)) => {
                read_device(&context, &mut device, &device_desc, &mut handle).unwrap()
            }
            None => println!("could not find device {:04x}:{:04x}", vid, pid),
        },
        Err(e) => panic!("could not initialize libusb: {}", e),
    }
}

fn open_device<T: UsbContext>(
    context: &mut T,
    vid: u16,
    pid: u16,
) -> Option<(Device<T>, DeviceDescriptor, DeviceHandle<T>)> {
    let devices = match context.devices() {
        Ok(d) => d,
        Err(_) => return None,
    };

    for device in devices.iter() {
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
            match device.open() {
                Ok(handle) => return Some((device, device_desc, handle)),
                Err(_) => continue,
            }
        }
    }

    None
}

fn read_device<T: UsbContext>(
    context: &rusb::Context,
    device: &rusb::Device<T>,
    device_desc: &rusb::DeviceDescriptor,
    handle: &mut rusb::DeviceHandle<T>,
) -> rusb::Result<()> {
    match find_readable_endpoint(device, device_desc, rusb::TransferType::Interrupt) {
        Some(endpoint) => read_endpoint(context, handle, endpoint, rusb::TransferType::Interrupt),
        None => println!("No readable interrupt endpoint"),
    }

    match find_readable_endpoint(device, device_desc, rusb::TransferType::Bulk) {
        Some(endpoint) => read_endpoint(context, handle, endpoint, rusb::TransferType::Bulk),
        None => println!("No readable bulk endpoint"),
    }

    Ok(())
}

fn find_readable_endpoint<T: UsbContext>(
    device: &rusb::Device<T>,
    device_desc: &rusb::DeviceDescriptor,
    transfer_type: rusb::TransferType,
) -> Option<Endpoint> {
    for n in 0..device_desc.num_configurations() {
        let config_desc = match device.config_descriptor(n) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                for endpoint_desc in interface_desc.endpoint_descriptors() {
                    if endpoint_desc.direction() == rusb::Direction::In
                        && endpoint_desc.transfer_type() == transfer_type
                    {
                        return Some(Endpoint {
                            config: config_desc.number(),
                            iface: interface_desc.interface_number(),
                            setting: interface_desc.setting_number(),
                            address: endpoint_desc.address(),
                        });
                    }
                }
            }
        }
    }

    None
}

fn read_endpoint<T: UsbContext>(
    context: &rusb::Context,
    handle: &mut rusb::DeviceHandle<T>,
    endpoint: Endpoint,
    transfer_type: rusb::TransferType,
) {
    println!("Reading from endpoint: {:?}", endpoint);

    configure_endpoint(handle, &endpoint).unwrap();

    let mut buffers = [[0u8; 64]; 8];

    {
        let mut async_group = ::rusb::AsyncGroup::new(context);
        let timeout = Duration::from_secs(1);

        match transfer_type {
            rusb::TransferType::Interrupt => {
                for buf in &mut buffers {
                    async_group
                        .submit(::rusb::Transfer::interrupt(
                            handle,
                            endpoint.address,
                            buf,
                            timeout,
                        ))
                        .unwrap();
                }
            }
            rusb::TransferType::Bulk => {
                for buf in &mut buffers {
                    async_group
                        .submit(::rusb::Transfer::bulk(
                            handle,
                            endpoint.address,
                            buf,
                            timeout,
                        ))
                        .unwrap();
                }
            }
            _ => unimplemented!(),
        }

        loop {
            let mut transfer = async_group.wait_any().unwrap();
            println!("Read: {:?} {:?}", transfer.status(), transfer.actual());
            async_group.submit(transfer).unwrap();
        }
    }
}

fn configure_endpoint<'a, T: UsbContext>(
    handle: &'a mut rusb::DeviceHandle<T>,
    endpoint: &Endpoint,
) -> rusb::Result<()> {
    handle.set_auto_detach_kernel_driver(true).unwrap();
    //handle.set_active_configuration(endpoint.config)?;
    handle.claim_interface(endpoint.iface)?;
    handle.set_alternate_setting(endpoint.iface, endpoint.setting)?;
    Ok(())
}
