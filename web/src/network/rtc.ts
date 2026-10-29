import { neighbors, ordered, distance, LINK_LIMIT, ROOM_LIMIT, type Member } from './topology';
export type Person = { id: string; name: string; order: number; direct: boolean; connected: boolean; relay?: boolean };
type Entry = { pc: RTCPeerConnection; channel?: RTCDataChannel; control?: RTCDataChannel; presence?: RTCDataChannel; early: RTCIceCandidateInit[]; queue: Uint8Array[]; queued: number; controlQueue: Uint8Array[]; controlQueued: number; cursors: Map<string, string> };
type Cursor = { kind: 'cursor'; origin: string; sequence: number; cursor: number[]; hops: number };
type Routed = { kind: 'route'; origin: string; sequence: number; to: string; frame: number[]; hops: number };
export class Mesh {
  private socket?: WebSocket;
  private peers = new Map<string, Entry>();
  private people = new Map<string, Person>();
  private signaled = new Set<string>();
  private ownOrder = 0;
  private stopped = false;
  private configuration: RTCConfiguration = {};
  private retry?: ReturnType<typeof setTimeout>;
  private topologyTimer?: ReturnType<typeof setTimeout>;
  private cursorTimer?: ReturnType<typeof setTimeout>;
  private pendingCursor?: number[];
  private lastCursor = '';
  private cursorSent = 0;
  private sequence = 0;
  private seen = new Map<string, number>();
  onPeople: (people: Person[]) => void = () => {};
  onFrame: (frame: Uint8Array) => void = () => {};
  onCursor: (id: string, cursor: number[]) => void = () => {};
  onStatus: (status: string) => void = () => {};
  constructor(private room: string, private id: string, private name: string) {}
  private members(): Member[] { return [{ id: this.id, order: this.ownOrder }, ...this.people.values()]; }
  async start() { await this.refreshConnectivity(); this.signal(); }
  private async refreshConnectivity() {
    try {
      const response = await fetch(import.meta.env.VITE_ICE_URL ?? '/ice', { signal: AbortSignal.timeout(5_000) });
      if (!response.ok) throw new Error('Unable to obtain connectivity credentials');
      this.configuration.iceServers = (await response.json()).iceServers;
    } catch {
      this.configuration.iceServers ??= [{ urls: 'stun:stun.l.google.com:19302' }];
      this.onStatus(navigator.onLine ? 'Connectivity service unavailable' : 'Offline');
    }
    if (new URLSearchParams(location.search).has('relay')) this.configuration.iceTransportPolicy = 'relay';
  }
  private signal() {
    if (this.stopped || (this.socket && this.socket.readyState < WebSocket.CLOSING)) return;
    this.onStatus('Connecting');
    const url = new URL(import.meta.env.VITE_SIGNAL_URL ?? '/signal', location.href);
    url.protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    url.search = new URLSearchParams({ room: this.room, session: this.id, name: this.name }).toString();
    const socket = new WebSocket(url); this.socket = socket;
    socket.onopen = () => {
      if (this.socket !== socket) { socket.close(); return; }
      this.onStatus('Online');
      for (const [id, entry] of this.peers) {
        if (entry.pc.signalingState === 'have-local-offer') this.sendSignal(id, { description: entry.pc.localDescription });
        else if (this.id < id && ['failed', 'disconnected'].includes(entry.pc.connectionState)) entry.pc.restartIce();
      }
    };
    socket.onclose = () => {
      if (this.socket !== socket || this.stopped) return;
      this.onStatus('Signaling unavailable'); this.retry = setTimeout(() => this.signal(), 1_500);
    };
    socket.onmessage = async ({ data }) => {
      if (this.socket !== socket || this.stopped) return;
      try {
        const message = JSON.parse(String(data));
        if (message.type === 'capacity') { this.stopped = true; this.onStatus(`Room is full (${ROOM_LIMIT} participants)`); socket.close(); return; }
        if (message.type === 'welcome') {
          this.ownOrder = message.order;
          this.signaled = new Set(message.peers.map((person: Person) => person.id));
          for (const [id] of this.people) if (!this.signaled.has(id) && this.peers.get(id)?.channel?.readyState !== 'open') this.people.delete(id);
          for (const person of message.peers) this.rememberPerson(person);
          this.scheduleTopology();
        }
        if (message.type === 'join') { this.rememberPerson(message.peer); this.scheduleTopology(); }
        // Metadata loss alone must not tear down a healthy peer connection.
        if (message.type === 'leave') {
          this.signaled.delete(message.id);
          if (this.peers.get(message.id)?.channel?.readyState !== 'open') {
            this.removeLink(message.id); this.people.delete(message.id);
          }
          this.notify();
        }
        if (message.type === 'signal') {
          if (!this.peers.has(message.from) && neighbors(this.id, this.members()).has(message.from)) this.addLink(message.from);
          const entry = this.peers.get(message.from); if (!entry) return;
          if (message.description) {
            await entry.pc.setRemoteDescription(message.description);
            for (const candidate of entry.early.splice(0)) await entry.pc.addIceCandidate(candidate);
            if (message.description.type === 'offer') {
              await entry.pc.setLocalDescription(await entry.pc.createAnswer());
              this.sendSignal(message.from, { description: entry.pc.localDescription });
            }
          } else if (message.candidate) {
            if (entry.pc.remoteDescription) await entry.pc.addIceCandidate(message.candidate);
            else if (entry.early.length < 128) entry.early.push(message.candidate);
          }
        }
      } catch (error) { this.onStatus(`Connection error: ${String(error)}`); }
    };
  }
  private rememberPerson(person: { id: string; name: string; order: number }) {
    if (person.id === this.id || (!this.people.has(person.id) && this.people.size >= ROOM_LIMIT - 1)) return;
    const old = this.people.get(person.id);
    this.signaled.add(person.id);
    this.people.set(person.id, { ...person, direct: old?.direct ?? false, connected: old?.connected ?? false, relay: old?.relay });
  }
  private sendSignal(to: string, value: object) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify({ to, ...value }));
  }
  private notify() {
    for (const person of this.people.values()) { person.direct = this.peers.has(person.id); person.connected = this.peers.get(person.id)?.channel?.readyState === 'open'; }
    this.onPeople(ordered([...this.people.values()]) as Person[]);
  }
  private scheduleTopology(delay = 350) {
    clearTimeout(this.topologyTimer);
    this.topologyTimer = setTimeout(() => this.reconcile(), delay);
    this.notify();
  }
  private removeLink(id: string) { const old = this.peers.get(id); this.peers.delete(id); old?.pc.close(); }
  private reconcile() {
    if (this.stopped || this.socket?.readyState !== WebSocket.OPEN) return;
    const wanted = neighbors(this.id, this.members());
    const ready = [...wanted].filter(id => this.peers.get(id)?.channel?.readyState === 'open').length;
    // Two spare links allow new routes to open before retiring old routes.
    for (const id of wanted) if (!this.peers.has(id)) {
      if (this.peers.size >= LINK_LIMIT && ready >= 2) {
        const stale = [...this.peers.keys()].find(id => !wanted.has(id)); if (stale) this.removeLink(stale);
      }
      this.addLink(id);
    }
    if (wanted.size && [...wanted].every(id => this.peers.get(id)?.channel?.readyState === 'open')) {
      for (const id of this.peers.keys()) if (!wanted.has(id)) this.removeLink(id);
    }
    this.notify();
  }
  private addLink(id: string) {
    if (id === this.id || this.peers.has(id) || this.peers.size >= LINK_LIMIT || !this.people.has(id)) return;
    const pc = new RTCPeerConnection(this.configuration);
    const entry: Entry = { pc, early: [], queue: [], queued: 0, controlQueue: [], controlQueued: 0, cursors: new Map() };
    this.peers.set(id, entry);
    pc.onicecandidate = ({ candidate }) => { if (candidate) this.sendSignal(id, { candidate: candidate.toJSON() }); };
    pc.onconnectionstatechange = () => {
      if (this.peers.get(id) !== entry) return;
      if (pc.connectionState === 'failed' && this.id < id) this.refreshConnectivity().then(() => {
        if (pc.connectionState !== 'closed') { pc.setConfiguration(this.configuration); pc.restartIce(); }
      }).catch(() => this.onStatus('Connection repair failed'));
      this.notify();
      if (pc.connectionState === 'connected') pc.getStats().then(stats => {
        for (const report of stats.values()) if (report.type === 'candidate-pair' && report.state === 'succeeded' && report.nominated) {
          const person = this.people.get(id); if (person) person.relay = stats.get(report.localCandidateId)?.candidateType === 'relay';
        }
        this.notify();
      }).catch(() => {});
    };
    const attach = (channel: RTCDataChannel) => {
      channel.binaryType = 'arraybuffer';
      if (!['document', 'presence', 'control'].includes(channel.label)) { channel.close(); return; }
      if (channel.label === 'control') {
        entry.control = channel; channel.bufferedAmountLowThreshold = 16 * 1024;
        channel.onopen = () => this.flush(entry); channel.onbufferedamountlow = () => this.flush(entry);
        channel.onmessage = ({ data }) => {
          if (data instanceof ArrayBuffer && data.byteLength <= 1_200) this.onFrame(new Uint8Array(data));
          else if (typeof data === 'string' && data.length <= 8_192) this.receiveRoute(id, data);
        };
      } else if (channel.label === 'presence') {
        entry.presence = channel; channel.bufferedAmountLowThreshold = 4_096;
        channel.onopen = () => this.flushPresence(entry); channel.onbufferedamountlow = () => this.flushPresence(entry);
        channel.onmessage = ({ data }) => { if (typeof data === 'string' && data.length <= 4_096) this.receiveCursor(id, data); };
      } else {
        entry.channel = channel; channel.bufferedAmountLowThreshold = 64 * 1024;
        channel.onbufferedamountlow = () => this.flush(entry);
        channel.onopen = () => { if (this.peers.get(id) === entry) { this.flush(entry); this.scheduleTopology(30); } };
        channel.onclose = () => { if (this.peers.get(id) === entry) { this.removeLink(id); if (!this.signaled.has(id)) this.people.delete(id); this.scheduleTopology(1_000); } };
        channel.onmessage = ({ data }) => { if (data instanceof ArrayBuffer && data.byteLength <= 1_200) this.onFrame(new Uint8Array(data)); };
      }
    };
    pc.ondatachannel = ({ channel }) => attach(channel);
    if (this.id < id) {
      attach(pc.createDataChannel('document', { ordered: true }));
      attach(pc.createDataChannel('presence', { ordered: false, maxRetransmits: 0 }));
      attach(pc.createDataChannel('control', { ordered: false, maxRetransmits: 0 }));
      pc.onnegotiationneeded = async () => {
        try { await pc.setLocalDescription(await pc.createOffer()); this.sendSignal(id, { description: pc.localDescription }); }
        catch { if (this.peers.get(id) === entry) this.onStatus('Connection negotiation failed'); }
      };
    }
    this.notify();
  }
  private flush(entry: Entry) {
    while (entry.control?.readyState === 'open' && entry.controlQueue.length && entry.control.bufferedAmount < 32 * 1024) {
      const bytes = entry.controlQueue.shift()!; entry.controlQueued -= bytes.byteLength; entry.control.send(bytes.buffer as ArrayBuffer);
    }
    while (entry.channel?.readyState === 'open' && entry.queue.length && entry.channel.bufferedAmount < 128 * 1024) {
      const bytes = entry.queue.shift()!; entry.queued -= bytes.byteLength; entry.channel.send(bytes.buffer as ArrayBuffer);
    }
  }
  send(to: string, bytes: Uint8Array, control = false) {
    const entry = this.peers.get(to);
    // Chunked summaries need reliable delivery; unordered probes stay separate.
    if (entry && control && new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(44, true) > 1) control = false;
    if (!entry && control && this.people.has(to)) {
      const route: Routed = { kind: 'route', origin: this.id, sequence: ++this.sequence, to, frame: Array.from(bytes), hops: 16 };
      this.remember('route', route.origin, route.sequence); this.forwardRoute(route); return;
    }
    if (entry && control) {
      if (entry.controlQueued + bytes.byteLength > 64 * 1024) return;
      entry.controlQueue.push(bytes); entry.controlQueued += bytes.byteLength; this.flush(entry); return;
    }
    if (!entry || entry.queued + bytes.byteLength > 8 * 1024 * 1024) return;
    entry.queue.push(bytes); entry.queued += bytes.byteLength; this.flush(entry);
  }
  broadcast(bytes: Uint8Array) { for (const id of this.peers.keys()) this.send(id, bytes); }
  private remember(kind: string, origin: string, sequence: number) {
    const key = `${kind}:${origin}:${sequence}`;
    if (this.seen.has(key)) return false;
    this.seen.set(key, Date.now());
    if (this.seen.size > 8_192) this.seen.delete(this.seen.keys().next().value!);
    return true;
  }
  private receiveRoute(from: string, data: string) {
    try {
      const route: Routed = JSON.parse(data);
      if (route.kind !== 'route' || !this.people.has(route.origin) || !Number.isSafeInteger(route.sequence) || route.sequence < 1
        || !Number.isInteger(route.hops) || route.hops < 0 || route.hops > 16 || !Array.isArray(route.frame) || route.frame.length > 1_200
        || !route.frame.every(n => Number.isInteger(n) && n >= 0 && n <= 255) || !this.remember('route', route.origin, route.sequence)) return;
      if (route.to === this.id) this.onFrame(new Uint8Array(route.frame));
      else this.forwardRoute({ ...route, hops: route.hops - 1 }, from);
    } catch { /* Bounded, best-effort control routing; protocol timers retry. */ }
  }
  private forwardRoute(route: Routed, from?: string) {
    if (route.hops < 0 || !this.people.has(route.to)) return;
    const choices = [...this.peers].filter(([id, entry]) => id !== from && entry.control?.readyState === 'open' && entry.control.bufferedAmount < 16 * 1024)
      .sort(([a], [b]) => distance(a, route.to, this.members()) - distance(b, route.to, this.members()));
    // Greedy ring routing normally takes a few hops. Hop limits and dedup bound
    // temporary detours while the topology is rewiring.
    choices[0]?.[1].control?.send(JSON.stringify(route));
  }
  private flushPresence(entry: Entry) {
    while (entry.presence?.readyState === 'open' && entry.presence.bufferedAmount < 4_096 && entry.cursors.size) {
      const [origin, data] = entry.cursors.entries().next().value!; entry.cursors.delete(origin); entry.presence.send(data);
    }
  }
  private forwardCursor(cursor: Cursor, from?: string) {
    if (cursor.hops < 0) return;
    const data = JSON.stringify(cursor);
    for (const [id, entry] of this.peers) if (id !== from) { entry.cursors.set(cursor.origin, data); this.flushPresence(entry); }
  }
  private receiveCursor(from: string, data: string) {
    try {
      const cursor: Cursor = JSON.parse(data);
      if (cursor.kind !== 'cursor' || !this.people.has(cursor.origin) || !Number.isSafeInteger(cursor.sequence) || cursor.sequence < 1
        || !Number.isInteger(cursor.hops) || cursor.hops < 0 || cursor.hops > 16 || !Array.isArray(cursor.cursor) || cursor.cursor.length > 128
        || !cursor.cursor.every(n => Number.isInteger(n) && n >= 0 && n <= 255) || !this.remember('cursor', cursor.origin, cursor.sequence)) return;
      this.onCursor(cursor.origin, cursor.cursor); this.forwardCursor({ ...cursor, hops: cursor.hops - 1 }, from);
    } catch { /* Presence is ephemeral. */ }
  }
  cursor(position: number[]) {
    const encoded = JSON.stringify(position);
    if (this.stopped || (encoded === this.lastCursor && Date.now() - this.cursorSent < 3_000)) return;
    this.pendingCursor = position;
    if (this.cursorTimer) return;
    this.cursorTimer = setTimeout(() => {
      this.cursorTimer = undefined;
      if (this.stopped || !this.pendingCursor) return;
      const cursor: Cursor = { kind: 'cursor', origin: this.id, sequence: ++this.sequence, cursor: this.pendingCursor, hops: 16 };
      this.lastCursor = JSON.stringify(this.pendingCursor); this.pendingCursor = undefined; this.cursorSent = Date.now();
      this.remember('cursor', cursor.origin, cursor.sequence); this.forwardCursor(cursor);
    }, Math.max(0, 160 - (Date.now() - this.cursorSent)));
  }
  pause() {
    this.stopped = true; clearTimeout(this.retry); clearTimeout(this.topologyTimer); clearTimeout(this.cursorTimer); this.cursorTimer = undefined;
    this.socket?.close(); this.socket = undefined;
    for (const entry of this.peers.values()) entry.pc.close();
    this.peers.clear(); this.people.clear(); this.signaled.clear(); this.seen.clear(); this.notify(); this.onStatus('Offline');
  }
  resume() { this.stopped = false; this.start().catch(() => this.onStatus('Connection repair failed')); }
  close() { this.pause(); }
}
