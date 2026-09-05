#!/usr/bin/env python3
"""Repeatable immediate-reply latency measurement; also supplies the test LSP server.

Example: python3 scripts/daemon_latency.py --binary target/debug/lsp-cli \
    --binary /tmp/lsp-cli-baseline --samples 100 --pipeline 16
Uses isolated temporary configuration and sockets, with playground/rust as workspace.
"""

import argparse
import contextlib
import json
import math
import os
from pathlib import Path
import shlex
import socket
import subprocess
import sys
import tempfile
import time


def read_message(reader):
    length = None
    while line := reader.readline():
        if line in (b"\r\n", b"\n"):
            break
        name, value = line.split(b":", 1)
        if name.lower() == b"content-length":
            length = int(value)
    if length is None:
        return None
    return json.loads(reader.read(length))


def write_message(writer, message):
    body = json.dumps(message, separators=(",", ":")).encode()
    writer.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
    writer.flush()


def serve():
    hang_shutdown = False
    while message := read_message(sys.stdin.buffer):
        method = message.get("method")
        if method == "exit":
            return
        if "id" not in message or method is None:
            continue
        result = message.get("params")
        response = {"jsonrpc": "2.0", "id": message["id"]}
        if method == "initialize":
            if (message.get("params", {}).get("initializationOptions") or {}).get("fail"):
                response["error"] = {"code": -32603, "message": "test initialization failure"}
            else:
                result = {"capabilities": {}, "serverInfo": {"name": "latency-fixture", "version": str(os.getpid())}}
        elif method == "shutdown":
            if hang_shutdown:
                continue
            result = None
        elif method == "latency/hangShutdown":
            hang_shutdown = True
            result = None
        elif method == "latency/register":
            write_message(sys.stdout.buffer, {"jsonrpc": "2.0", "id": "registration", "method": "client/registerCapability", "params": {"registrations": []}})
        if "error" not in response:
            response["result"] = result
        write_message(sys.stdout.buffer, response)


class Peer:
    def __init__(self, reader, writer):
        self.reader = reader
        self.writer = writer
        self.sequence = 0

    def send(self, method, params=None):
        self.sequence += 1
        write_message(self.writer, {"jsonrpc": "2.0", "id": self.sequence, "method": method, "params": params})
        return self.sequence

    def notify(self, method):
        write_message(self.writer, {"jsonrpc": "2.0", "method": method, "params": {}})

    def receive(self, request_id):
        while message := read_message(self.reader):
            if message.get("method") and "id" in message:
                write_message(self.writer, {"jsonrpc": "2.0", "id": message["id"], "result": None})
                continue
            if message.get("id") == request_id:
                return message
            raise AssertionError(f"unexpected response: {message}")
        raise AssertionError("peer closed before replying")

    def request(self, method, params=None):
        return self.receive(self.send(method, params))

    def initialize(self, workspace, capabilities=None, fail=False):
        response = self.request("initialize", {
            "rootUri": workspace.as_uri().rstrip("/") + "/", "capabilities": capabilities or {},
            "initializationOptions": {"fail": fail},
        })
        if not fail:
            assert "result" in response, response
            self.notify("initialized")
            return response["result"]["serverInfo"]["version"]
        assert "error" in response, response

    def finish(self):
        assert self.request("shutdown")["result"] is None
        self.notify("exit")


@contextlib.contextmanager
def connect(path):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(10)
        stream.connect(str(path))
        with stream.makefile("rb") as reader, stream.makefile("wb") as writer:
            yield Peer(reader, writer)


@contextlib.contextmanager
def raw_connection(path):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(5)
        stream.connect(str(path))
        yield stream


def handshake_smoke(path, peer):
    """Exercise the independent handshake readers outside the latency sample interval."""
    import threading

    with raw_connection(path) as silent, raw_connection(path) as trickling:
        trickling.sendall(b"Content-Length: 10000\r\n\r\n")
        stopped = threading.Event()

        def trickle():
            while not stopped.wait(.01):
                try:
                    trickling.sendall(b" ")
                except OSError:
                    return

        writer = threading.Thread(target=trickle)
        writer.start()
        try:
            for index in range(20):
                assert peer.request("latency/echo", index)["result"] == index
            # Receiving EOF verifies absolute expiry despite bytes arriving on every read.
            assert trickling.recv(1) == b"", "trickling peer did not expire"
            assert silent.recv(1) == b"", "silent peer did not expire"
            assert peer.request("latency/echo", "after-expiry")["result"] == "after-expiry"
        finally:
            stopped.set()
            writer.join(timeout=5)
            assert not writer.is_alive(), "trickle writer did not finish"


def report(peer, samples, pipeline):
    def percentile(values, fraction):
        return round(sorted(values)[max(0, math.ceil(len(values) * fraction) - 1)], 3)

    results = {}
    for width in (1, pipeline):
        elapsed = []
        for _ in range(samples):
            pending = []
            for index in range(width):
                started = time.perf_counter_ns()
                pending.append((peer.send("latency/echo", index), index, started))
            for request_id, expected, started in pending:
                assert peer.receive(request_id)["result"] == expected
                elapsed.append((time.perf_counter_ns() - started) / 1_000_000)
        results[f"pipeline_{width}"] = {
            "samples": len(elapsed), "p50_ms": percentile(elapsed, .5),
            "p95_ms": percentile(elapsed, .95), "p99_ms": percentile(elapsed, .99),
        }
    return results


