#!/usr/bin/env bash
# Builds the FFmpeg that ships with PsyAudio: static, LGPL, with the encoders, readers and the
# HDCD filter PsyAudio uses, plus FFprobe. LAME, Ogg, Vorbis, Opus and Speex are built from their
# sources first and linked in.
# Usage: build_ffmpeg.sh linux|windows|macos OUTPUT_FOLDER
# macos builds on a Mac for its own architecture; linux and windows (mingw-w64) build on Linux.
# Result: OUTPUT_FOLDER/bin/ffmpeg and ffprobe (.exe on Windows), OUTPUT_FOLDER/licenses/
set -euo pipefail

FFMPEG=9.0.2
FFMPEG_SHA256=8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e
# LAME publishes no checksum; this is the SHA256 of the SourceForge tarball as distributions pin it
LAME=3.100
LAME_SHA256=ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e
# Xiph: from the SHA256SUMS files next to the tarballs
OGG=1.3.6
OGG_SHA256=83e6704730683d004d20e21b8f7f55dcb3383cdf84c0daedf30bde175f774638
VORBIS=1.3.7
VORBIS_SHA256=0e982409a9c3fc82ee06e08205b1355e5c6aa4c36bca58146ef399621b0ce5ab
OPUS=1.6.1
OPUS_SHA256=6ffcb593207be92584df15b32466ed64bbec99109f007c82205f0194572411a1
SPEEX=1.2.1
SPEEX_SHA256=4b44d4f2b38a370a2d98a78329fefc56a0cf93d1c1be70029217baae6628feea

FFMPEG_URL=https://ffmpeg.org/releases/ffmpeg-$FFMPEG.tar.xz
LAME_URL=https://downloads.sourceforge.net/project/lame/lame/$LAME/lame-$LAME.tar.gz
OGG_URL=https://downloads.xiph.org/releases/ogg/libogg-$OGG.tar.gz
VORBIS_URL=https://downloads.xiph.org/releases/vorbis/libvorbis-$VORBIS.tar.gz
OPUS_URL=https://downloads.xiph.org/releases/opus/opus-$OPUS.tar.gz
SPEEX_URL=https://downloads.xiph.org/releases/speex/speex-$SPEEX.tar.gz

target=${1:?linux, windows or macos}
mkdir -p "${2:?output folder}"
output=$(cd "$2" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
prefix=$work/prefix
jobs=$(getconf _NPROCESSORS_ONLN)

case "$target" in
    linux)
        host=()
        ff_platform=()
        ff_ldflags=-static
        ;;
    windows)
        host=(--host=x86_64-w64-mingw32)
        ff_platform=(--target-os=mingw32 --arch=x86_64 --cross-prefix=x86_64-w64-mingw32-)
        ff_ldflags=-static
        # Opus adds -D_FORTIFY_SOURCE=2 unless it is set already; its __memcpy_chk and friends
        # are missing from a static mingw-w64 link
        export CPPFLAGS=-D_FORTIFY_SOURCE=0
        ;;
    macos)
        # old config.guess files do not know arm64 Macs: name the build system
        host=(--build="$(uname -m | sed 's/arm64/aarch64/')-apple-darwin")
        ff_platform=()
        ff_ldflags=-mmacosx-version-min=13.0
        export CFLAGS="-O2 -mmacosx-version-min=13.0"
        export LDFLAGS="-mmacosx-version-min=13.0"
        ;;
    *) echo "unknown target: $target" >&2; exit 1 ;;
esac

# fetch URL SHA256: downloads, checks and unpacks into the work folder
fetch() {
    local file
    file=$work/$(basename "$1")
    curl -fsSL --retry 3 -o "$file" "$1"
    echo "$2  $file" | shasum -a 256 -c -
    tar -xf "$file" -C "$work"
}

# lib FOLDER [configure options]: a static library into the prefix
lib() {
    local dir=$work/$1
    shift
    # ${a[@]+"${a[@]}"}: an empty list is an error in macOS' bash 3.2 under set -u
    (cd "$dir" && ./configure --prefix="$prefix" ${host[@]+"${host[@]}"} --enable-static --disable-shared --with-pic "$@" \
        && make -j"$jobs" && make install)
}

fetch "$LAME_URL" "$LAME_SHA256"
fetch "$OGG_URL" "$OGG_SHA256"
fetch "$VORBIS_URL" "$VORBIS_SHA256"
fetch "$OPUS_URL" "$OPUS_SHA256"
fetch "$SPEEX_URL" "$SPEEX_SHA256"
fetch "$FFMPEG_URL" "$FFMPEG_SHA256"

