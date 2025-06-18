import sys
print("This will print to stdout")
print("This will print to stderr", file=sys.stderr)
sys.exit(1)  # Exit with error code