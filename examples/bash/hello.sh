#!/bin/bash
echo "Hello from Bash in Singleload!"
echo "Bash version: $BASH_VERSION"
echo "Current user: $USER"
echo "Working directory: $(pwd)"
echo "Available commands: $(ls /usr/local/bin | tr '\n' ' ')"