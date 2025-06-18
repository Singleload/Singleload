#!/bin/bash
# This script tests security restrictions
echo "Testing security restrictions..."

# Try to access network (should fail)
echo -n "Network test: "
if ping -c 1 google.com &>/dev/null; then
    echo "FAIL - Network is accessible (should be blocked)"
else
    echo "PASS - Network is properly blocked"
fi

# Try to write to system directories (should fail)
echo -n "Filesystem test: "
if touch /etc/test 2>/dev/null; then
    echo "FAIL - Can write to /etc (should be blocked)"
    rm /etc/test 2>/dev/null
else
    echo "PASS - Cannot write to system directories"
fi

# Check capabilities
echo -n "Capabilities test: "
if command -v capsh &>/dev/null; then
    caps=$(capsh --print | grep "Current:" | cut -d' ' -f3)
    if [ "$caps" = "=" ]; then
        echo "PASS - No capabilities"
    else
        echo "WARN - Has capabilities: $caps"
    fi
else
    echo "SKIP - capsh not available"
fi

# Check user
echo -n "User test: "
if [ "$UID" -eq 0 ]; then
    echo "FAIL - Running as root (should be nonroot)"
else
    echo "PASS - Running as non-root user (UID: $UID)"
fi