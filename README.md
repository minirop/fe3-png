# FE3 png

Give it a compressed graphics from Fire Emblem 3 and it spits out a PNG file.

## Usage

```console
$ fe3-png <romfile> -a <starting offset> -o <output file.png>
```
*starting offset* can be omitted if it's zero.

## TODO

- Only handles 4bpp images currently
- Allow "decompress only" for non graphics data
- Write a compressor
- Rework the code to deduplicate `0x7`
