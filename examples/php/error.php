<?php
echo "Normal output\n";
fwrite(STDERR, "Error output\n");
exit(1);