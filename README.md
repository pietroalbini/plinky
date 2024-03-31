# plinky

TODO

## Differences with GNU ld

* The lack of an entry point will result in an error, instead of following [GNU
  ld][ld-entry]'s behavior of setting it to the first byte of `.text`, or to 0
  if no such section exists.

* The stack is marked as non-executable by default, and it can be set back to
  executable with `-z execstack`. Because of this, the presence or lack of a
  `.note.GNU-stack` section is ignored, and `-z noexecstack` does nothing.
  This follows [LLD's behavior][lld-noexecstack].

[ld-entry]: https://sourceware.org/binutils/docs/ld/Entry-Point.html
[lld-noexecstack]: https://github.com/llvm/llvm-project/issues/57009
