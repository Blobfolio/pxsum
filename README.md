# pxsum

Pxsum is an x86-64 unix CLI tool for quickly calculating and verifying checksums corresponding to the _decoded pixel data_ within image files, making it possible to evaluate visual equality independent of factors like format, encoding, and metadata.

| statler.png | statler.webp | waldorf.png |
| ----------- | ------------ | ----------- |
| <img src="https://github.com/Blobfolio/pxsum/raw/master/skel/assets/statler.png" width="80" height="80" alt="Statler"></img> | <img src="https://github.com/Blobfolio/pxsum/raw/master/skel/assets/statler.webp" width="80" height="80" alt="Statler"></img> | <img src="https://github.com/Blobfolio/pxsum/raw/master/skel/assets/waldorf.png" width="80" height="80" alt="Waldorf"></img> |
| `e20bf1e38053…` | `e20bf1e38053…` | `c0323c3e2bc4…` |

One of these is not like the others…



## How It Works

Pxsum reads and decodes each image into a neutral RGBA bitmap format, hashing the results with the fast and secure [BLAKE3](https://github.com/BLAKE3-team/BLAKE3/) algorithm.

Because most encoders, even "lossless" ones, treat invisble pixels as fair game for palette optimization, pxsum by default hashes their _positions_ rather than color data, ensuring consistent checksums.

_If a color can't be seen, is it really a color?_

To enable _true_ lossless comparisons, pass the `--strict` flag. In this mode, all colors, even invisible ones, will get factored into the checksums.

| statler.png | statler.webp | waldorf.png |
| ----------- | ------------ | ----------- |
| <img src="https://github.com/Blobfolio/pxsum/raw/master/skel/assets/statler.png" width="80" height="80" alt="Statler"></img> | <img src="https://github.com/Blobfolio/pxsum/raw/master/skel/assets/statler.webp" width="80" height="80" alt="Statler"></img> | <img src="https://github.com/Blobfolio/pxsum/raw/master/skel/assets/waldorf.png" width="80" height="80" alt="Waldorf"></img> |
| `dde6d8e12be9…` | `2f62cb0941ce…` | `0b37c6af4734…` |

Evidently WebP took some liberties with the negative space…



## Supported Image Formats

Pxsum can detect and decode (most) images in the following formats:

* AVIF
* BMP
* GIF
* ICO
* JPEG
* JPEG 2000
* JPEG XL
* PNG
* TIFF
* WebP

(If you'd like to see support for something else, just open an [issue](https://github.com/Blobfolio/pxsum/issues).)

Image file paths passed to pxsum must end with extensions associated with these types or they will be silently ignored.

Paths must additionally be valid UTF-8 and may _not_ contain backslashes or weird control characters — like escape, null, line breaks, bell, etc. — or again, they will be silently ignored.



## Basic Usage

Pxsum shares the same basic interface as tools like `md5sum` and `b3sum`, so should feel immediately familiar to anyone who regularly works with checksums.


### Crunching

Image data can be directly piped to pxsum through STDIN and/or passed as file path argument(s).

```bash
# STDIN.
cat image.jpeg | pxsum
cat image.jpeg | pxsum -             # A dash is optional.

# Path(s).
pxsum image.jpeg another.png

# Both.
cat image.jpeg | pxsum - another.png # A dash is mandatory here.

# Crunch a whole directory.
find ~/Pictures -type f -exec pxsum {} +
pxsum -d ~/Pictures                  # This is more efficient.
```

The resulting checksum/path pairs are printed to STDOUT, looking something like this:

```text
fc6e48e935f7b7330cb6bc95c8c725f57e8d9b1efe01b7afc90ea53e9d968aa9  ./assets/ash.jpg
84372180f4763895ff1165487003106774953ad6bce56b5b1344893c52f175c2  ./assets/carl.jpg
2212ead939f81398e078745cbdfeb57f67398c6d3ff16f822449df035bafee0f  ./assets/cmyk.JPG
3c112bc262d2f292adec259c35ad1981088e9cd45ccc5669ada0c9b7efe08559  ./assets/dingo.png
```

If the `-g`/`--group-by-checksum` flag is used, results will instead be grouped by checksum, like this:

```text
2212ead939f81398e078745cbdfeb57f67398c6d3ff16f822449df035bafee0f
  ./assets/cmyk.JPG
2ac10da49b973cd4c44bd3b15cf4e0882f3bfd9bd0edf99bca28b3376ff1a70f
  ./assets/santo.bmp
  ./assets/santo.ico
  ./assets/symto.bmp
3c112bc262d2f292adec259c35ad1981088e9cd45ccc5669ada0c9b7efe08559
  ./assets/dingo.png
```

Either way, the output can be saved to a file for later reference the usual way:

```bash
# Note: use any file extension you like.
pxsum -d ~/Pictures > my-images.chk
pxsum -g -d ~/Pictures > my-images.chk
```

Note that miscellaneous errors and warnings, if any, are printed to STDERR instead of STDOUT, ensuring clean separation from the program's "expected" output.


#### Options

| Short | Long | Value | Description |
| ----- | ---- | ----- | ----------- |
| | `--bench` | | Print the total execution time before exiting. |
| `-d` | `--dir` | Path | Recursively search the directory for image files and pxsum them (along with any other FILE(S)). |
| `-g` | `--group-by-checksum` | | Crunch as usual, but group the results by checksum. Note this will delay output until the end of the run. |
| `-j` | | Number | Limit parallelization to this many threads (instead of giving each logical core its own image to work on). If negative, the value will be subtracted from the total number of logical cores. |
| | `--no-warnings` | | Suppress warnings related to image decoding. |
| | `--only-dupes` | | Same as `-g`/`--group-by-checksum`, but only checksums with two or more matching images will be printed. |
| | `--strict` | | Include color data from invisible pixels in checksum calculations. |


### (Re)Verifying

To (re)verify one or more previously-generated pxsum checksums, use the `-c`/`--check` flag.

In this mode, the output becomes the input…

As with images, manifest data can be directly piped to pxsum through STDIN and/or passed as file path argument(s). 

```bash
# These are all equivalent.
cat my-images.chk | pxsum -c
cat my-images.chk | pxsum -c - # Optional dash.
pxsum -c my-images.chk
pxsum my-images.chk -c         # Order doesn't matter.
```

The line-by-line verification results are printed to STDOUT like this:

```text
./assets/ash.jpg: OK
./assets/dingo.png: OK
./assets/cmyk.JPG: OK
./assets/carl.jpg: FAILED
```

Depending on how it went, a warning may be printed to STDERR at the end:

```text
Warning: 1 computed checksum did NOT match
```

#### Options

The following options are compatible with `-c`/`--check`:

| Short | Long | Value | Description |
| ----- | ---- | ----- | ----------- |
| | `--bench` | | Print the total execution time before exiting. |
| `-j` | | Number | Limit parallelization to this many threads (instead of giving each logical core its own image to work on). If negative, the value will be subtracted from the total number of logical cores. |
| | `--no-warnings` | | Suppress warnings related to malformed check manifest lines. |
| `-q` | `--quiet` | | Suppress OK messages. |



## Exit Codes

In keeping with `md5sum`, _et al_, pxsum emits different exit codes to indicate success or failure independently of the program output.

| Code | Description | Mode |
| ---- | ----------- | ---- |
| **0** | Business as usual! | |
| **1** | Something blew up! | |
| **2** | No checksum/path pairs were outputted. | crunch |
| **3** | One or more images failed to re-verify. | check |



## Installation

Debian and Ubuntu users can just grab the pre-built `.deb` package from the [latest release](https://github.com/Blobfolio/pxsum/releases/latest).

This application is written in [Rust](https://www.rust-lang.org/) and can alternatively be built from source using [Cargo](https://github.com/rust-lang/cargo):

```bash
# Clone the source.
git clone https://github.com/Blobfolio/pxsum.git

# Go to it.
cd pxsum

# Build as usual.
cargo build \
    --bin pxsum \
    --release
```

Note that some of the image decoders — *\*\*cough\*\* JPEG XL \*\*cough\*\** — come with some extra build dependencies of their own. The specifics will vary by system, but you'll probably need `gcc`/`g++` (or Clang), NASM, and make/cmake.

While specifically designed for Linux systems, pxsum can probably be built for other 64-bit Unix platforms like Mac too.



## License

See also: [CREDITS.md](CREDITS.md)

Copyright © 2024 [Blobfolio, LLC](https://blobfolio.com) &lt;hello@blobfolio.com&gt;

This work is free. You can redistribute it and/or modify it under the terms of the Do What The Fuck You Want To Public License, Version 2.

    DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
    Version 2, December 2004
    
    Copyright (C) 2004 Sam Hocevar <sam@hocevar.net>
    
    Everyone is permitted to copy and distribute verbatim or modified
    copies of this license document, and changing it is allowed as long
    as the name is changed.
    
    DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
    TERMS AND CONDITIONS FOR COPYING, DISTRIBUTION AND MODIFICATION
    
    0. You just DO WHAT THE FUCK YOU WANT TO.
