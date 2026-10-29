export type Person = { id: string; name: string; connected: boolean; relay?: boolean };
type Entry = { pc: RTCPeerConnection; channel?: RTCDataChannel; presence?: RTCDataChannel; early: RTCIceCandidateInit[]; queue: Uint8Array[]; queued: number; lastCursor?: number; lastSeen?: number };
export class Mesh {
  private socket?: WebSocket;
  private peers = new Map<string, Entry>();
  private people = new Map<string, Person>();
  private stopped = false;
  private configuration: RTCConfiguration = {};
  private retry?: ReturnType<typeof setTimeout>;
  onPeople: (people: Person[]) => void = () => {};
  onFrame: (frame: Uint8Array) => void = () => {};
  onCursor: (id: string, cursor: number[]) => void = () => {};
  onStatus: (status: string) => void = () => {};
  constructor(private room: string, private id: string, private name: string) {}
  async start() {
    const iceUrl = import.meta.env.VITE_ICE_URL ?? '/ice';
    if (iceUrl) {
      try {
        const response = await fetch(iceUrl);
        if (!response.ok) throw new Error('Unable to obtain connectivity credentials');
        this.configuration.iceServers = (await response.json()).iceServers;
      } catch {
        this.configuration.iceServers = [{ urls: 'stun:stun.l.google.com:19302' }];
        this.onStatus(navigator.onLine ? 'Connectivity service unavailable' : 'Offline');
      }
    } else this.configuration.iceServers = [{ urls: 'stun:stun.l.google.com:19302' }];
    if (new URLSearchParams(location.search).has('relay')) this.configuration.iceTransportPolicy = 'relay';
    this.signal();
  }
  private signal() {
    if (this.stopped) return;
    this.onStatus('Connecting');
    const url = new URL(import.meta.env.VITE_SIGNAL_URL ?? '/signal', location.href);
    url.protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    url.search = new URLSearchParams({ room: this.room, session: this.id, name: this.name }).toString();
    this.socket = new WebSocket(url);
    this.socket.onopen = () => this.onStatus('Online');
    this.socket.onclose = () => {
      this.onStatus('Signaling unavailable');
      if (!this.stopped) this.retry = setTimeout(() => this.signal(), 1_500);
    };
    this.socket.onmessage = async ({ data }) => {
      try {
        const message = JSON.parse(String(data));
        if (message.type === 'welcome') for (const person of message.peers) await this.add(person);
        if (message.type === 'join') await this.add(message.peer);
        // A signaling disconnect does not close a healthy data channel.
        if (message.type === 'leave') {
          if (this.peers.get(message.id)?.channel?.readyState !== 'open') {
            this.peers.get(message.id)?.pc.close(); this.peers.delete(message.id); this.people.delete(message.id);
          }
          this.notify();
        }
        if (message.type === 'signal') {
          const entry = this.peers.get(message.from);
          if (!entry) return;
          if (message.description) {
            await entry.pc.setRemoteDescription(message.description);
            for (const candidate of entry.early.splice(0)) await entry.pc.addIceCandidate(candidate);
            if (message.description.type === 'offer') {
              await entry.pc.setLocalDescription(await entry.pc.createAnswer());
              this.sendSignal(message.from, { description: entry.pc.localDescription });
            }
          } else if (message.candidate) {
            if (entry.pc.remoteDescription) await entry.pc.addIceCandidate(message.candidate);
            else entry.early.push(message.candidate);
          }
        }
      } catch (error) { this.onStatus(`Connection error: ${String(error)}`); }
    };
  }
  private sendSignal(to: string, value: object) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify({ to, ...value }));
  }
  private notify() { this.onPeople([...this.people.values()]); }
  private async add(person: { id: string; name: string }) {
    if (person.id === this.id) return;
    const existing = this.peers.get(person.id);
    if (existing?.channel?.readyState === 'open') return;
    if (existing) { existing.pc.close(); this.peers.delete(person.id); }
    if (this.peers.size >= 7) return;
    this.people.set(person.id, { ...person, connected: false });
    const pc = new RTCPeerConnection(this.configuration);
    const entry: Entry = { pc, early: [], queue: [], queued: 0 };
    this.peers.set(person.id, entry);
    pc.onicecandidate = ({ candidate }) => { if (candidate) this.sendSignal(person.id, { candidate: candidate.toJSON() }); };
    pc.onconnectionstatechange = () => {
      if (pc.connectionState === 'failed' && this.id < person.id) pc.restartIce();
      const saved = this.people.get(person.id);
      if (saved) saved.connected = entry.channel?.readyState === 'open';
      this.notify();
      if (pc.connectionState === 'connected') pc.getStats().then(stats => {
        for (const report of stats.values()) if (report.type === 'candidate-pair' && report.state === 'succeeded' && report.nominated) {
          const candidate = stats.get(report.localCandidateId);
          const state = this.people.get(person.id);
          if (state) state.relay = candidate?.candidateType === 'relay';
        }
        this.notify();
      }).catch(() => {});
    };
    const attach = (channel: RTCDataChannel) => {
      channel.binaryType = 'arraybuffer';
      if (channel.label === 'presence') {
        entry.presence = channel;
        channel.onmessage = ({ data }) => {
          try {
            const cursor = JSON.parse(String(data)).cursor;
            if (Array.isArray(cursor) && cursor.length <= 128 && cursor.every(n => Number.isInteger(n) && n >= 0 && n <= 255)) this.onCursor(person.id, cursor);
          } catch { /* ephemeral malformed presence is discarded */ }
        };
        return;
      }
      entry.channel = channel;
      channel.bufferedAmountLowThreshold = 64 * 1024;
      channel.onbufferedamountlow = () => this.flush(entry);
      channel.onopen = () => { const personState = this.people.get(person.id); if (personState) personState.connected = true; this.flush(entry); this.notify(); };
      channel.onclose = () => { const personState = this.people.get(person.id); if (personState) personState.connected = false; this.notify(); };
      channel.onmessage = ({ data }) => { if (data instanceof ArrayBuffer && data.byteLength <= 1_200) this.onFrame(new Uint8Array(data)); };
    };
    pc.ondatachannel = ({ channel }) => attach(channel);
    if (this.id < person.id) {
      attach(pc.createDataChannel('document', { ordered: true }));
      attach(pc.createDataChannel('presence', { ordered: false, maxRetransmits: 0 }));
      pc.onnegotiationneeded = async () => {
        try {
          await pc.setLocalDescription(await pc.createOffer());
          this.sendSignal(person.id, { description: pc.localDescription });
        } catch { this.onStatus('Connection negotiation failed'); }
      };
    }
    this.notify();
  }
  private flush(entry: Entry) {
    while (entry.channel?.readyState === 'open' && entry.queue.length && entry.channel.bufferedAmount < 128 * 1024) {
      const bytes = entry.queue.shift()!;
      entry.queued -= bytes.byteLength;
      entry.channel.send(bytes.buffer as ArrayBuffer);
    }
  }
  send(to: string, bytes: Uint8Array) {
    const entry = this.peers.get(to);
    if (!entry || entry.queued + bytes.byteLength > 8 * 1024 * 1024) return;
    entry.queue.push(bytes); entry.queued += bytes.byteLength; this.flush(entry);
  }
  broadcast(bytes: Uint8Array) { for (const id of this.peers.keys()) this.send(id, bytes); }
  cursor(position: number[]) {
    for (const entry of this.peers.values()) if (entry.presence?.readyState === 'open') entry.presence.send(JSON.stringify({ cursor: position }));
  }
  pause() {
    this.stopped = true; clearTimeout(this.retry); this.socket?.close();
    for (const entry of this.peers.values()) entry.pc.close();
    this.peers.clear(); this.people.clear(); this.notify(); this.onStatus('Offline');
  }
  resume() { this.stopped = false; this.signal(); }
  close() { this.stopped = true; clearTimeout(this.retry); this.socket?.close(); for (const entry of this.peers.values()) entry.pc.close(); }
}
