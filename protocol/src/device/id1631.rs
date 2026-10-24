//! Device support for tumble dryers with software ID 1631.
//!
//! Supports appliances with software ID 1631, such as the Miele Softtronic T 8625 C. The dryer has
//! an EW151 board with a Renesas M16C/62P (M3062LFGPFP) and an ELP330 board with a Renesas
//! H8/3064. The diagnostic interface answers from the H8: the code read at 0x0f000 is H8/300H,
//! and it keeps its variables at 0xfef30-0xff329, in the H8's on-chip RAM.
//!
//! **This implementation is read-only and preliminary.** The memory map of this software ID is not
//! known yet, so it offers no properties. It unlocks read and full access, so that the device can
//! be connected to and mapped (see *Mapping a new device* in `esphome/README.md`). With read
//! access alone, memory reads outside 0x0f000 time out. It does not halt the device or write to
//! it.
//!
//! A tumble dryer instance can be obtained using [`TumbleDryer::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, Device, DeviceKind, Error, Interface, Property, Result, Value, private,
};
use alloc::boxed::Box;
use embedded_io_async::{Read, Write};

macro_rules! compatible_software_ids {
    () => {
        1631
    };
}
pub(super) use compatible_software_ids;

/// Read access key, found with `find_keys` on a T 8625 C (issue #57).
const READ_ACCESS_KEY: u16 = 0xbde7;

/// Full access key, found with `find_keys` on a T 8625 C (issue #57).
///
/// Unlocked so that memory reads work, as in the dumps made for that issue. Nothing here halts the
/// device or writes to it.
const FULL_ACCESS_KEY: u16 = 0x902f;

/// Tumble dryer device implementation.
#[derive(Debug)]
pub struct TumbleDryer<P> {
    intf: Interface<P>,
    software_id: u16,
}

impl<P: Read + Write> TumbleDryer<P> {
    pub(crate) async fn initialize(
        mut intf: Interface<P>,
        software_id: u16,
    ) -> Result<Self, P::Error> {
        intf.unlock_read_access(READ_ACCESS_KEY).await?;
        intf.unlock_full_access(FULL_ACCESS_KEY).await?;

        Ok(Self { intf, software_id })
    }
}

#[async_trait::async_trait(?Send)]
impl<P: Read + Write> Device<P> for TumbleDryer<P> {
    async fn connect(port: P) -> Result<Self, P::Error> {
        let mut intf = Interface::new(port);
        let id = intf.query_software_id().await?;

        match id {
            compatible_software_ids!() => Self::initialize(intf, id).await,
            _ => Err(Error::UnknownSoftwareId(id)),
        }
    }

    fn interface(&mut self) -> &mut Interface<P> {
        &mut self.intf
    }

    fn software_id(&self) -> u16 {
        self.software_id
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::TumbleDryer
    }

    fn properties(&self) -> &'static [Property] {
        &[]
    }

    fn actions(&self) -> &'static [Action] {
        &[]
    }

    async fn query_property(&mut self, _prop: &Property) -> Result<Value, P::Error> {
        Err(Error::UnknownProperty)
    }

    async fn trigger_action(
        &mut self,
        _action: &Action,
        _param: Option<&str>,
    ) -> Result<(), P::Error> {
        Err(Error::UnknownAction)
    }
}

impl<P> private::Sealed for TumbleDryer<P> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::init_logger;
    use alloc::collections::vec_deque::VecDeque;
    use core::convert::Infallible;

    #[tokio::test]
    async fn connect_unlocks_access_only() -> Result<(), Infallible> {
        init_logger();

        // Software ID 1631 (0x065f), then the acknowledgements of the two unlocks
        let mut deque = VecDeque::from([0x00, 0x5f, 0x06, 0x65, 0x00, 0x00]);

        {
            let dev = crate::device::connect(&mut deque).await?;

            assert_eq!(dev.software_id(), 1631, "software ID should be correct");
            assert_eq!(
                dev.kind(),
                DeviceKind::TumbleDryer,
                "device kind should be correct"
            );
            assert!(dev.properties().is_empty(), "no properties are known yet");
        }

        // What was sent: the software ID query, the read access unlock with key 0xbde7 and the
        // full access unlock with key 0x902f, and nothing after them
        assert_eq!(
            deque,
            [
                0x11, 0x00, 0x00, 0x02, 0x13, 0x00, 0x20, 0xe7, 0xbd, 0x00, 0xc4, 0x32, 0x2f, 0x90,
                0x00, 0xf1,
            ],
            "deque contents should be correct"
        );

        Ok(())
    }
}
