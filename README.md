![Logo](https://raw.githubusercontent.com/dejuri/rad/main/logo.png)
# rad

rad is a source-based package manager for Radian GNU/Linux and other LFS-built GNU/Linux systems based on using easy-writable toml package headers. It stands for writing an overlay with ability to edit packages flexible as high as on default LFS. It usually manages system packages built from source using TOML package headers, but you still can install binaries with it

rad is abbreviation for Radian Automated TOML-packages Handler, though it is "rath"

just when it combines with Slavic God Radogost, who is the God of trade and seafaring,
even easier to call it just rad

It stays for managing system packages, user ones is better to manage with [nix](https://github.com/NixOS/nix) or other

## Installing GNU hello with rad

![Showcase](https://raw.githubusercontent.com/dejuri/rad/refs/heads/main/preview.gif)

## Dependencies
If you do use LFS, you maybe have got most of those dependencies, but be sure you've got everything, otherwise you'll get an error one day.
* `cargo` (building rad and runtime)
* `make` (runtime)
* `cmake` (runtime)
* `meson` (runtime)
* `ninja` (runtime)
* `pip` (runtime)
* `tar` (runtime for unpacking tarballs)
* `unzip` (runtime for non-tar archives)
* `git` (runtime)
* `sh` (runtime, can be symlink on other POSIX shell)
* `wget` (runtime)
* `which` (runtime, for rad could in checking own dependencies in --info)
## Installation
Make sure you have installed runtime dependencies

At the first clone rad with git

```sh
git clone https://github.com/dejuri/rad.git Rad
```
Then change directory to just cloned project

```sh
cd Rad
```

Build it with cargo (you might want to firstly execute `cargo update`)

```sh
cargo build --release
```

Then install rad into the system (execute as root)

```sh
cp ./target/release/rad /usr/bin
```

Now you have to create the config file (`/etc/rad/config.toml`), or just copy the example one and edit it (as root)

```sh
mkdir /etc/rad -p
cp ./examples/config.toml /etc/rad/
```

Rad is installed now. Check if all of runtime dependencies are seen by rad (look at DEPENDENCIES section)

```sh
rad -I
```

That's all!

### P.S. If you want you can install rad from rad itself now

```sh
rad -i rad
```


## To get help of the usage

You might execute `rad -h` firstly, to see available arguments and how to use rad properly.
## Examples
Ok, you need now to understand how to describe own package. You can look for the examples in [repository](https://github.com/dejuri/radpkg), or look at this example of hello package, remember, they must be at .toml format, or rad won't find them
```toml
[package]
name = "hello"
version = "2.12.1"
description = "GNU Hello - the classic greeting program"
source = "https://ftp.gnu.org/gnu/hello/hello-2.12.1.tar.gz"

[build]
system = "autotools"
multilib_support = false
depends = ""
configure_args = [ "" ]
```

Understand? And now if you created own packages repository, you can use them already without publishing somewhere and creating repository. This means you can just use local packages. How? In `/etc/rad/config.toml` you can add `overlays` massive in `[repo]` section. Just look:
```toml
# repo section of config.toml
[repo]
url = "https://raw.githubusercontent.com/dejuri/radpkg/main/stable-13"
overlays = [
	"/home/adolf/radpkg/overlay",
	"https://raw.githubusercontent.com/adolfAVGN/radpkg/main"
]
```
You should add local or hosted overlays! This is pretty nice. But don't forget, it must contain `packages.index` in it. Use this script to generate in to generate it fast
```sh
#!/bin/sh
# this is gen-index.sh for generating packages index
find . -mindepth 2 -maxdepth 2 -name '*.toml' \
  | sed 's|^\./||; s|\.toml$||' \
  | sort > packages.index
```

How to create a local overlay? Here, it is pretty easy
```sh
mkdir -p ~/.rad/overlays/ # it is not necessary to make the same directory, it could be in ~/radolf or something
cd ~/.rad/overlays/
```
Then, you should clone repository of rad pkgs, because it already has bunch of packages, why would you write own nvidia-drivers? :)
```sh
git clone https://github.com/dejuri/radpkg radolf # as example
cd radolf/stable-13
```

Now you have the overlay, you can change packages here of default radpkg repository. Run `ls` to see what categories are here
If some packages you dont want to edit, better to remove them for not updating them manually, i sometimes update package headers in repository

If you remove or add new packages in the overlay, be sure to run
```sh
./gen-index.sh
```

This will generate packages.index. This is necessary for rad to search packages in categories. So don't forget this
And after all of these work, you need to add overlay in config, then rad will use it (as root of course)
```toml
[repo]
overlays = [
	"/home/adolf/.rad/overlays/radolf/stable-13"
]
```

So now you understand that you can do overlays very easily. Also if you did it you can tell me, maybe i should correct something :)

Uhhh, but if you don't want overlay, if the basic repo is enabled in config, you already can use it. Try do (as root)
```sh
rad --sync
```
Then the package index will be downloaded and used. Try to check info about some package
```sh
rad -P <atom>
```
This will give you info for your package, if you see this package and rad give information about local package, you can use it. Try to install it (as root)
```sh
rad -i <atom>
```
That's all, if no errors, you've installed your package. Now rad remember the installed files of package by the register, you can view
```sh
cat /var/lib/rad/installed/<atom>
```
You will get the list of installed files. It also supports symlinks, don't worry about it.

Rad, like other basic package managers, can remove installed package (as root)
```sh
rad -r <atom>
```
## Issues
If you have an issue or some different bugs, create a [new issue](https://github.com/dejuri/radpkg/issues/new) please and describe what happened and i will see it
## What now?
You better know why you've installed it. Remember, the code is open, i think you have rights to know what are you installing, yes?

Ok, so what now, you can create your repository of packages, install it, and rad will control it, it is good. This is the stability.

You can just make a system built with rad, a Radian, and i have a project and done some progress. I use it on desktop but it is a hard experience sometimes, don't forget that it is an experimental project for now. But still, good luck, comrade, do what you find needed in this
## License

[GPL 3.0](https://choosealicense.com/licenses/gpl-3.0/)


