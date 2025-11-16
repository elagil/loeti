# Graphical user application

This is a GUI for observing the PID filter operation. It can be used to manually tune the PID filter in firmware.

## Usage

- Start this application with `RUST_LOG=INFO cargo run`.
- Connect a device that was built with the `comm` feature enabled.
- Display should start automatically.

## Permissions

On Linux, udev rules may have to be adjusted. Place [the rules file](./99-ergot.rules) in `/etc/udev/rules.d/` and run

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

for reloading the rules.