def benchmark(binary, workspace, samples, pipeline, check_handshakes):
    with tempfile.TemporaryDirectory(prefix="lsp-latency-") as temporary:
        root = Path(temporary)
        data = root / "data"
        (data / "filetypes").mkdir(parents=True)
        (data / "lsp").mkdir()
        (data / "filetypes" / "rust.yaml").write_text('extensions: ["rs"]\n')
        command = shlex.join([sys.executable, str(Path(__file__).resolve()), "--server"])
        (data / "lsp" / "latency.yaml").write_text(
            'filetypes: ["rust"]\nroot_markers: ["Cargo.toml"]\nname: latency-fixture\n'
            f'cmdline: {json.dumps(command)}\nwait-for-index: false\n'
        )
        (data / "lsp-cli.yaml").write_text("download: false\ndetach: false\n")
        env = {**os.environ, "LSP_DATA": str(data), "XDG_CONFIG_HOME": str(root / "config"),
               "XDG_RUNTIME_DIR": str(root / "run")}
        args = [str(binary), "daemon", str(workspace), "--lsp", "latency-fixture", "--idle-timeout", "30"]
        stderr = (root / "daemon.stderr").open("w+")
        daemon = subprocess.Popen(args, env={**env, "LSP_CLI_DAEMON_BACKGROUND": "1"},
                                  stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=stderr, text=True)
        status = daemon.stdout.readline().strip()
        payload = daemon.stdout.readline().strip()
        if status != "READY":
            daemon.wait(timeout=5)
            stderr.seek(0)
            raise RuntimeError(f"{status}: {payload}\n{stderr.read()}")
        path = Path(payload)
        try:
            with connect(path) as peer:
                pid = peer.initialize(workspace)
                with connect(path) as busy:
                    assert "error" in busy.request("initialize", {"capabilities": {}})
                peer.finish()
            # Session cleanup is asynchronous; keep it outside the measured request interval.
            time.sleep(.1)
            with connect(path) as peer:
                assert peer.initialize(workspace) == pid, "warm upstream was not reused"
                measurements = report(peer, samples, pipeline)
                if check_handshakes:
                    handshake_smoke(path, peer)
                peer.request("latency/register")
                peer.finish()
            time.sleep(.1)
            with connect(path) as peer:
                assert peer.initialize(workspace) != pid, "dynamic registration did not restart upstream"
                peer.finish()
            time.sleep(.1)
            with connect(path) as peer:
                pid = peer.initialize(workspace, {"window": {"workDoneProgress": True}})
                peer.finish()
            time.sleep(.1)
            with connect(path) as peer:
                assert peer.initialize(workspace) != pid, "capability mismatch did not restart upstream"
                peer.finish()
            time.sleep(.1)
            with connect(path) as peer:
                peer.initialize(workspace, fail=True)
                peer.notify("exit")
            time.sleep(.1)
            with connect(path) as peer:
                peer.initialize(workspace)
                peer.request("latency/echo", "recovered")
                peer.finish()
            time.sleep(.1)
            for _ in range(2):
                query = subprocess.run([str(binary), "server-capabilities", str(workspace),
                                        "--lsp", "latency-fixture", "--detach"],
                                       env=env, capture_output=True, text=True, timeout=15, check=True)
                assert "latency-fixture" in query.stdout, "capability command returned no server information"
                time.sleep(.1)
            with connect(path) as peer:
                peer.initialize(workspace)
                peer.request("latency/hangShutdown")
                # A silent connection must not delay a later stop control connection.
                with raw_connection(path):
                    started = time.monotonic()
                    subprocess.run([str(binary), "stop", str(workspace), "--lsp", "latency-fixture"],
                                   env=env, capture_output=True, timeout=15, check=True)
                    assert time.monotonic() - started < 5, "daemon stop exceeded lifecycle deadline"
            assert not path.exists(), "daemon socket survived stop"
            return measurements
        except Exception:
            stderr.seek(0)
            print(stderr.read(), file=sys.stderr)
            raise
        finally:
            if path.exists():
                subprocess.run([str(binary), "stop", str(workspace), "--lsp", "latency-fixture"],
                               env=env, capture_output=True, timeout=15, check=True)
            daemon.wait(timeout=10)
            daemon.stdout.close()
            stderr.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", action="store_true", help="Run the immediate-reply test LSP server on stdio")
    parser.add_argument("--binary", type=Path, action="append", help="Binary to measure (repeat to compare builds)")
    parser.add_argument("--skip-handshake-checks", action="store_true",
                        help="Skip absolute handshake deadline checks when comparing binaries predating this feature")
    parser.add_argument("--workspace", type=Path, default=Path(__file__).resolve().parents[1] / "playground/rust")
    parser.add_argument("--samples", type=int, default=100, help="Number of batches at each pipeline width")
    parser.add_argument("--pipeline", type=int, default=16, help="Requests per pipelined batch")
    args = parser.parse_args()
    if args.server:
        serve()
        return
    if args.samples < 1 or args.pipeline < 2:
        parser.error("samples must be positive and pipeline must be at least 2")
    workspace = args.workspace.resolve()
    with subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "--server"],
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE) as server:
        direct = Peer(server.stdout, server.stdin)
        direct.initialize(workspace)
        print(json.dumps({"direct": report(direct, args.samples, args.pipeline)}), flush=True)
        direct.finish()
        server.wait(timeout=5)
    for binary in args.binary or [Path("target/debug/lsp-cli")]:
        print(json.dumps({str(binary): benchmark(binary.resolve(), workspace, args.samples, args.pipeline, not args.skip_handshake_checks)}), flush=True)


if __name__ == "__main__":
    main()
