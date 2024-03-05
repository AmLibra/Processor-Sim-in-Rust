#!/bin/bash

# Check if exactly two arguments are provided
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 input_file.json output_file.json"
    exit 1
fi

# Assuming the first argument is the input file and the second is the output file
input_file=$1
output_file=$2

cd cpusim/src &&
cargo run -- "$input_file" "$output_file"
