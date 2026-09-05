# Native ABI Smoke Test

The C smoke test only includes the public header and verifies the C compiler's
view of the ABI layout. It does not link to the native library.

With a C11 compiler available, run from the WorkerLib root:

```text
cc -std=c11 -Wall -Wextra -Werror -I include -c tests/c_header_smoke.c -o c_header_smoke.o
cc c_header_smoke.o -o c_header_smoke
```

The generated object and executable are local verification artifacts and must
not be committed.
