use super::FEATURE_NAME;
use communication;
use error::*;
use std::collections::HashSet;
use std::sync::mpsc;
use std::thread;
use std::time;
use wrapper::dbus;

const INTERFACE_DBUS_PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const INTERFACE_UPOWER: &str = "org.freedesktop.UPower";
const MEMBER_DEVICE_ADDED: &str = "DeviceAdded";
const MEMBER_ENUMERATE_DEVICES: &str = "EnumerateDevices";
const MEMBER_PROPERTIES_CHANGED: &str = "PropertiesChanged";
const PATH_BATTERY_DEVICES_PREFIX: &str = "/org/freedesktop/UPower/devices/battery_";
const PATH_DEVICES_PREFIX: &str = "/org/freedesktop/UPower/devices";
const PATH_UPOWER: &str = "/org/freedesktop/UPower";

pub(super) enum DeviceMessage {
    Added(String),
    Removed(String),
}

pub(super) struct DbusWatcher {
    connection: dbus::Connection,
    id: usize,
    tx: mpsc::Sender<communication::Message>,
    tx_devices: mpsc::Sender<DeviceMessage>,
}

impl DbusWatcher {
    pub(super) fn new(
        id: usize,
        tx: mpsc::Sender<communication::Message>,
        tx_devices: mpsc::Sender<DeviceMessage>,
    ) -> Result<Self> {
        Ok(Self {
            connection: dbus::Connection::new()?,
            id,
            tx,
            tx_devices,
        })
    }

    pub(super) fn start(&self) -> Result<()> {
        self.connection.add_match(dbus::Match {
            interface: INTERFACE_UPOWER,
            member: None,
            path: PATH_UPOWER,
        })?;

        let mut devices = HashSet::new();

        for device in self.get_current_devices()? {
            self.add_device(&mut devices, &device)?;
        }

        // Manually send message before listen because `get_current_devices` waits for
        // dbus method call with a 2 seconds timeout. While waiting it's possible that
        // the initial `update` has already been triggered, so the status bar would show
        // the "no battery" information.
        communication::send_message(FEATURE_NAME, self.id, &self.tx);

        self.connection.listen_for_signals(|signal| {
            if signal.is_interface(INTERFACE_UPOWER)? {
                let path = signal.return_value::<dbus::Path<'_>>()?;

                if signal.is_member(MEMBER_DEVICE_ADDED)? {
                    self.add_device(&mut devices, &path)?;
                } else {
                    self.remove_device(&mut devices, &path)?;
                }

                communication::send_message(FEATURE_NAME, self.id, &self.tx);
            } else if signal.is_member(MEMBER_PROPERTIES_CHANGED)? {
                // wait for /sys/class/power_supply files updates
                thread::sleep(time::Duration::from_secs(2));

                communication::send_message(FEATURE_NAME, self.id, &self.tx);
            }

            Ok(())
        })
    }

    fn add_device<'a>(
        &self,
        devices: &mut HashSet<dbus::Path<'a>>,
        path: &dbus::Path<'a>,
    ) -> Result<()> {
        let name = self.get_device_name(path)?;

        // ignore line power devices
        if name.starts_with(PATH_DEVICES_PREFIX) || devices.contains(path) {
            return Ok(());
        }

        self.connection.add_match(dbus::Match {
            interface: INTERFACE_DBUS_PROPERTIES,
            member: Some(MEMBER_PROPERTIES_CHANGED),
            path,
        })?;

        self.tx_devices
            .send(DeviceMessage::Added(String::from(name)))
            .wrap_error(FEATURE_NAME, "failed to send device added message")?;

        devices.insert(path.clone());

        Ok(())
    }

    fn get_current_devices(&self) -> Result<Vec<dbus::Path<'_>>> {
        let message = dbus::Message::new_method_call(
            INTERFACE_UPOWER,
            PATH_UPOWER,
            INTERFACE_UPOWER,
            MEMBER_ENUMERATE_DEVICES,
        )?;

        let response = self.connection.send_message(message)?;

        response.return_value::<Vec<dbus::Path<'_>>>()
    }

    fn get_device_name<'a>(&self, path: &'a dbus::Path<'_>) -> Result<&'a str> {
        let string = path.as_cstr().to_str().wrap_error(
            FEATURE_NAME,
            "failed to create utf8 string of dbus object path",
        )?;

        Ok(string.trim_left_matches(PATH_BATTERY_DEVICES_PREFIX))
    }

    fn remove_device<'a>(
        &self,
        devices: &mut HashSet<dbus::Path<'a>>,
        path: &dbus::Path<'a>,
    ) -> Result<()> {
        if !devices.contains(path) {
            return Ok(());
        }

        let name = self.get_device_name(path)?;

        self.connection.remove_match(dbus::Match {
            interface: INTERFACE_DBUS_PROPERTIES,
            member: Some(MEMBER_PROPERTIES_CHANGED),
            path,
        })?;

        self.tx_devices
            .send(DeviceMessage::Removed(String::from(name)))
            .wrap_error(FEATURE_NAME, "failed to send device removed message")?;

        devices.remove(path);

        Ok(())
    }
}
