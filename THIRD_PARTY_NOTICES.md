# Third-party media tool notices

Pandan's production image redistributes the following pinned media tools for the YouTube Downloads
feature. The image continues to include the license and copyright files installed by Debian under
`/usr/share/doc`.

## yt-dlp 2026.08.19

Pandan uses the official architecture-specific `yt-dlp_linux` and `yt-dlp_linux_aarch64`
PyInstaller executables from the immutable 2026.08.19 release. The project is released under the
Unlicense; the standalone executables also contain Python and other components under their own
licenses. The exact upstream inventory is in yt-dlp's
[`THIRD_PARTY_LICENSES.txt`](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/THIRD_PARTY_LICENSES.txt).

The executable bundles `yt-dlp-ejs`. That project is released under the Unlicense; its prebuilt
artifacts include `meriyah` under ISC and `astring` under MIT. Pandan disables remote component
downloads, plugin discovery, and yt-dlp self-update.

- Source: <https://github.com/yt-dlp/yt-dlp/tree/2026.08.19>
- EJS source: <https://github.com/yt-dlp/ejs>
- Release checksums: <https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/SHA2-256SUMS>

## Deno 2.9.6

Pandan redistributes the official Linux x86_64 and aarch64 Deno 2.9.6 binaries as yt-dlp's
restricted JavaScript challenge runtime. Deno is licensed under MIT.

- Source and license: <https://github.com/denoland/deno/tree/v2.9.6>
- Release assets: <https://github.com/denoland/deno/releases/tag/v2.9.6>

## FFmpeg 7.1.5

The runtime installs Debian trixie's `ffmpeg` package version `7:7.1.5-0+deb13u1`, which also
provides `ffprobe`. FFmpeg is primarily LGPL-2.1-or-later, with GPL-covered optional portions; the
applicable terms for Debian's build and its linked libraries are recorded in the package copyright
files shipped inside the image.

- Debian source package: <https://packages.debian.org/trixie/source/ffmpeg>
- FFmpeg legal information: <https://ffmpeg.org/legal.html>
