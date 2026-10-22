"""Bounded USBPcap capture of newly connected devices on one selected root hub.

Starts before connecting the controller; does not capture existing USB devices.
The installed USBPcap tool may display its own administrator prompt.
"""
import argparse
import json
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tool', type=Path, required=True)
    parser.add_argument('--hub', type=int, required=True)
    parser.add_argument('--seconds', type=int, default=300)
    parser.add_argument('--snaplen', type=int, default=65535)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not 1 <= args.hub <= 32 or not 1 <= args.seconds <= 600 or not 512 <= args.snaplen <= 65535:
        parser.error('Hub must be 1..32; duration 1..600 seconds; snaplen 512..65535')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    ready = args.output.with_suffix('.ready.json')
    log = args.output.with_suffix('.tool.txt')
    with args.output.open('xb') as output, log.open('xb') as errors:
        # Stream through stdout so stopping this parent closes the worker pipe.
        # USBPcap's worker stops on the broken pipe rather than leaving a capture.
        child = subprocess.Popen(
            [str(args.tool), '-d', r'\\.\USBPcap' + str(args.hub),
             '--capture-from-new-devices', '-s', str(args.snaplen), '-o', '-'],
            stdin=subprocess.DEVNULL, stdout=output, stderr=errors,
            creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            deadline = time.monotonic() + 20
            while args.output.stat().st_size < 24:
                if child.poll() is not None:
                    raise RuntimeError('USBPcap exited before capture was ready; inspect ' + str(log))
                if time.monotonic() > deadline:
                    raise RuntimeError('USBPcap startup timeout; inspect administrator prompt or ' + str(log))
                time.sleep(.1)
            ready.write_text(json.dumps({'pid': child.pid, 'output': str(args.output.resolve()),
                                         'hub': args.hub}), encoding='utf-8')
            print('CAPTURE_READY ' + str(ready.resolve()), flush=True)
            try:
                child.wait(timeout=args.seconds)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait(timeout=5)
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=5)
    print('CAPTURE_CLOSED bytes=' + str(args.output.stat().st_size), flush=True)


if __name__ == '__main__':
    main()
