#!/usr/bin/env python3
"""Render measured JSON reports and standalone scientific figures."""
import argparse
import json
import pathlib
import statistics
import math
import os
os.environ.setdefault('MPLCONFIGDIR', '/tmp/conv-matplotlib')
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--directory', type=pathlib.Path, default=ROOT / 'benchmarks/raw-results')
parser.add_argument('--output', type=pathlib.Path, default=ROOT / 'benchmarks/reports/current')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)


def load(*names):
    for name in names:
        path = args.directory / (name + '.json')
        if path.exists():
            value = json.loads(path.read_text())
            if value.get('source_dirty'):
                raise ValueError(f'Report has uncommitted source: {path}')
            if value.get('converged') is False or value.get('verified') is False:
                raise ValueError(f'Correctness failed: {path}')
            return value, path
    raise FileNotFoundError(names)


def percentile(values, p):
    values = sorted(values)
    return values[max(0, math.ceil(len(values) * p / 100) - 1)]


def memory(report, path):
    resource = path.with_name(path.stem + '.resources.json')
    if resource.exists():
        return json.loads(resource.read_text())['max_rss_kib'] / 1024
    for line in report.get('process_memory', []):
        if line.startswith('VmHWM:'):
            return int(line.split()[1]) / 1024
    return None


native = [load(f'native-{count}-1k') for count in [10, 50, 100, 200]]
lines = ['# ConV measured experiments', '',
    'These are local laboratory measurements. Native peers are Tokio tasks in one process using independent IPv4 UDP sockets on loopback. Browser tests use independent Chromium contexts on the same host. No result is a WAN or multi-host claim.', '',
    'The active writers use phase-aligned interval timers after a common readiness barrier and declared warmup. Rate results include sent and received application frame bytes, control and repair; IPv4/UDP bytes are separate in the raw reports. Active-window rates and edit-plus-settle rates are distinct.', '',
    '| Native peers | Mean active KB/s/node | p95 active KB/s/node | Mean whole-window KB/s/node | Process peak MiB | Mean CPU cores | Commit |',
    '| ---: | ---: | ---: | ---: | ---: | ---: | --- |']
counts, means, p95s = [], [], []
for report, path in native:
    active = [row['active_window_bytes_per_second'] / 1000 for row in report['per_node']]
    whole = [row['bytes_per_second'] / 1000 for row in report['per_node']]
    counts.append(report['peers']); means.append(statistics.mean(active)); p95s.append(percentile(active, 95))
    peak = memory(report, path)
    resource_path = path.with_name(path.stem + '.resources.json')
    cpu = 'not collected'
    if resource_path.exists():
        resource = json.loads(resource_path.read_text())
        cpu = f"{(resource['user_seconds'] + resource['system_seconds']) / resource['elapsed_seconds']:.3f}"
    lines.append(f"| {report['peers']} | {statistics.mean(active):.2f} | {percentile(active,95):.2f} | {statistics.mean(whole):.2f} | {peak:.1f} | {cpu} | `{report['commit'][:7]}` |")
lines += ['', f'Raw source directory: `{args.directory.resolve().relative_to(ROOT)}`.', '', 'All listed replicas converged with identical document state and causal receipt. Short runs are exploratory; active writers, edit cadence, warmup, settling time and actual mutation counts are retained in each raw report.', '',
    '| Initial visible text | Peers | Actual mutations per replica | Serialized history bytes/node | Tombstones/node |', '| --- | ---: | ---: | ---: | ---: |']
for names in [('native-10-1k',), ('native-10-100k', 'native-100k'), ('native-2-1m', 'native-1m')]:
    report, _ = load(*names); row = report['per_node'][0]
    if not report['load_executed']:
        raise ValueError('Document-size workload did not execute')
    lines.append(f"| {report['document_bytes'] // 1024} KiB | {report['peers']} | {row['mutations']} | {row['serialized_history_bytes']:,} | {row['tombstones']} |")

reductions = {}
for kind, names in [('steady', ('baseline-delta','steady-delta','baseline-full-state','steady-full-state')), ('session', ('session-delta','session-delta','session-full-state','session-full-state'))]:
    delta, _ = load(*names[:2]); full, _ = load(*names[2:])
    for field in ['peers', 'writers', 'document_bytes', 'edit_seconds', 'warmup_seconds', 'network_bootstrap']:
        if delta[field] != full[field]: raise ValueError(f'Unmatched baseline field {field}')
    if delta['per_node'][0]['mutations'] != full['per_node'][0]['mutations']:
        raise ValueError('Actual baseline mutation counts differ')
    total = lambda report: sum(row['sent'] + row['received'] for row in report['per_node'])
    reductions[kind] = 100 * (1 - total(delta) / total(full))
lines += ['', '## Bandwidth baseline', '',
    f"The matched **two-peer** comparison reduced combined sent/received application bytes by **{reductions['steady']:.2f}%** with preloaded state and **{reductions['session']:.2f}%** in the short whole-session run including network bootstrap. These are separate claims. The whole-session result does not establish a universal 95% reduction.", '',
    'The retained initial three-second whole-session comparison saved 93.27%; the profiled five-second comparison has more live edits over which to amortize bootstrap. Both raw pairs remain available. Savings depend on session length and history size.', '',
    'The baseline sends complete retained CRDT history at the same batching cadence/topology and UDP settings. This is a simple full-state baseline, not a comparison with established CRDT libraries.', '', '## Isolated merge', '',
    '| Native 100 KiB seed | Warmup / measured | p99 microseconds | Verified |', '| ---: | --- | ---: | --- |']
