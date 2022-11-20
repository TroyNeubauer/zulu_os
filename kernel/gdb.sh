#!/bin/bash
gdb target/x86_64/debug/zulu_os -ex "target remote :1234" \
    "--eval-command=b jmp" \
    "--eval-command=b breakpoint_handler"
