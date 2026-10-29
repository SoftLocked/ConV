#!/usr/bin/env python3
"""Exercise every native writer and retain failures as well as successes."""
import argparse
import datetime
import json
import os
import pathlib
import signal
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--peers', type=int, default=200)
parser.add_argument('--seconds', type=int, default=10)
parser.add_argument('--settle-seconds', type=int, default=30)
parser.add_argument('--edits-per-second', type=int, default=5)
parser.add_argument('--seeds', default='0,1,2')
parser.add_argument('--cases', nargs='+', choices=['healthy', 'lossy-partition', 'deletion-heavy'], default=['healthy', 'lossy-partition', 'deletion-heavy'])
parser.add_argument('--min-load-fraction', type=float, default=.95)
parser.add_argument('--max-rss-mib', type=float)
args = parser.parse_args()
if not 2 <= args.peers <= 500 or args.seconds < 4 or args.settle_seconds < 1 or not 0 < args.min_load_fraction <= 1:
    parser.error('Use 2..500 peers, at least four editing seconds, a positive settling window and a load fraction in (0,1]')
seeds = [int(value) for value in args.seeds.split(',')]
if not seeds or any(seed < 0 for seed in seeds):
    parser.error('Supply one or more nonnegative seeds')
output = ROOT / 'benchmarks/stress-results' / datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')
output.mkdir(parents=True, exist_ok=False)
manifest = []


def run(case, seed):
    name = f'{case}-seed-{seed}'
    report_path = output / (name + '.json')
    resource_path = output / (name + '.resources.json')
    command = [str(ROOT / 'target/release/conv-bench'), '--peers', str(args.peers), '--writers', str(args.peers),
               '--edits-per-second', str(args.edits_per_second), '--seconds', str(args.seconds),
               '--settle-seconds', str(args.settle_seconds), '--seed', str(seed), '--output', str(report_path)]
    if case == 'lossy-partition':
        command += ['--drop-per-million', '10000', '--partition-start-seconds', str(max(1, args.seconds // 4)),
                    '--partition-seconds', str(max(1, args.seconds // 3))]
    if case == 'deletion-heavy':
        command += ['--delete-every', '3']
    measured = ['/usr/bin/time', '-f', '{"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M,"elapsed_seconds":%e}', '-o', str(resource_path), *command]
    print('Running', name, flush=True)
    process = subprocess.Popen(measured, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True)
    timed_out = False
    try:
        log, _ = process.communicate(timeout=args.seconds + args.settle_seconds + 180)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGTERM)
        try:
            log, _ = process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            log, _ = process.communicate()
    (output / (name + '.log')).write_text(log)
    usage = None
    if resource_path.exists() and resource_path.read_text().strip():
        try:
            usage = json.loads(resource_path.read_text().splitlines()[-1])
            usage.update(command=command, returncode=process.returncode, scope='whole process including preparation, settling and report serialization')
            resource_path.write_text(json.dumps(usage, indent=2) + '\n')
        except json.JSONDecodeError:
            pass
    failures = []
    if process.returncode or timed_out:
        failures.append('timeout' if timed_out else f'process exit {process.returncode}')
    report = json.loads(report_path.read_text()) if report_path.exists() else {}
    if not report.get('verified'):
        failures.append('canonical state, complete history or writer participation failed')
    if report.get('source_dirty', True):
        failures.append('source was not committed')
    fraction = report.get('generated_live_mutations', 0) / max(1, report.get('requested_live_mutations', 0))
    if fraction < args.min_load_fraction:
        failures.append(f'achieved {fraction:.1%} of requested load; required {args.min_load_fraction:.1%}')
    rows = report.get('per_node', [])
    if len(rows) != args.peers or any(row.get('local_mutations', 0) == 0 for row in rows):
        failures.append('every peer must actually write')
    loss = sum(row.get('loss_dropped_frames', 0) for row in rows)
    partition = sum(row.get('partition_dropped_frames', 0) for row in rows)
    if case == 'lossy-partition' and (not loss or not partition):
        failures.append('configured faults did not execute')
    if case == 'deletion-heavy' and not any(row.get('tombstones', 0) for row in rows):
        failures.append('delete workload did not execute')
    if usage is None:
        failures.append('resource measurement missing')
    elif args.max_rss_mib and usage['max_rss_kib'] / 1024 > args.max_rss_mib:
        failures.append('RSS budget exceeded')
    entry = dict(case=case, seed=seed, report=report_path.name, resources=resource_path.name, returncode=process.returncode,
                 timed_out=timed_out, verified=report.get('verified', False), writers=report.get('writers'),
                 generated=report.get('generated_live_mutations', 0), requested=report.get('requested_live_mutations', 0),
                 achieved_load_fraction=fraction, loss_drops=loss, partition_drops=partition,
                 max_rss_mib=usage['max_rss_kib'] / 1024 if usage else None, passed=not failures, failures=failures)
    manifest.append(entry)
    (output / 'manifest.json').write_text(json.dumps(dict(configuration=vars(args), runs=manifest), indent=2) + '\n')
    print(('PASS' if not failures else 'FAIL'), name, f'load={fraction:.1%}', '; '.join(failures), flush=True)


for case in args.cases:
    for seed in seeds:
        run(case, seed)
lines = ['# All-writer native stress results', '',
         'Each peer is a Tokio task with its own real UDP loopback socket. Every peer writes. These are single-host engine/transport stress checks, not browser or WAN capacity claims.', '',
         f'Requested {args.peers} writers × {args.edits_per_second} single-scalar mutations/s for {args.seconds} seconds; settling deadline {args.settle_seconds} seconds. Minimum accepted offered-load fraction: {args.min_load_fraction:.0%}.', '',
         'Pass requires identical full CRDT state and causal summaries, every generated mutation retained by every replica, every writer participating, and the configured load/faults actually executing. Failed runs are retained.', '',
         '| Case / seed | Generated / requested | Load achieved | Complete convergence | Injected loss / partition drops | Peak RSS MiB | Result |',
         '| --- | ---: | ---: | --- | ---: | ---: | --- |']
for entry in manifest:
    memory = f"{entry['max_rss_mib']:.1f}" if entry['max_rss_mib'] is not None else 'unavailable'
    lines.append(f"| {entry['case']} / {entry['seed']} | {entry['generated']} / {entry['requested']} | {entry['achieved_load_fraction']:.1%} | {entry['verified']} | {entry['loss_drops']} / {entry['partition_drops']} | {memory} | {'PASS' if entry['passed'] else 'FAIL'} |")
    if entry['failures']:
        lines.append('\nFailure: ' + '; '.join(entry['failures']) + '\n')
lines += ['', 'Loss is seeded application-level outbound frame discard. The partition cuts communication between two halves during editing; both halves continue writing. Accepted packets still travel over real UDP. There is no emulated RTT, process crash or multi-host network in this suite.', '',
          'CPU/RSS sidecars cover the entire child process. Canonical checks happen after writers stop and the settling deadline expires. A pass establishes this workload and deadline, not unbounded history growth, arbitrary WAN behavior or browser room capacity.', '']
(output / 'report.md').write_text('\n'.join(lines))
print('Retained stress results:', output)
raise SystemExit(0 if all(entry['passed'] for entry in manifest) else 1)
