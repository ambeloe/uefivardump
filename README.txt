uefivardump is a tool to dump uefi variables to json (file or stdout). You can run it from efi shell.

use vdd (https://github.com/ambeloe/vdd) for simple comparisons between dumps

example: uefivardump.efi -vtrue -ftest.json

uefivardumper takes three optional parameters: -v[true/false], -r,and -f[filename].
    -v specifies if the saved variables are volatile or not (-vtrue only saves voltatile, -vfalse only saves persistent)
        not specifying the variable will save all variables.
    -f specifies the output filename (path is relative to the drive uefivardumper is stored on) defaults to - for stdout
    -r reboot to uefi after dump is finished
