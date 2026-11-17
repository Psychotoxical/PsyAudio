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
if ! "$ff" -hide_banner -filters | grep -qE "^ [^ ]+ +hdcd +"; then
    echo "::error::FFmpeg has no hdcd filter: $ff" >&2
    exit 1
fi
"$probe" -hide_banner -version | head -n 1
echo "FFmpeg can do what PsyAudio needs: $ff"
