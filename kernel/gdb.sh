#!/bin/bash
if [[ $* == *--flag* ]]
then
    echo "using release"
    FILE="target/x86_64/debug/zulu_os"
else
    FILE="target/x86_64/release/zulu_os"
fi
echo 'got file $FILE'
rust-gdb $FILE -ex "target remote :1234" \
    "--eval-command=b jmp" \
    "--eval-command=b breakpoint_handler" \
    "--eval-command=c" \
    "--eval-command=add-symbol-file processes/userspace_test 0x660000"
