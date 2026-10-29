"""Real UDP/process smoke test; requires cargo build -p conv-native-peer."""
import pathlib
import socket
import subprocess
import tempfile
import time

BINARY = pathlib.Path(__file__).resolve().parents[2] / "target/debug/conv-native-peer"


def port():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_for(path, text, process):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if text in path.read_text():
            return
        if process.poll() is not None:
            raise AssertionError(path.read_text())
        time.sleep(0.05)
    raise AssertionError(f"Timed out waiting for {text!r}: {path.read_text()}")


with tempfile.TemporaryDirectory(prefix="conv-smoke-") as directory:
    root = pathlib.Path(directory)
    a, b = port(), port()
    processes = []
    files = []

    def start(name, local, seed):
        log_path = root / (name + ".log")
        log = log_path.open("a")
        files.append(log)
        process = subprocess.Popen([
            str(BINARY), "--bind", f"127.0.0.1:{local}",
            "--seed", f"127.0.0.1:{seed}", "--state", str(root / (name + ".conv")),
        ], stdin=subprocess.PIPE, stdout=log, stderr=log, text=True)
        processes.append(process)
        return process, log_path

    try:
        alice, alice_log = start("alice", a, b)
        bob, bob_log = start("bob", b, a)
        wait_for(alice_log, "listening", alice)
        wait_for(bob_log, "listening", bob)
        alice.stdin.write("first-edit\n")
        alice.stdin.flush()
        wait_for(bob_log, "first-edit", bob)
        bob.stdin.write("/quit\n")
        bob.stdin.flush()
        bob.wait(timeout=5)
        alice.stdin.write("offline-edit\n")
        alice.stdin.flush()
        bob, bob_log = start("bob", b, a)
        wait_for(bob_log, "offline-edit", bob)
        bob.stdin.write("after-restart\n")
        bob.stdin.flush()
        wait_for(alice_log, "after-restart", alice)
        print("PASS: real UDP replication, offline repair, durable restart, fresh writer")
    finally:
        for process in processes:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        for log in files:
            log.close()
