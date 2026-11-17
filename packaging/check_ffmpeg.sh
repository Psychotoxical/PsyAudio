#!/usr/bin/env bash
# Checks that an FFmpeg can do everything PsyAudio asks of it, and that FFprobe lies beside it.
# Usage: check_ffmpeg.sh PATH_TO_FFMPEG
set -euo pipefail

ff=${1:?path to ffmpeg}
exe=
case "$ff" in *.exe) exe=.exe ;; esac
probe=$(dirname "$ff")/ffprobe$exe

"$ff" -hide_banner -version | head -n 1
encoders=$("$ff" -hide_banner -encoders)
for e in libmp3lame flac aac alac libopus libvorbis wavpack wmav2 libspeex; do
    if ! grep -qE "^ A[^ ]* +$e( |$)" <<<"$encoders"; then
        echo "::error::FFmpeg has no encoder $e: $ff" >&2
        exit 1
    fi
done
# reading: what symphonia leaves to FFmpeg, and the codecs inside mka, webm, mp4, caf and dff
decoders=$("$ff" -hide_banner -decoders)
for d in aac alac ape opus vorbis wavpack wmav2 wmalossless speex tta ac3 eac3 dca truehd mlp dst amrnb mp3float; do
    if ! grep -qE "^ A[^ ]* +$d( |$)" <<<"$decoders"; then
        echo "::error::FFmpeg has no decoder $d: $ff" >&2
        exit 1
    fi
done
if ! "$ff" -hide_banner -filters | grep -qE "^ [^ ]+ +hdcd +"; then
    echo "::error::FFmpeg has no hdcd filter: $ff" >&2
    exit 1
fi
# PsyAudio pipes raw PCM in through stdin and reads raw PCM back: one second of silence each way
pcm() { head -c 176400 /dev/zero | "$ff" -v error -nostdin -f s16le -ar 44100 -ac 2 -i - "$@" > /dev/null; }
pcm -af hdcd -f s32le -
pcm -c:a libopus -f opus -
pcm -f f32le -
"$probe" -hide_banner -version | head -n 1
echo "FFmpeg can do what PsyAudio needs: $ff"
