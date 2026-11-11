#!/usr/bin/env python3
"""Cross-correlation shift curve: for each frame i in [a,b), measure the
(x,y) integer shift of test frame i relative to test frame a (grayscale,
half-Hanning-windowed NCC via FFT), and the same for ref frames.
Prints two curves side by side.

Usage: shift_curve.py REF_PNG_DIR_OR_MP4 TEST_PNG_DIR_OR_MP4 --start A --end B [--size 1280x720]
PNG dirs use frame_%04d.png naming (opencat --frames output).
"""
import argparse, os, re, subprocess
import numpy as np

def load_idx(path, idx, w, h):
    if os.path.isdir(path):
        p = os.path.join(path, f"frame_{idx:04d}.png")
        from PIL import Image
        return np.asarray(Image.open(p).convert("RGB").resize((w, h)), np.float32)
    expr = f"eq(n\,{idx})"
    r = subprocess.run(["ffmpeg","-v","error","-i",path,"-vf",f"select='{expr}',scale={w}:{h}",
        "-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"],
        stdout=subprocess.PIPE, check=True)
    a = np.frombuffer(r.stdout, np.uint8)
    return a.reshape(h, w, 3).astype(np.float32)

def ncc_shift(a, b, search=48):
    """integer shift of b relative to a via FFT NCC on gray, mean-subtracted."""
    ga = a.mean(axis=2); gb = b.mean(axis=2)
    ga = ga - ga.mean(); gb = gb - gb.mean()
    H, W = ga.shape
    F = np.fft.rfft2(gb) * np.conj(np.fft.rfft2(ga))
    n = np.fft.irfft2(F / (np.abs(F) + 1e-6), s=(H, W))
    n = n[:search*2+1, :search*2+1]
    # normalize by local energy for true NCC
    ea = np.fft.irfft2(np.fft.rfft2(ga**2) * np.ones_like(np.fft.rfft2(gb))[:H,:W//2+1]*0+0)  # unused
    iy, ix = np.unravel_index(np.argmax(n), n.shape)
    if iy > search: iy -= (2*search+1)
    if ix > search: ix -= (2*search+1)
    return int(ix), int(iy), float(n.max())

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref"); ap.add_argument("test")
    ap.add_argument("--start", type=int, required=True)
    ap.add_argument("--end", type=int, required=True)
    ap.add_argument("--size", default="1280x720")
    ap.add_argument("--search", type=int, default=48)
    args = ap.parse_args()
    w, h = map(int, args.size.split("x"))
    base_r = load_idx(args.ref, args.start, w, h)
    base_t = load_idx(args.test, args.start, w, h)
    print(f"frame | ref shift(x,y) ncc | test shift(x,y) ncc")
    for i in range(args.start+1, args.end):
        fr = load_idx(args.ref, i, w, h)
        ft = load_idx(args.test, i, w, h)
        sr = ncc_shift(base_r, fr, args.search)
        st = ncc_shift(base_t, ft, args.search)
        print(f"f{i:04d} | ref {sr[0]:+4d},{sr[1]:+4d} ncc={sr[2]:.3f} | test {st[0]:+4d},{st[1]:+4d} ncc={st[2]:.3f}")

if __name__ == "__main__":
    main()
