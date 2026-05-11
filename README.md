![Logo](https://raw.githubusercontent.com/dejuri/rad/main/logo.png)
# rad

rad — a lightweight source-based package manager for Radrix GNU/Linux and other LFS-built GNU/Linux systems. Manages system packages built from source using TOML package headers, ok?

rad is an abbreviation for Rathrix Automated TOML-packages Header

but when it combines with Slavic God Radogost, who is the God of trade and seafaring,
even easier to call it just rad.

It stays for managing system packages, user ones is better to manage with [nix](https://github.com/NixOS/nix) or other

## Installing GNU hello with rad

![Showcase](https://raw.githubusercontent.com/dejuri/rad/refs/heads/main/preview.gif)

## Dependencies
If you do use LFS, you maybe have got most of those dependencies, but be sure you've got everything, otherwise you'll get an error one day.
* cargo (building rad and runtime)
* make (runtime)
* cmake (runtime)
* meson (runtime)
* ninja (runtime)
* pip (runtime)
* tar (runtime for unpacking tarballs)
* unzip (runtime for non-tar archives)
* git (runtime)
* sh (runtime, can be symlink on other POSIX shell)
* wget (runtime)
* which (runtime, for rad could in checking own dependencies in --info)
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
## Usage/Examples
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
configure_args = ""
```

Understand? And now if you created own packages repository, you can use them already without publishing somewhere and creating repository. Because you can install local packages, just change directory to where your packages are, and do 
```sh
rad -P <pkg>
```
this will give you info for your package, if you see this package and rad give information about local package, you can use it. Try to install it (as root)
```sh
rad -i <pkg>
```
That's all, if no errors, you've installed your package. Now rad remember the installed files of package by the register, you can view
```sh
cat /var/lib/rad/installed/<pkg>
```
you will get the list of installed files. It also supports symlinks, don't worry about it.

Now, if needed, remove the package (as root)
```sh
rad -r <pkg>
```
## Issues
If you have an issue or some different bugs, create a [new issue](https://github.com/dejuri/radpkg/issues/new) please and describe what happened and i will see it
## What now?
You better know why you've installed it. Remember, the code is open, i think you have rights to know what are you installing, yes?

Ok, so what now, you can create your repository of packages, install it, and rad will control it, it is good. This is the stability.

You can just make a system built with rad, a Radrix, and i have a project and done some progress. Good luck, comrade, do what you find needed
## License

[GPL 3.0](https://choosealicense.com/licenses/gpl-3.0/)


