# plinky

TODO

## Differences with GNU ld

* The entry point will not be set to the first byte of `.text` or to 0 if no
  entry point can be otherwise found. Instead, an error will be returned.

* The stack is marked as non-executable by default, and it can be set back to
  executable with the `-z execstack` flag. Because of this, the presence or
  lack of a `.note.GNU-stack` section is ignored, and the `-z noexecstack` does
  nothing. This follows [LLD's behavior][lld-noexecstack].

[lld-noexecstack]: https://github.com/llvm/llvm-project/issues/57009
