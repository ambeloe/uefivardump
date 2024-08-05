uefivardump is a tool to dump uefi variables to json (file or stdout). You can run it from efi shell.

use vdd (https://github.com/ambeloe/vdd) for simple comparisons between dumps

v1 dumps are not compatible with v2 and vice-versa

example: uefivardump.efi -vtrue -ftest.json

uefivardumper takes five optional parameters: -v[true/false], -r, -f[filename], -d, and -w.
    -v specifies if the saved/written variables are volatile or not (-vtrue only saves/writes voltatile, -vfalse only saves/writes persistent)
        not specifying the variable will save all variables.
    -f specifies the dump filename (path is relative to the drive uefivardumper is stored on) defaults to - for stdout
    -r reboot to uefi after dump is finished
    -w writes vars in dump to uefi (-f must be specified)
    -d dry run (does everything but actually write the variable and restart)