lib "lame-$LAME" --disable-frontend --disable-decoder
lib "libogg-$OGG"
# libvorbis' configure adds a flag current Apple compilers reject
perl -pi -e 's/-force_cpusubtype_ALL//g' "$work/libvorbis-$VORBIS/configure"
lib "libvorbis-$VORBIS" --with-ogg="$prefix" --disable-docs --disable-examples --disable-oggtest
# Opus' run-time checks and stack protector need libssp, which a static mingw link does not have
opus_extra=()
[ "$target" = windows ] && opus_extra=(--disable-hardening --disable-stack-protector)
lib "opus-$OPUS" --disable-doc --disable-extra-programs ${opus_extra[@]+"${opus_extra[@]}"}
lib "speex-$SPEEX" --disable-binaries

encoders=libmp3lame,flac,aac,alac,libopus,libvorbis,wavpack,wmav2,libspeex,pcm_s16le,pcm_s32le,pcm_f32le
muxers=mp3,flac,ipod,opus,ogg,wv,asf,f32le,s16le,s32le
demuxers=aac,ac3,aiff,amr,ape,asf,au,caf,dsf,eac3,flac,iff,matroska,mov,mp3,mpc,mpc8,ogg,tak,tta,w64,wav,wv,f32le,s16le,s32le
decoders=aac,aac_latm,ac3,eac3,alac,amrnb,amrwb,ape,dsd_lsbf,dsd_lsbf_planar,dsd_msbf,dsd_msbf_planar,flac,mp1float,mp2float
decoders+=,mp3float,mpc7,mpc8,opus,speex,tak,tta,vorbis,wavpack,wmalossless,wmapro,wmav1,wmav2,wmavoice,'pcm_*','adpcm_*'
parsers=aac,aac_latm,ac3,flac,mpegaudio,opus,tak,vorbis
filters=aformat,anull,aresample,format,null,hdcd

cd "$work/ffmpeg-$FFMPEG"
# FFmpeg's configure says why a library was not found only in its log: show the end of it
export PKG_CONFIG_LIBDIR=$prefix/lib/pkgconfig
trap 'tail -n 80 ffbuild/config.log 2>/dev/null; rm -rf "$work"' EXIT
./configure ${ff_platform[@]+"${ff_platform[@]}"} \
    --pkg-config=pkg-config --pkg-config-flags=--static \
    --extra-cflags="-I$prefix/include ${CFLAGS:-}" --extra-ldflags="-L$prefix/lib $ff_ldflags" \
    --disable-everything --disable-autodetect --disable-doc --disable-debug --disable-network --disable-iconv \
    --disable-ffplay --enable-ffmpeg --enable-ffprobe --enable-static --disable-shared \
    --enable-avfilter --enable-swresample \
    --enable-libmp3lame --enable-libopus --enable-libvorbis --enable-libspeex \
    --enable-protocol=file,pipe --enable-filter="$filters" \
    --enable-encoder="$encoders" --enable-muxer="$muxers" \
    --enable-demuxer="$demuxers" --enable-decoder="$decoders" --enable-parser="$parsers"
trap 'rm -rf "$work"' EXIT
make -j"$jobs"

exe=
[ "$target" = windows ] && exe=.exe
test -f "ffmpeg$exe" && test -f "ffprobe$exe"
mkdir -p "$output/bin" "$output/licenses"
cp "ffmpeg$exe" "ffprobe$exe" "$output/bin/"
cp COPYING.LGPLv2.1 "$output/licenses/FFmpeg-LGPL-2.1.txt"
cp "$work/lame-$LAME/COPYING" "$output/licenses/LAME-LGPL.txt"
cp "$work/libogg-$OGG/COPYING" "$output/licenses/libogg-BSD.txt"
cp "$work/libvorbis-$VORBIS/COPYING" "$output/licenses/libvorbis-BSD.txt"
cp "$work/opus-$OPUS/COPYING" "$output/licenses/Opus-BSD.txt"
cp "$work/speex-$SPEEX/COPYING" "$output/licenses/Speex-BSD.txt"
cat > "$output/licenses/SOURCES.txt" <<EOF
PsyAudio ships FFmpeg and FFprobe built from these sources with packaging/build_ffmpeg.sh
(https://github.com/Psychotoxical/PsyAudio). Each is under the license in the file next to this one.

FFmpeg $FFMPEG    $FFMPEG_URL
LAME $LAME        $LAME_URL
libogg $OGG       $OGG_URL
libvorbis $VORBIS $VORBIS_URL
Opus $OPUS        $OPUS_URL
Speex $SPEEX      $SPEEX_URL
EOF