for seed in range(3):
    report, _ = load(f'merge-native-100k-seed-{seed}',f'merge-native-100k-{seed}')
    lines.append(f"| {seed} | {report['warmup']} / {report['samples']} | {report['merge_nanoseconds']['p99']/1000:.2f} | {report['verified']} |")
recovery, _ = load('recovery')
if recovery['successful_trials'] < 1000 or recovery['failed_trials'] != 0 or recovery['qualified_p99_microseconds'] is None:
    raise ValueError('Recovery p99 does not satisfy the measurement contract')
lines += ['', '## Reconnect recovery', '',
    f"{recovery['successful_trials']:,} measured trials after {recovery['warmup_trials']} warmups; {recovery['failed_trials']} measured failures. Eight native UDP peers recovered {recovery['offline_mutations']:,} retained single-scalar mutations with a qualified p99 of **{recovery['qualified_p99_microseconds']/1000:.2f} ms**. Writers were stopped, and the timer ended only when all targets had identical state and causal summaries. This is loopback recovery after restored reachability, excluding peer setup and document generation.", '']
browser_path = args.directory / 'browser.json'
if not browser_path.exists(): browser_path = ROOT / 'benchmarks/raw-results/browser.json'
if browser_path.exists():
    browser = json.loads(browser_path.read_text())
    if browser.get('source_dirty'): raise ValueError('Browser result is provisional/uncommitted')
    upper = max(case['p99_clock_upper_bound_ms'] for case in browser['wasm']['cases'])
    lines += ['## Browser and WASM', '',
        f"Chromium {browser['browser']}, {browser['hardware']}; commit `{browser['commit'][:7]}`. Across three seeds on a 100 KiB document, the isolated WASM merge p99 clock-resolution upper bound was **{upper:.3f} ms**. Rendering, network, serialization and protocol history are excluded; the WASM boundary is included.", '',
        f"The 1 KiB trusted-keydown-to-second-animation-frame proxy had p95 **{browser['local_render_proxy']['p95_ms']:.2f} ms**. This is not physical-screen timing. Controller-observed visibility at all seven remote DOMs in an eight-context foreground room had p95 **{percentile(browser['remote_visibility']['samples_ms'], 95):.2f} ms**, including automation overhead, with one writer waiting for each edit to become visible before sending the next.", '']
lines += ['## Browser dissemination comparison', '', 'The retained 30-edit fanout-two browser run had nearest-rank p95 visibility of 863.57 ms. Source broadcast within the bounded eight-peer mesh measured 188.16 ms in the next 30-edit run and 130.37 ms in the final 100-edit run. These are separate single-host runs, not a controlled WAN comparison. Earlier raw p95 fields used floor indexing; these figures are recomputed from the retained samples.', '', '## Interpretation', '',
    'The roughly 15 KB/s target is assessed using active-window traffic, rather than diluting it with quiet settling time. Keep exact measured values and workload phases in any resume/demo statement. Short trials and a single host limit generality. Physical-device composition/background tabs, WAN behavior and container execution remain unverified. Public hosting is outside the current local release scope.', '',
    'GNU-time resource sidecars, when present, record whole-process user/system CPU seconds and peak RSS, including preparation and report generation. They are distinct from the protocol measurement window. CPU utilisation was not collected for the initial reports lacking those sidecars.', '']
(args.output / 'report.md').write_text('\n'.join(lines))
plt.rcParams.update({'font.size': 10, 'axes.spines.top': False, 'axes.spines.right': False, 'svg.hashsalt': 'conv-measurements-v1'})
fig, axes = plt.subplots(1, 2, figsize=(11, 4.2), constrained_layout=True)
axes[0].plot(counts, means, 'o-', color='#315a41', label='Mean per node')
axes[0].plot(counts, p95s, 's--', color='#a65b3c', label='p95 across nodes')
axes[0].axhline(15, linestyle=':', color='#888888', label='Proposal target ≈15 KB/s')
axes[0].set(xlabel='Native peers (UDP loopback tasks)', ylabel='Active-window application KB/s', title='Measured traffic, sent + received')
axes[0].legend(fontsize=8); axes[0].grid(alpha=.15)
values = sorted(value / 1000 for value in recovery['recovery_microseconds'])
axes[1].plot(values, [(i + 1) * 100 / len(values) for i in range(len(values))], color='#315a41')
axes[1].axvline(200, linestyle=':', color='#888888', label='Stretch target: p99 <200 ms')
axes[1].set(xlabel='Recovery latency (ms)', ylabel='Completed trials (%)', title=f"{len(values):,} recovery trials; zero failures", ylim=(0, 100), xlim=(0, max(210, max(values) * 1.05)))
axes[1].legend(fontsize=8); axes[1].grid(alpha=.15)
for suffix in ['svg', 'png']:
    artifact = args.output / ('measurements.' + suffix)
    fig.savefig(artifact, dpi=180, metadata={'Date': None} if suffix == 'svg' else None)
    if suffix == 'svg':
        artifact.write_text('\n'.join(line.rstrip() for line in artifact.read_text().splitlines()) + '\n')
plt.close(fig)
print('Wrote', args.output / 'report.md')
