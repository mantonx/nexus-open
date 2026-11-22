# Installing Nexus Open

Nexus Open has two parts. A small service, `nexusd`, draws the strip; it
runs as you, starts when you log in, and keeps working with every window
closed. The settings window, **Nexus Open Settings** in your app menu,
edits what the strip shows. `nexusctl` is the same thing for the command
line.

Download the file for your system from the
[releases page](https://github.com/mantonx/nexus-open/releases/latest).
In the commands below, replace `VERSION` with the version you downloaded.

You need a desktop Linux with systemd (every distribution below has it),
on a 64-bit PC (x86_64).

## Coming from the old Nexus Open

The old app (written in Go, with a tray icon and a Flutter settings
window) is replaced by this one. The packages have the same name,
`nexus-open`, so installing the new package replaces the old one.

1. **Quit the old app** before installing: click its tray icon and choose
   Quit. The new package also stops the old app's service.
2. **If the strip goes dark** and stays dark, unplug it and plug it back in
   once. The old app takes the strip from the system's driver and does not
   give it back when it quits. The package tries to give it back by itself,
   so usually you will not need to.
3. **Your old layout** is not moved over by itself. To bring it across:

   ```
   nexusctl import-legacy --activate
   ```

   This reads the old app's database and says what could not be carried
   over. Run `nexusctl import-legacy --print` first to see the result
   without changing anything.

## Arch Linux

```
sudo pacman -U nexus-open-VERSION-1-x86_64.pkg.tar.zst
```

To build it yourself from the source instead, run
`packaging/nexusd/arch/build-local.sh` in a checkout (no root needed), then
install the package it writes to `target/dist/`.

## Debian and Ubuntu

Debian 13 or newer, Ubuntu 24.04 or newer. Open the `.deb` file with your
software centre and choose Install, or:

```
sudo apt install ./nexus-open_VERSION_amd64.deb
```

Type the `./`: it tells apt this is a file, so it also installs what the
package needs.

## Fedora

Open the `.rpm` file with GNOME Software (or Discover) and choose Install,
or:

```
sudo dnf install ./nexus-open-VERSION-1.x86_64.rpm
```

## Any other distribution: the tarball

The tarball installs for you alone, under your home folder, and needs
root for one step only.

```
tar -xzf nexus-open-VERSION-x86_64-linux.tar.gz
cd nexus-open-VERSION-x86_64-linux
./install.sh
```

`install.sh` puts `nexusd` and `nexusctl` in `~/.local/bin`, the service in
`~/.config/systemd/user`, and the menu entry and icon in `~/.local/share`,
then starts the service and sets it to start when you log in.

The one step that needs root is the udev rule, which lets you use the
strip. If it is not installed, `install.sh` prints the command, which is:

```
sudo install -m 644 ~/.local/share/nexusd/70-nexus-open.rules /etc/udev/rules.d/ && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw
```

Run `./install.sh` again from a newer tarball to update.

## What the package does

The Arch, Debian and Fedora packages install the same files:

| File | What it is for |
| --- | --- |
| `/usr/bin/nexusd`, `/usr/bin/nexusctl` | the service and settings window; the command line |
| `/usr/lib/udev/rules.d/70-nexus-open.rules` | lets the person sitting at the computer use the strip, with no group to join |
| `/usr/lib/systemd/user/nexusd.service` | the service, run as each user, not as root |
| `/usr/lib/systemd/user-preset/80-nexus-open.preset` | says the service starts for everyone who logs in |
| `/usr/share/applications/io.nexus.open.desktop`, the icon, `/usr/share/metainfo/io.nexus.open.metainfo.xml` | Nexus Open Settings in the app menu and in software centres |
| `/usr/lib/nexus-open/package-hook` | what the install and removal steps below run |

When it is installed, the package:

1. **Loads the udev rule and applies it to a strip that is already plugged
   in**, so you do not need to unplug it.
2. **Turns the service on for every user**, so it starts whenever anyone
   logs in. Users of the login screen itself (system accounts) are left
   out.
3. **Stops the old app's service** if it runs, and hands the strip back to
   the system's driver if the old app left it without one.
4. **Starts the service for everyone who is logged in now**, so the strip
   lights up a few seconds after the install finishes, without logging out.

If the strip is still dark after that, open Nexus Open Settings. If the
service is not running, the window says so and has a **Start the service**
button; for anything else, it names the problem and what to do. Logging
out and back in also starts the service.

## Checking that it works

The strip shows the time and your computer's readings within a few seconds
of the install. Nexus Open Settings shows "Strip connected" in its header.
From a terminal, `nexusctl doctor` checks everything the strip needs and
says how to fix what is wrong; see [TROUBLESHOOTING.md](TROUBLESHOOTING.md).

## Turning the service off

The package turns the service on for every user, so `systemctl --user
disable` does not keep it off. To keep it off for yourself:

```
systemctl --user mask --now nexusd.service
```

and `systemctl --user unmask nexusd.service` to have it back.

## Uninstalling

Removing the package stops the service for everyone logged in, turns it
off, and removes the udev rule. Your layouts stay in
`~/.local/state/nexusd`; delete that folder to remove them too.

- Arch: `sudo pacman -R nexus-open`
- Debian and Ubuntu: `sudo apt remove nexus-open`
- Fedora: `sudo dnf remove nexus-open`
- Tarball: `./install.sh --uninstall` from the unpacked folder. If you
  installed the udev rule, it prints the command to remove that too:
  `sudo rm /etc/udev/rules.d/70-nexus-open.rules && sudo udevadm control --reload`

After removal, you keep access to the strip until you unplug it or log
out; nothing uses it once the service has stopped.
