Openat (ct) Crate
=================

**Status: Beta**

[Documentation](https://docs.rs/openat_ct) |
[Github](https://github.com/cehteh/openat) |
[Crate](https://crates.io/crates/openat_ct)


The interface to ``openat``, ``symlinkat``, and other functions in ``*at``
family.

About this Fork
===============

This is a fork of the original 'openat' at https://github.com/tailhook/openat

The objective is to fix existing Issues and add features as required. An eventual goal would
be to get this merged back into upstream. In most cases the API should stay backward
compatible with the original openat (Unless functionality and fixes dictate changes).

It can be used as drop in replacement by aliasing the import:

    use openat_ct as openat;


Dependent crates
================

This crate is a thin wrapper for the underlying system calls.
You may find the extension methods in [openat-ext](https://crates.io/crates/openat-ext) useful.

License
=======

Licensed under either of

* Apache License, Version 2.0,
  (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)
  at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
