# Löti

Soldering station hardware and firmware, powered by USB PD, delivering up to 100 W.

Supports irons with thermocouples as temperature probes, for example
- JBC C210
- JBC C245

## Resources

- [Hardware design](./hardware)
- [Firmware](./firmware)

## Updating firmware

On the device, either
- hold the `BOOT`-button on the PCB and insert the USB cable into the data/debug port, or
- go to the user menu (long-press encoder button), scroll down to `DFU mode`, and select.

Once in DFU mode, flash a binary from the release artifacts. For example with

```bash
dfu-util -a 0 -s 0x08000000:leave -D <firmware_file_name>.bin
```

## The finished product

![Active](img/active.jpg "Active")
![Side 1](img/side_1.jpg "Side 1")
![Side 2](img/side_2.jpg "Side 2")
![Front](img/pcb_front.jpg "Front")
![Back alt](img/pcb_back_alt.jpg "Back alt")
![Back](img/pcb_back.jpg "Back")
