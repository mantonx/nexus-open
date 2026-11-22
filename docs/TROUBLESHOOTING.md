# Troubleshooting

Start with the doctor. It checks everything below and says what to do:

```
nexusctl doctor
```

The settings window runs the same checks and shows any failure in a
banner, with a **Copy diagnostics** button for bug reports (your home
folder is shown as `~`). Each section here is named after a line of the
doctor's output.

## daemon

`FAIL daemon: nexusd is not running`

nexusd draws the strip; without it the strip stays dark or frozen. If
the package installed its service, start it and read its log if it stops
again:

```
systemctl --user enable --now nexusd
journalctl --user -u nexusd -e
```

Without the service, run `nexusd run` in a terminal. It stops with the
terminal.

## daemon device

`note daemon device: …`

nexusd is running but is not drawing on the strip. The detail says why;
usually it is the same reason the `device`, `access` or `holders` line
below fails. nexusd retries by itself every second or so, so once that is
fixed the strip lights without a restart.

## old udev rule

`FAIL old udev rule: /etc/udev/rules.d/99-corsair-nexus.rules is installed`

An older version installed a rule numbered 99. systemd applies seat
access in `73-seat-late.rules`, so a rule sorting after it never grants
access. Remove it; the current rule is `70-nexus-open.rules`, which the
package installs:

```
sudo rm /etc/udev/rules.d/99-corsair-nexus.rules
sudo udevadm control --reload
```

## device

`FAIL device: no iCUE Nexus found`

No USB device with id `1b1c:1b8e` is present. Check the cable, and that
`lsusb | grep 1b1c:1b8e` lists it. A hub that sleeps can drop it; try a
port on the computer itself.

`FAIL device: the strip is in use by nexus-open (pid …)`

Another program has taken the strip: it detached the kernel's HID driver
to drive the strip itself, so nexusd cannot reach it. The old Go version
of Nexus Open does this whenever it runs, including when started from its
menu entry. Quit it from its tray icon (Quit). Then unplug the strip and
plug it back in: the old app does not hand the strip back to its driver
when it quits, so it stays dark until replugged. nexusd picks it up by
itself.

For another program the doctor names, quit it; if the strip stays dark,
replug it. When the line says `in use by another program` without a
name, the program runs as another user, so this user cannot see which.
When nexusd runs as its service, `nexusctl status` always says it that
way: the service's sandbox cannot see other programs' open files. The
doctor and the settings window can, and name it.

`FAIL device: the strip was left without its driver by another program`

A program took the strip (see above) and has quit without giving it
back. Unplug the strip and plug it back in.

`FAIL device: the iCUE Nexus is plugged in but its hidraw node is not there yet`

Normal for a few seconds after plugging it in or waking the computer. If
it lasts, replug it.

## access

`FAIL access: permission denied`

The strip is found but this user cannot open it. The packages install
the udev rule (`/usr/lib/udev/rules.d/70-nexus-open.rules`) and apply it,
so this means it is missing or was not applied.

If it is missing, reinstall the package. With the tarball, `install.sh`
leaves a copy in `~/.local/share/nexusd/`; install it with:

```
sudo install -m 644 ~/.local/share/nexusd/70-nexus-open.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw
```

From a source checkout, the rule is `packaging/udev/70-nexus-open.rules`.

If the rule is installed but not applied yet, unplug the Nexus and plug it
back in, or run the second command above. The rule grants access to the
user at the seat, so a session over SSH does not get it.

## holders

`FAIL holders: also open in …`

Another program has the strip open and the two fight over what it shows.
The old Go version of Nexus Open is the usual one:

```
systemctl --user disable --now nexus-open
```

## store

`FAIL store: … cannot be used`

The layout database (`~/.local/state/nexusd/nexusd.db`) could not be
opened or read. The detail says why: a full disk, wrong permissions, or
a file from a newer version. Until it is fixed the strip shows the
default layout and changes are not saved. The database is never deleted
or rewritten to recover; see [STORAGE.md](STORAGE.md) for its backups.

`note store: … does not exist yet` is normal before the first start.

## other devices

`-- other devices: K100 RGB keyboard (no access), …; see nexusctl devices`

Corsair devices other than the strip were found. None is needed for the
strip, and their support is still being built, so this is a note, not a
failure. `nexusctl devices` lists each one, what it will be for, and
what blocks it. "no access" is expected with the package: its rule opens
only the strip, and devices whose support is still being built stay
closed. The development rule (`data/udev/70-nexus.rules`) opens the
interfaces the app will use; a keyboard's typing interface stays closed.
