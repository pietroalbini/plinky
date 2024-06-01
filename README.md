# plinky

> [!CAUTION]
>
> This is just a fun side project of mine. It is probably broken. I don't
> intend to provide any support or compatibility guarantee for it, nor accept
> third party contributors. Use it at your own risk.

plinky is an x86 and x86-64 ELF linker targeting Linux systems you probably
should **not** use.

This is a side project of mine, with the goal of better understanding how
linkers work. I am learning the world of linkers as I develop this, so parts of
the implementation are probably incorrect or badly architected.

As an additional challenge for me, I am trying to develop plinky without
relying on third party dependencies in build scripts and runtime code
(dependency in tests are fine), because why not.

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or
[MIT License](./LICENSE-MIT), at your option.

## Known differences with GNU ld

* The lack of an entry point will result in an error, instead of following [GNU
  ld][ld-entry]'s behavior of setting it to the first byte of `.text`, or to 0
  if no such section exists.

* The stack is marked as non-executable by default, and it can be set back to
  executable with `-z execstack`. Because of this, the presence or lack of a
  `.note.GNU-stack` section is ignored, and `-z noexecstack` does nothing.
  This follows [LLD's behavior][lld-noexecstack].

[ld-entry]: https://sourceware.org/binutils/docs/ld/Entry-Point.html
[lld-noexecstack]: https://github.com/llvm/llvm-project/issues/57009
