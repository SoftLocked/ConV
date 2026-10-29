#!/usr/bin/env python3
"""Serial real-process experiments with CPU/RSS sidecars and retained raw results."""
import argparse
import datetime
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--only', choices=['all', 'network', 'merge', 'recovery'], default='all')
parser.add_argument('--quick', action='store_true', help='smoke tests; never qualifies recovery p99')
options = parser.parse_args()
output = ROOT / 'benchmarks/raw-results' / datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')
output.mkdir(parents=True, exist_ok=False)
manifest = []


def run(name, binary, arguments):
    report = output / (name + '.json')
    resource = output / (name + '.resources.json')
    command = [str(ROOT / 'target/release' / binary), *arguments, '--output', str(report)]
    print('Running', name, flush=True)
    measured = ['/usr/bin/time', '-f', '{"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M,"elapsed_seconds":%e}', '-o', str(resource), *command]
    result = subprocess.run(measured, cwd=ROOT, check=False)
    usage = json.loads(resource.read_text().splitlines()[-1])
    usage.update(command=command, returncode=result.returncode, scope='entire child process, including preparation and report serialization')
    resource.write_text(json.dumps(usage, indent=2) + '\n')
    manifest.append(dict(name=name, report=report.name, resources=resource.name, returncode=result.returncode))
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    if result.returncode:
        raise SystemExit(f'Experiment failed; raw result and resources retained in {output}')


if options.only in ['all', 'network']:
    duration, settle = ('1', '3') if options.quick else ('5', '8')
    for peers in ([10] if options.quick else [10, 50, 100, 200]):
        run(f'native-{peers}-1k', 'conv-bench', ['--peers', str(peers), '--writers', '10', '--seconds', duration, '--settle-seconds', settle])
    if not options.quick:
        for seed in [1, 2]:
            run(f'native-200-seed-{seed}', 'conv-bench', ['--peers', '200', '--writers', '10', '--seed', str(seed), '--seconds', duration, '--settle-seconds', settle])
        run('native-100k', 'conv-bench', ['--peers', '10', '--writers', '2', '--seconds', '15', '--settle-seconds', '5', '--document-bytes', '102400'])
        run('native-1m', 'conv-bench', ['--peers', '2', '--writers', '1', '--seconds', '3', '--settle-seconds', '5', '--document-bytes', '1048576'])
    for session in [False, True]:
        for baseline in [False, True]:
            name = ('session' if session else 'steady') + ('-full-state' if baseline else '-delta')
            args = ['--peers', '2', '--writers', '1', '--seconds', duration, '--settle-seconds', settle]
            if session:
                args += ['--network-bootstrap', '--warmup-seconds', '0']
            if baseline:
                args += ['--full-state']
            run(name, 'conv-bench', args)
    run('idle-10', 'conv-bench', ['--peers', '10', '--writers', '0', '--seconds', duration, '--settle-seconds', settle])
    run('deletion-heavy-50', 'conv-bench', ['--peers', '50', '--writers', '10', '--delete-every', '2', '--seconds', duration, '--settle-seconds', settle])
if options.only in ['all', 'merge']:
    for seed in ([0] if options.quick else [0, 1, 2]):
        run(f'merge-native-100k-{seed}', 'conv-merge', ['--seed', str(seed)])
    run('merge-native-deletion-heavy', 'conv-merge', ['--deletion-heavy'])
if options.only in ['all', 'recovery']:
    run('recovery', 'conv-recovery', ['--trials', '10', '--warmup', '2'] if options.quick else [])
print('Retained raw reports:', output)
