#!/bin/bash
echo "=== System Information ==="
echo "Hostname: $(hostname)"
echo "Kernel: $(uname -r)"
echo "Architecture: $(uname -m)"
echo "Memory info:"
free -h 2>/dev/null || echo "free command not available"
echo "Disk usage:"
df -h / 2>/dev/null || echo "df command not available"
echo "Process info:"
ps aux | head -5